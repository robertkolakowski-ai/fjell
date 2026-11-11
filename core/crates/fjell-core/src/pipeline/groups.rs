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

use super::{PipelineError, StoreHandle};
use crate::cluster::{exact_groups, visual_groups, Params};
use crate::model::{AssetId, Clock, Hash64, Kind, ReasonCode, SuggestionKind};
use crate::quality::LOW_CONTRAST;
use crate::screenshot::ShotSignals;
use crate::store::{GroupSpec, GroupingStats, IndexRow, LoneSuggestion};
use crate::suggest::{
    blur_threshold, blurry_for, group_suggestions, screenshot_suggestion_default, GroupKind,
    GroupResult, Member,
};
use std::collections::{BTreeMap, BTreeSet};

/// Bit i `fingerprint.shot_signals` for lavinformasjon (signalene bruker de fire laveste).
pub const LOW_INFO_BIT: u8 = 0x10;

/// Ferdig utregnet gruppering.
#[derive(Clone, Debug, PartialEq)]
pub struct Grouping {
    pub groups: Vec<GroupSpec>,
    pub lone: Vec<LoneSuggestion>,
}

fn member(r: &IndexRow) -> Member {
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
) -> GroupSpec {
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

pub fn compute(rows: &[IndexRow]) -> Grouping {
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
        let Some(res) = group_suggestions(GroupKind::Exact, &members) else {
            continue;
        };
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
        ));
    }
    // Visuelle grupper.
    let hashes: BTreeMap<AssetId, (Hash64, Hash64)> = rows
        .iter()
        .filter(|r| reps.contains(&r.id))
        .filter_map(|r| Some((r.id, (Hash64(r.dhash?), Hash64(r.phash?)))))
        .collect();
    let items: Vec<(AssetId, Hash64, Hash64)> =
        hashes.iter().map(|(id, (d, p))| (*id, *d, *p)).collect();
    for g in visual_groups(&items, &Params::default()) {
        let members: Vec<Member> = g.members.iter().map(|id| member(by_id[id])).collect();
        let max_hamming = max_hamming(&g.members, &hashes);
        let kind = GroupKind::Visual { max_hamming };
        let Some(res) = group_suggestions(kind, &members) else {
            continue;
        };
        let conf = res.suggestions.first().map_or(0.0, |s| s.confidence);
        groups.push(spec(
            false,
            &res,
            &by_id,
            SuggestionKind::VisualDup,
            ReasonCode::VisualDuplicate,
            conf,
        ));
    }
    // Uten gruppe: uskarpt og skjermbilde.
    let mut lone = Vec::new();
    let scores: Vec<f32> = rows.iter().filter_map(|r| r.blur).collect();
    let threshold = blur_threshold(&scores);
    for r in rows.iter().filter(|r| r.algo > 0) {
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
pub fn refresh_groups<H: StoreHandle>(
    store: &H,
    clock: &dyn Clock,
) -> Result<GroupingStats, PipelineError> {
    store.with_store(|s| {
        let rows = s.index_rows()?;
        let g = compute(&rows);
        Ok(s.apply_grouping(&g.groups, &g.lone, clock.now())?)
    })
}
