//! Klynging og forslag (PLAN-KJERNE K7 steg 4). Del av `pipeline` (L3).
//!
//! Avhenger av: store, cluster, suggest, quality, screenshot, model. Grensesnitt:
//! - `compute(&[IndexRow]) -> Grouping`: ren funksjon fra lagrede fingeravtrykk til grupper og forslag
//!   (samme modulkall som hver for seg: `exact_groups`, `visual_groups`, `group_suggestions`,
//!   `blur_threshold`, `blurry_for`, `screenshot_suggestion_default`).
//! - `refresh_groups(store, clock) -> GroupingStats`: leser aktive rader, regner og skriver gruppene
//!   i én transaksjon (`Store::apply_grouping`). `api` kaller den etter trash/restore/purge, og
//!   skanningen kaller den etter rask fase, hver 2 000. bilde og til slutt. Bare `state = active`
//!   deltar. Låsen på lagringen holdes under regningen, så resultatet aldri bygger på foreldet tilstand.
//!
//! Avvik fra planteksten: gruppene regnes alltid ut fra alle aktive fingeravtrykk (rimelig billig,
//! alltid riktig); bare endrede grupper skrives (`apply_grouping` gjenbruker like grupper).
//! Visuell klynging tar én representant per eksakt gruppe (keeperen) og hopper over lavinformasjon.

use super::keeper::{is_protected, pick_group, RuleCtx};
use super::{PipelineError, StoreHandle};
use crate::cluster::{exact_groups, Params, VisualIndex};
use crate::model::{AssetId, Clock, Hash64, Kind, ReasonCode, SuggestionKind};
use crate::quality::LOW_CONTRAST;
use crate::rules::{decode_rules, KeeperRule, RuleReason};
use crate::screenshot::ShotSignals;
use crate::store::{
    index_rows_on, plan_grouping, setting_on, source_ids_on, GroupSpec, GroupingStats, IndexRow,
    LoneSuggestion, KEEPER_RULES_KEY,
};
use crate::suggest::{
    blur_threshold, blurry_for, screenshot_suggestion_default, GroupKind, GroupResult, Member,
};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Mutex;

/// Bit i `fingerprint.shot_signals` for lavinformasjon (signalene bruker de fire laveste).
pub const LOW_INFO_BIT: u8 = 0x10;

/// Ferdig utregnet gruppering.
#[derive(Clone, Debug, PartialEq)]
pub struct Grouping {
    pub groups: Vec<GroupSpec>,
    pub lone: Vec<LoneSuggestion>,
}

pub(crate) fn member(r: &IndexRow) -> Member {
    Member {
        id: r.id,
        width: r.width,
        height: r.height,
        bytes: r.bytes,
        blur_score: r.blur.unwrap_or(LOW_CONTRAST),
        created_at: r.created_at,
        protect_flags: r.protect,
    }
}

fn low_info(r: &IndexRow) -> bool {
    r.signals.is_some_and(|s| s & LOW_INFO_BIT != 0)
}

/// Keeper først, så resten på id; score = skarphet (0 hvis ikke vurderbar).
fn spec(
    exact: bool,
    res: &GroupResult,
    by_id: &BTreeMap<AssetId, &IndexRow>,
    kind: SuggestionKind,
    reason: ReasonCode,
    confidence: f32,
    rule: Option<&RuleReason>,
) -> GroupSpec {
    let locked = matches!(rule, Some(RuleReason::ProtectedFolder { .. }));
    let mut ids: Vec<AssetId> = res.suggestions.iter().map(|s| s.asset_id).collect();
    ids.insert(0, res.keeper);
    let members = ids
        .into_iter()
        .map(|id| {
            let b = by_id[&id].blur.filter(|b| *b < LOW_CONTRAST).unwrap_or(0.0);
            (id, b)
        })
        .collect();
    GroupSpec {
        exact,
        members,
        keeper: res.keeper,
        keeper_reason: res.keeper_reason,
        keeper_rule: rule.map(RuleReason::encode),
        keeper_locked: locked,
        kind,
        reason,
        confidence,
    }
}

fn max_hamming(ids: &[AssetId], h: &BTreeMap<AssetId, (Hash64, Hash64)>) -> u32 {
    let mut m = 0;
    for (i, a) in ids.iter().enumerate() {
        for b in &ids[i + 1..] {
            let ((da, pa), (db, pb)) = (h[a], h[b]);
            m = m.max(da.hamming(db)).max(pa.hamming(pb));
        }
    }
    m
}

/// Hurtigbuffer for visuell klynging mellom utregninger: `VisualIndex` bygger bare om komponentene som
/// berøres, så en ny utregning etter trash/angre koster omtrent diffen, ikke hele biblioteket.
pub struct GroupEngine {
    vis: VisualIndex,
    indexed: BTreeMap<AssetId, (u64, u64)>,
    rows: usize,
}

impl Default for GroupEngine {
    fn default() -> Self {
        GroupEngine {
            vis: VisualIndex::new(Params::default()),
            indexed: BTreeMap::new(),
            rows: 0,
        }
    }
}

impl GroupEngine {
    pub fn new() -> GroupEngine {
        GroupEngine::default()
    }

    /// Antall aktive bilder med fingeravtrykk i siste utregning (0 før første).
    pub fn rows(&self) -> usize {
        self.rows
    }
}

/// Ren utregning uten buffer (full bygging hver gang). Gir samme resultat som `compute_with`.
pub fn compute(rows: &[IndexRow]) -> Grouping {
    compute_with(rows, &mut GroupEngine::new())
}

/// Utregning med inkrementell visuell klynging via `engine`.
pub fn compute_with(rows: &[IndexRow], engine: &mut GroupEngine) -> Grouping {
    compute_with_rules(rows, engine, &RuleCtx::NONE)
}

/// Som `compute_with`, med brukerregler for keeper-valg og beskyttede mapper. Med `RuleCtx::NONE`
/// er resultatet byte-likt `compute_with` (K5 alene).
pub fn compute_with_rules(rows: &[IndexRow], engine: &mut GroupEngine, ctx: &RuleCtx) -> Grouping {
    engine.rows = rows.len();
    let by_id: BTreeMap<AssetId, &IndexRow> = rows.iter().map(|r| (r.id, r)).collect();
    let mut groups = Vec::new();
    // Eksakte grupper.
    let hashed: Vec<(AssetId, [u8; 32])> = rows
        .iter()
        .filter(|r| r.kind != Kind::Video)
        .filter_map(|r| r.content_hash.map(|h| (r.id, h)))
        .collect();
    let mut reps: BTreeSet<AssetId> = rows
        .iter()
        .filter(|r| r.kind == Kind::Photo && !low_info(r))
        .map(|r| r.id)
        .collect();
    for g in exact_groups(&hashed) {
        let members: Vec<Member> = g.members.iter().map(|id| member(by_id[id])).collect();
        let Some(pick) = pick_group(GroupKind::Exact, &members, ctx) else {
            // Alle beskyttet: ingen gruppe, og ingen av dem går videre til visuell klynging.
            if !members.is_empty() {
                g.members.iter().for_each(|m| {
                    reps.remove(m);
                });
            }
            continue;
        };
        let res = pick.result;
        // Bare keeperen går videre til visuell klynging.
        for m in &g.members {
            if *m != res.keeper {
                reps.remove(m);
            }
        }
        groups.push(spec(
            true,
            &res,
            &by_id,
            SuggestionKind::ExactDup,
            ReasonCode::ExactCopy,
            1.0,
            pick.rule.as_ref(),
        ));
    }
    // Visuelle grupper.
    let hashes: BTreeMap<AssetId, (Hash64, Hash64)> = rows
        .iter()
        .filter(|r| reps.contains(&r.id))
        .filter_map(|r| Some((r.id, (Hash64(r.dhash?), Hash64(r.phash?)))))
        .collect();
    let wanted: BTreeMap<AssetId, (u64, u64)> = hashes
        .iter()
        .map(|(id, (d, p))| (*id, (d.0, p.0)))
        .collect();
    let removed: Vec<AssetId> = engine
        .indexed
        .keys()
        .filter(|id| !wanted.contains_key(id))
        .copied()
        .collect();
    let upserts: Vec<(AssetId, Hash64, Hash64)> = hashes
        .iter()
        .filter(|(id, (d, p))| engine.indexed.get(id) != Some(&(d.0, p.0)))
        .map(|(id, (d, p))| (*id, *d, *p))
        .collect();
    engine.vis.update(&upserts, &removed);
    engine.indexed = wanted;
    for g in engine.vis.groups() {
        let members: Vec<Member> = g.members.iter().map(|id| member(by_id[id])).collect();
        let max_hamming = max_hamming(&g.members, &hashes);
        let kind = GroupKind::Visual { max_hamming };
        let Some(pick) = pick_group(kind, &members, ctx) else {
            continue;
        };
        let res = pick.result;
        let conf = res.suggestions.first().map_or(0.0, |s| s.confidence);
        groups.push(spec(
            false,
            &res,
            &by_id,
            SuggestionKind::VisualDup,
            ReasonCode::VisualDuplicate,
            conf,
            pick.rule.as_ref(),
        ));
    }
    // Uten gruppe: uskarpt og skjermbilde.
    let mut lone = Vec::new();
    let scores: Vec<f32> = rows.iter().filter_map(|r| r.blur).collect();
    let threshold = blur_threshold(&scores);
    for r in rows
        .iter()
        .filter(|r| r.algo > 0 && !is_protected(ctx, r.id))
    {
        if r.kind == Kind::Photo {
            if let Some(s) = r.blur.and_then(|b| blurry_for(r.id, b, threshold)) {
                lone.push(LoneSuggestion {
                    asset: r.id,
                    kind: s.kind,
                    reason: s.reason,
                    confidence: s.confidence,
                });
            }
        }
        if r.kind != Kind::Video {
            let sig = ShotSignals(r.signals.unwrap_or(0) & 0x0F);
            if let Some(s) = screenshot_suggestion_default(r.id, sig) {
                lone.push(LoneSuggestion {
                    asset: r.id,
                    kind: s.kind,
                    reason: s.reason,
                    confidence: s.confidence,
                });
            }
        }
    }
    Grouping { groups, lone }
}

/// Regner gruppene på nytt og skriver endringene. Se modulhodet.
///
/// Med filbasert indeks skjer lesing (fingeravtrykk, dagens grupper), utregning og diff på en egen
/// lesetilkobling utenfor skrivelåsen; låsen tas bare for å sjekke at ingenting er skrevet imens
/// (`Store::change_counter`) og for å skrive de endrede radene. Er noe skrevet imens, forkastes
/// resultatet og utregningen gjentas; etter `TRIES` forsøk regnes det under låsen, så resultatet
/// alltid er riktig. Minnedatabaser (ingen lesetilkobling) regner alltid under låsen.
pub fn refresh_groups<H: StoreHandle>(
    store: &H,
    clock: &dyn Clock,
) -> Result<GroupingStats, PipelineError> {
    const TRIES: usize = 3;
    let local = Mutex::new(GroupEngine::new());
    let engine = store.engine().unwrap_or(&local);
    for _ in 0..TRIES {
        let (reader, stamp) = store.with_store(|s| (s.reader(), s.change_counter()));
        let Some(conn) = reader? else { break };
        let rows = index_rows_on(&conn)?;
        let rules = decode_rules(&setting_on(&conn, KEEPER_RULES_KEY)?.unwrap_or_default());
        let paths = if rules.is_empty() {
            BTreeMap::new()
        } else {
            source_ids_on(&conn)?
        };
        let ctx = RuleCtx {
            rules: &rules,
            paths: &paths,
        };
        let g = compute_with_rules(
            &rows,
            &mut engine.lock().unwrap_or_else(|e| e.into_inner()),
            &ctx,
        );
        let plan = plan_grouping(&conn, &g.groups, &g.lone, clock.now())?;
        drop(conn);
        let applied = store.with_store(|s| {
            if s.change_counter() != stamp {
                return Ok(None);
            }
            s.apply_plan(&plan).map(Some)
        })?;
        if let Some(stats) = applied {
            return Ok(stats);
        }
    }
    store.with_store(|s| {
        let rows = s.index_rows()?;
        let rules: Vec<KeeperRule> =
            decode_rules(&s.setting(KEEPER_RULES_KEY)?.unwrap_or_default());
        let paths = if rules.is_empty() {
            BTreeMap::new()
        } else {
            s.active_paths()?
        };
        let ctx = RuleCtx {
            rules: &rules,
            paths: &paths,
        };
        let g = compute_with_rules(
            &rows,
            &mut engine.lock().unwrap_or_else(|e| e.into_inner()),
            &ctx,
        );
        Ok(s.apply_grouping(&g.groups, &g.lone, clock.now())?)
    })
}
