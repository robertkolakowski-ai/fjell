//! Klynging og forslag (PLAN-KJERNE K7 steg 4). Del av `pipeline` (L3).
//!
//! Avhenger av: store, cluster, suggest, quality, screenshot, series, rules, suggest_rules, model,
//! `super::tuning`, `super::keeper`. Grensesnitt:
//! - `compute(&[IndexRow]) -> Grouping`: ren funksjon fra lagrede fingeravtrykk til grupper og forslag
//!   (samme modulkall som hver for seg: `exact_groups`, `visual_groups`, `group_suggestions`,
//!   `blur_threshold`, `blurry_for`, `screenshot_suggestion_default`). `compute_with(rows, engine)` bruker
//!   inkrementell visuell klynging, `compute_with_rules` legger til keeper-regler (docs/mvp/REGLER.md) og
//!   `compute_tuned` terskler fra sensitivitet (V1); `Tuning::default()` og `RuleCtx::NONE` = MVP.
//! - `series_specs(rows, &Grouping, eyes, &SeriesParams)`: ren funksjon fra rader til serier (V1).
//! - `refresh_groups(store, clock) -> GroupingStats`: leser aktive rader og sensitivitet, regner og skriver
//!   gruppene i én transaksjon (`Store::apply_grouping`), deretter serier (`Store::apply_series`) og til
//!   slutt forslagsregler (`suggest_rules::evaluate`). `api` kaller den etter trash/restore/purge, og
//!   skanningen kaller den etter rask fase, hver 2 000. bilde og til slutt. Bare `state = active`
//!   deltar. Låsen på lagringen holdes under regningen, så resultatet aldri bygger på foreldet tilstand.
//!
//! Serier (V1, åpent spørsmål 6 i docs/v1/V1-PLAN.md): et bilde som alt er eksakt eller visuelt duplikat
//! (alle medlemmer av visuelle grupper, og alle utenom keeperen i eksakte grupper) deltar ikke i serier.
//! Prioritet i kort er eksakt > visuelt > serie > regel > skjermbilde > uskarpt (`store/cards.rs`), så
//! samme bilde vises aldri i flere kort. Uten øyeanalysator (Null) rangeres seriene på skarphet alene.
//!
//! Avvik fra planteksten: gruppene regnes alltid ut fra alle aktive fingeravtrykk (rimelig billig,
//! alltid riktig); bare endrede grupper skrives (`apply_grouping` gjenbruker like grupper).
//! Visuell klynging tar én representant per eksakt gruppe (keeperen) og hopper over lavinformasjon.

use super::keeper::{is_protected, pick_group, RuleCtx};
use super::tuning::{tuning, tuning_from_store, Tuning};
use super::{PipelineError, StoreHandle};
use crate::cluster::{exact_groups, Params, VisualIndex};
use crate::model::{AssetId, Clock, EyeReport, Hash64, Kind, ReasonCode, SuggestionKind};
use crate::quality::LOW_CONTRAST;
use crate::rules::{decode_rules, protecting_prefix, KeeperRule, RuleReason};
use crate::screenshot::ShotSignals;
use crate::series::{build_series, SeriesItem, SeriesParams};
use crate::store::{
    index_rows_on, plan_grouping, setting_on, source_ids_on, GroupSpec, GroupingStats, IndexRow,
    LoneSuggestion, SeriesMemberSpec, SeriesSpec, Store, KEEPER_RULES_KEY,
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
    /// Tersklene `vis` er bygget med (sensitivitet, V1).
    params: Params,
    indexed: BTreeMap<AssetId, (u64, u64)>,
    rows: usize,
}

impl Default for GroupEngine {
    fn default() -> Self {
        GroupEngine {
            vis: VisualIndex::new(Params::default()),
            params: Params::default(),
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
    compute_tuned(rows, engine, ctx, &Tuning::default())
}

/// Som `compute_with_rules`, med terskler fra sensitivitet (V1). `Tuning::default()` gir nøyaktig
/// `compute_with_rules`. Endres tersklene for visuell klynging, bygges `engine` opp på nytt.
pub fn compute_tuned(
    rows: &[IndexRow],
    engine: &mut GroupEngine,
    ctx: &RuleCtx,
    tuning: &Tuning,
) -> Grouping {
    if engine.params != tuning.visual {
        engine.params = tuning.visual.clone();
        engine.vis = VisualIndex::new(tuning.visual.clone());
        engine.indexed.clear();
    }
    engine.rows = rows.len();
    let by_id: BTreeMap<AssetId, &IndexRow> = rows.iter().map(|r| (r.id, r)).collect();
    let mut groups = Vec::new();
    // Eksakte grupper.
    // Video er med her (bytelik hash er like trygt for video som for bilder), men aldri i visuell
    // klynging, uskarphet eller skjermbilde under: video dekodes ikke.
    let hashed: Vec<(AssetId, [u8; 32])> = rows
        .iter()
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
    let threshold = blur_threshold(&scores) * tuning.blur_scale;
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

/// Serier av bilder som ikke alt er duplikater. Se modulhodet.
pub fn series_specs(
    rows: &[IndexRow],
    grouping: &Grouping,
    eyes: &BTreeMap<AssetId, EyeReport>,
    p: &SeriesParams,
) -> Vec<SeriesSpec> {
    let mut taken: BTreeSet<AssetId> = BTreeSet::new();
    for g in &grouping.groups {
        for (m, _) in &g.members {
            if !g.exact || *m != g.keeper {
                taken.insert(*m);
            }
        }
    }
    let items: Vec<SeriesItem> = rows
        .iter()
        .filter(|r| r.kind == Kind::Photo && !low_info(r) && !taken.contains(&r.id))
        .filter_map(|r| {
            Some(SeriesItem {
                id: r.id,
                taken_at: r.created_at,
                dhash: Hash64(r.dhash?),
                phash: Hash64(r.phash?),
                blur: r.blur.unwrap_or(LOW_CONTRAST),
                eyes: eyes.get(&r.id).copied(),
            })
        })
        .collect();
    build_series(&items, p)
        .into_iter()
        .map(|s| {
            let top = s.members.first().map_or(0.0, |m| m.score);
            SeriesSpec {
                keeper: s.keeper,
                members: s
                    .members
                    .iter()
                    .map(|m| SeriesMemberSpec {
                        asset: m.id,
                        score: m.score,
                        reason: m.reason,
                        // Jo større avstand til keeperen, jo tryggere forslag (0,5..=1,0).
                        confidence: (0.5 + 0.5 * (top - m.score)).clamp(0.5, 1.0),
                    })
                    .collect(),
            }
        })
        .collect()
}

/// Serier for V1, uten bilder i en beskyttet mappe (`ProtectFolder`, docs/mvp/REGLER.md): de får aldri forslag.
/// Ren funksjon; kan regnes utenfor skrivelåsen.
fn open_series(
    rows: &[IndexRow],
    g: &Grouping,
    eyes: &BTreeMap<AssetId, EyeReport>,
    tuning: &Tuning,
    ctx: &RuleCtx,
) -> Vec<SeriesSpec> {
    if ctx.rules.is_empty() {
        return series_specs(rows, g, eyes, &tuning.series);
    }
    let open: Vec<IndexRow> = rows
        .iter()
        .filter(|r| !is_protected(ctx, r.id))
        .cloned()
        .collect();
    series_specs(&open, g, eyes, &tuning.series)
}

/// V1-etterarbeid etter at gruppene er skrevet, under skrivelåsen: serier (`Store::apply_series`) og
/// forslagsregler (`suggest_rules::evaluate`). Bilder i en beskyttet mappe får heller ikke regelforslag.
fn apply_v1(
    s: &mut Store,
    series: &[SeriesSpec],
    ctx: &RuleCtx,
    now: i64,
) -> Result<(), PipelineError> {
    s.apply_series(series, now)?;
    let guard = |path: &str| protecting_prefix(ctx.rules, path).is_some();
    let guard: crate::suggest_rules::Guard = if ctx.rules.is_empty() {
        None
    } else {
        Some(&guard)
    };
    crate::suggest_rules::evaluate(s, now, guard)
        .map_err(|e| PipelineError::Rules(format!("{e}")))?;
    Ok(())
}

/// Regner gruppene på nytt og skriver endringene. Se modulhodet.
///
/// Med filbasert indeks skjer lesing (fingeravtrykk, dagens grupper), utregning og diff på en egen
/// lesetilkobling utenfor skrivelåsen; låsen tas bare for å sjekke at ingenting er skrevet imens
/// (`Store::change_counter`) og for å skrive de endrede radene. Er noe skrevet imens, forkastes
/// resultatet og utregningen gjentas; etter `TRIES` forsøk regnes det under låsen, så resultatet
/// alltid er riktig. Minnedatabaser (ingen lesetilkobling) regner alltid under låsen.
/// V1: seriene regnes også utenfor låsen (`open_series`); `apply_v1` skriver serier og evaluerer
/// forslagsregler under låsen rett etter gruppene.
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
        // Lesefeil gir normal (som `tuning_from_store`).
        let tuning = tuning(|k| {
            setting_on(&conn, &format!("sensitivity.{}", k.as_str()))
                .ok()
                .flatten()
        });
        let g = compute_tuned(
            &rows,
            &mut engine.lock().unwrap_or_else(|e| e.into_inner()),
            &ctx,
            &tuning,
        );
        let now = clock.now();
        let plan = plan_grouping(&conn, &g.groups, &g.lone, now)?;
        drop(conn);
        // Øyeanalysene leses med en kort lås; seriene regnes utenfor låsen.
        let ids: Vec<AssetId> = rows.iter().map(|r| r.id).collect();
        let eyes = store.with_store(|s| s.eye_results(&ids))?;
        let series = open_series(&rows, &g, &eyes, &tuning, &ctx);
        let applied = store.with_store(|s| -> Result<Option<GroupingStats>, PipelineError> {
            if s.change_counter() != stamp {
                return Ok(None);
            }
            let stats = s.apply_plan(&plan)?;
            apply_v1(s, &series, &ctx, now)?;
            Ok(Some(stats))
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
        let tuning = tuning_from_store(s);
        let g = compute_tuned(
            &rows,
            &mut engine.lock().unwrap_or_else(|e| e.into_inner()),
            &ctx,
            &tuning,
        );
        let now = clock.now();
        let stats = s.apply_grouping(&g.groups, &g.lone, now)?;
        let ids: Vec<AssetId> = rows.iter().map(|r| r.id).collect();
        let eyes = s.eye_results(&ids)?;
        let series = open_series(&rows, &g, &eyes, &tuning, &ctx);
        apply_v1(s, &series, &ctx, now)?;
        Ok(stats)
    })
}
