//! Utskifting av grupper og forslag (PLAN-KJERNE K6 punkt 6, brukt av K7 steg 4). Del av `store` (L1).
//!
//! Avhenger av: `model` og `super::Store`. Ingen klynging eller rangering her: kalleren leverer
//! ferdige grupper (`GroupSpec`) og forslag uten gruppe (`LoneSuggestion`).
//! Grensesnitt:
//! - `apply_grouping(&[GroupSpec], &[LoneSuggestion]) -> GroupingStats`: alt i én transaksjon.
//!   En gruppe med samme aktive medlemmer som en eksisterende gjenbrukes (id og keeper beholdes, så
//!   et manuelt keeper-valg overlever ny klynging). Øvrige gamle grupper byttes ut.
//! - Forslag: bare `pending` endres eller fjernes. `accepted`/`rejected` røres aldri, så en
//!   avvisning overstyres ikke (K5-8) og angre-historikken beholdes. Keeper får aldri forslag.
//! - `set_group_keeper(group, keeper)`: bytter keeper og flytter det ventende forslaget.

use super::{Store, StoreError};
use crate::model::{AssetId, GroupId, ReasonCode, SuggestionKind};
use rusqlite::{params, OptionalExtension, Transaction};
use std::collections::{BTreeMap, HashSet};

/// Én gruppe (eksakt eller visuell) med medlemmer, beste først.
#[derive(Clone, Debug, PartialEq)]
pub struct GroupSpec {
    pub exact: bool,
    /// (bilde, score), beste først. Rangen er posisjonen.
    pub members: Vec<(AssetId, f32)>,
    pub keeper: AssetId,
    /// Forslagstype og begrunnelse for ikke-keepere (`ExactDup` eller `VisualDup`).
    pub kind: SuggestionKind,
    pub reason: ReasonCode,
    pub confidence: f32,
}

/// Forslag uten gruppe (uskarpt, skjermbilde).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LoneSuggestion {
    pub asset: AssetId,
    pub kind: SuggestionKind,
    pub reason: ReasonCode,
    pub confidence: f32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GroupingStats {
    pub groups: u64,
    pub kept_groups: u64,
    pub suggestions: u64,
    pub removed_suggestions: u64,
}

fn kind_str(exact: bool) -> &'static str {
    if exact {
        "exact"
    } else {
        "visual"
    }
}

type Key = (bool, Vec<AssetId>);
type Existing = (BTreeMap<Key, (i64, Option<i64>)>, Vec<i64>);

/// Eksisterende grupper på aktive medlemmer: (kind, medlemmer) -> (gruppe, keeper).
fn existing_groups(tx: &Transaction) -> Result<Existing, StoreError> {
    let mut st = tx.prepare_cached(
        "SELECT g.id, g.kind, g.keeper_id, m.asset_id FROM grp g LEFT JOIN grp_member m ON m.grp_id = g.id \
         LEFT JOIN asset a ON a.id = m.asset_id WHERE a.state = 0 OR m.asset_id IS NULL ORDER BY g.id, m.asset_id",
    )?;
    let rows = st.query_map([], |r| {
        Ok((
            r.get::<_, i64>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, Option<i64>>(2)?,
            r.get::<_, Option<i64>>(3)?,
        ))
    })?;
    let mut by_id: BTreeMap<i64, (bool, Option<i64>, Vec<AssetId>)> = BTreeMap::new();
    for row in rows {
        let (id, kind, keeper, member) = row?;
        let e = by_id
            .entry(id)
            .or_insert((kind == "exact", keeper, Vec::new()));
        e.2.extend(member.map(|m| AssetId(m as u64)));
    }
    let all_ids: Vec<i64> = {
        let mut st = tx.prepare_cached("SELECT id FROM grp")?;
        let rows = st.query_map([], |r| r.get(0))?;
        rows.collect::<Result<_, _>>()?
    };
    let map = by_id
        .into_iter()
        .map(|(id, (exact, keeper, members))| ((exact, members), (id, keeper)))
        .collect();
    Ok((map, all_ids))
}

type SuggKey = (i64, String);

fn existing_suggestions(tx: &Transaction) -> Result<BTreeMap<SuggKey, (i64, String)>, StoreError> {
    let mut st = tx.prepare_cached("SELECT id, asset_id, kind, status FROM suggestion")?;
    let rows = st.query_map([], |r| {
        Ok((
            r.get::<_, i64>(0)?,
            r.get::<_, i64>(1)?,
            r.get::<_, String>(2)?,
            r.get::<_, String>(3)?,
        ))
    })?;
    let mut out = BTreeMap::new();
    for row in rows {
        let (id, asset, kind, status) = row?;
        out.insert((asset, kind), (id, status));
    }
    Ok(out)
}

impl Store {
    pub fn apply_grouping(
        &mut self,
        groups: &[GroupSpec],
        lone: &[LoneSuggestion],
        ts: i64,
    ) -> Result<GroupingStats, StoreError> {
        let tx = self.conn.transaction()?;
        let mut stats = GroupingStats::default();
        let (existing, all_ids) = existing_groups(&tx)?;
        let mut kept: HashSet<i64> = HashSet::new();
        // (asset, kind, gruppe, reason, confidence) som skal finnes som ventende.
        let mut desired: Vec<(i64, SuggestionKind, Option<i64>, ReasonCode, f32)> = Vec::new();
        for g in groups {
            let mut ids: Vec<AssetId> = g.members.iter().map(|m| m.0).collect();
            ids.sort();
            let reuse = existing
                .get(&(g.exact, ids))
                .filter(|(_, k)| k.is_some_and(|k| g.members.iter().any(|m| m.0 .0 as i64 == k)));
            let (gid, keeper) = match reuse {
                Some(&(id, Some(keeper))) => {
                    kept.insert(id);
                    stats.kept_groups += 1;
                    (id, keeper)
                }
                _ => {
                    tx.prepare_cached(
                        "INSERT INTO grp(kind, keeper_id, created_at) VALUES (?1, ?2, ?3)",
                    )?
                    .execute(params![
                        kind_str(g.exact),
                        g.keeper.0 as i64,
                        ts
                    ])?;
                    let id = tx.last_insert_rowid();
                    for (rank, (m, score)) in g.members.iter().enumerate() {
                        tx.prepare_cached(
                            "INSERT INTO grp_member(grp_id, asset_id, rank, score) VALUES (?1,?2,?3,?4)",
                        )?
                        .execute(params![id, m.0 as i64, rank as i64, *score as f64])?;
                    }
                    kept.insert(id);
                    (id, g.keeper.0 as i64)
                }
            };
            stats.groups += 1;
            for (m, _) in &g.members {
                if m.0 as i64 != keeper {
                    desired.push((m.0 as i64, g.kind, Some(gid), g.reason, g.confidence));
                }
            }
        }
        for id in all_ids.into_iter().filter(|id| !kept.contains(id)) {
            tx.prepare_cached("DELETE FROM grp WHERE id = ?1")?
                .execute([id])?;
        }
        desired.extend(
            lone.iter()
                .map(|l| (l.asset.0 as i64, l.kind, None, l.reason, l.confidence)),
        );
        sync_suggestions(&tx, &desired, &mut stats)?;
        tx.commit()?;
        Ok(stats)
    }

    /// Bytter keeper i en gruppe. Det ventende forslaget flyttes fra ny til gammel keeper (som tar
    /// over type, begrunnelse og konfidens). Avviste/aksepterte forslag røres ikke.
    pub fn set_group_keeper(
        &mut self,
        group: GroupId,
        keeper: AssetId,
    ) -> Result<bool, StoreError> {
        let tx = self.conn.transaction()?;
        let cur: Option<Option<i64>> = tx
            .prepare_cached("SELECT keeper_id FROM grp WHERE id = ?1")?
            .query_row([group.0 as i64], |r| r.get(0))
            .optional()?;
        let member: bool = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM grp_member m JOIN asset a ON a.id = m.asset_id \
             WHERE m.grp_id = ?1 AND m.asset_id = ?2 AND a.state = 0)",
            params![group.0 as i64, keeper.0 as i64],
            |r| r.get(0),
        )?;
        let Some(old) = cur.flatten() else {
            return Ok(false);
        };
        if !member {
            return Ok(false);
        }
        if old != keeper.0 as i64 {
            let moved: Option<(String, String, f64)> = tx
                .prepare_cached(
                    "SELECT kind, reason_code, confidence FROM suggestion \
                     WHERE asset_id = ?1 AND grp_id = ?2 AND status = 'pending'",
                )?
                .query_row(params![keeper.0 as i64, group.0 as i64], |r| {
                    Ok((r.get(0)?, r.get(1)?, r.get(2)?))
                })
                .optional()?;
            tx.execute(
                "UPDATE grp SET keeper_id = ?2 WHERE id = ?1",
                params![group.0 as i64, keeper.0 as i64],
            )?;
            if let Some((kind, reason, conf)) = moved {
                tx.execute(
                    "DELETE FROM suggestion WHERE asset_id = ?1 AND grp_id = ?2 AND status = 'pending'",
                    params![keeper.0 as i64, group.0 as i64],
                )?;
                tx.execute(
                    "INSERT INTO suggestion(asset_id, kind, grp_id, reason_code, confidence) \
                     VALUES (?1,?2,?3,?4,?5) ON CONFLICT(asset_id, kind) DO NOTHING",
                    params![old, kind, group.0 as i64, reason, conf],
                )?;
            }
        }
        tx.commit()?;
        Ok(true)
    }
}

/// Gjør de ventende forslagene lik `desired`; andre statuser røres ikke.
fn sync_suggestions(
    tx: &Transaction,
    desired: &[(i64, SuggestionKind, Option<i64>, ReasonCode, f32)],
    stats: &mut GroupingStats,
) -> Result<(), StoreError> {
    let existing = existing_suggestions(tx)?;
    let want: HashSet<SuggKey> = desired
        .iter()
        .map(|d| (d.0, d.1.as_str().to_string()))
        .collect();
    for (key, (id, status)) in &existing {
        if status == "pending" && !want.contains(key) {
            tx.prepare_cached("DELETE FROM suggestion WHERE id = ?1")?
                .execute([id])?;
            stats.removed_suggestions += 1;
        }
    }
    for (asset, kind, grp, reason, conf) in desired {
        match existing.get(&(*asset, kind.as_str().to_string())) {
            Some((_, status)) if status != "pending" => {}
            Some((id, _)) => {
                tx.prepare_cached(
                    "UPDATE suggestion SET grp_id=?2, reason_code=?3, confidence=?4 WHERE id=?1",
                )?
                .execute(params![id, grp, reason.key(), *conf as f64])?;
                stats.suggestions += 1;
            }
            None => {
                tx.prepare_cached(
                    "INSERT INTO suggestion(asset_id, kind, grp_id, reason_code, confidence) \
                     VALUES (?1,?2,?3,?4,?5)",
                )?
                .execute(params![
                    asset,
                    kind.as_str(),
                    grp,
                    reason.key(),
                    *conf as f64
                ])?;
                stats.suggestions += 1;
            }
        }
    }
    Ok(())
}
