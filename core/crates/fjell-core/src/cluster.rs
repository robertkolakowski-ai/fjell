//! Modul `cluster` (L2). Se docs/mvp/ARKITEKTUR.md del 2 og PLAN-KJERNE del 3 og 4 (K1, K2b).
//!
//! Avhenger av: model. Ren funksjon på hashlister, ingen database.
//! Grensesnitt (hovedpunkter):
//! - `exact_groups(&[(AssetId,[u8;32])]) -> Vec<Group>`: lik `content_hash`, bare grupper med minst
//!   to medlemmer. Resultatet er stabilt: medlemmer sortert på `AssetId`, grupper på første medlem.
//! - `visual_groups(&[(AssetId,Hash64,Hash64)], &Params) -> Vec<Group>`: MIH + union-find (se `visual`).
//! - `VisualIndex`: inkrementell variant (`update`), gir samme grupper som full ombygging.
//!
//! Kalleren leverer bare aktive bilder (kind = photo, ikke `low_info`) og én representant per eksakt
//! gruppe, slik at ett bilde aldri gir både eksakt og visuelt forslag.

use crate::model::AssetId;
use std::collections::BTreeMap;

mod visual;
pub use visual::{similar_pairs, visual_groups, Params, Stats, VisualIndex};

#[cfg(test)]
mod tests;

/// En gruppe med minst to bilder. `members` er sortert på `AssetId`.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Group {
    pub members: Vec<AssetId>,
}

/// Eksakte grupper: alle bilder med lik hash. Uavhengig av inndatarekkefølge.
pub fn exact_groups(items: &[(AssetId, [u8; 32])]) -> Vec<Group> {
    let mut by_hash: BTreeMap<[u8; 32], Vec<AssetId>> = BTreeMap::new();
    for (id, h) in items {
        by_hash.entry(*h).or_default().push(*id);
    }
    let mut out: Vec<Group> = by_hash
        .into_values()
        .filter_map(|mut v| {
            v.sort();
            v.dedup();
            (v.len() > 1).then_some(Group { members: v })
        })
        .collect();
    out.sort();
    out
}
