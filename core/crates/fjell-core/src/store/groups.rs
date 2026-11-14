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
//! - `plan_grouping(conn, ..) -> GroupingPlan` leser og regner ut diffen (kan kjøres på en egen
//!   lesetilkobling, utenfor skrivelåsen) og `apply_plan(plan)` skriver den (bare endrede rader).
//!   `apply_grouping` er begge deler i én transaksjon.
//! - `set_group_keeper(group, keeper)`: bytter keeper og flytter det ventende forslaget.
//! - Serier (`grp.kind = 'series'`) og forslag av typen `series` og `rule` (V1) eies av `series.rs` og
//!   `suggest_rules.rs` og røres aldri her: en database uten dem oppfører seg nøyaktig som før.
//! - Gruppen lagrer `keeper_reason` (begrunnelse for keeperen; `None` etter manuelt valg).

use super::{Store, StoreError};
use crate::model::{AssetId, GroupId, ReasonCode, SuggestionKind};
use rusqlite::{params, Connection, OptionalExtension};
use std::collections::{BTreeMap, HashSet};

/// Én gruppe (eksakt eller visuell) med medlemmer, beste først.
#[derive(Clone, Debug, PartialEq)]
pub struct GroupSpec {
    pub exact: bool,
    /// (bilde, score), beste først. Rangen er posisjonen.
    pub members: Vec<(AssetId, f32)>,
    pub keeper: AssetId,
    /// Hvorfor denne er keeper (lagres i `grp.keeper_reason`).
    pub keeper_reason: ReasonCode,
    /// Brukerregelen som valgte keeper (`RuleReason::encode`), `None` = automatikken (K5).
    pub keeper_rule: Option<String>,
    /// Keeperen er valgt fordi et beskyttet bilde (`ProtectFolder`) må bli keeper. En eksisterende
    /// gruppe med samme medlemmer gjenbrukes da bare hvis den allerede har denne keeperen, så et
    /// beskyttet bilde aldri beholder et ventende forslag fra en eldre keeper.
    pub keeper_locked: bool,
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
fn existing_groups(tx: &Connection) -> Result<Existing, StoreError> {
    let mut st = tx.prepare_cached(
        "SELECT g.id, g.kind, g.keeper_id, m.asset_id FROM grp g LEFT JOIN grp_member m ON m.grp_id = g.id \
         LEFT JOIN asset a ON a.id = m.asset_id \
         WHERE g.kind <> 'series' AND (a.state = 0 OR m.asset_id IS NULL) ORDER BY g.id, m.asset_id",
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
        let mut st = tx.prepare_cached("SELECT id FROM grp WHERE kind <> 'series'")?;
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
/// (id, status, gruppe, begrunnelse, konfidens)
type ExistingSugg = (i64, String, Option<i64>, String, f64);

fn existing_suggestions(tx: &Connection) -> Result<BTreeMap<SuggKey, ExistingSugg>, StoreError> {
    let mut st = tx.prepare_cached(
        "SELECT id, asset_id, kind, status, grp_id, reason_code, confidence FROM suggestion \
         WHERE kind NOT IN ('series', 'rule')",
    )?;
    let rows = st.query_map([], |r| {
        Ok((
            r.get::<_, i64>(0)?,
            r.get::<_, i64>(1)?,
            r.get::<_, String>(2)?,
            r.get::<_, String>(3)?,
            r.get::<_, Option<i64>>(4)?,
            r.get::<_, String>(5)?,
            r.get::<_, f64>(6)?,
        ))
    })?;
    let mut out = BTreeMap::new();
    for row in rows {
        let (id, asset, kind, status, grp, reason, conf) = row?;
        out.insert((asset, kind), (id, status, grp, reason, conf));
    }
    Ok(out)
}

/// Referanse til en gruppe i en plan: eksisterende rad eller ny (indeks i `new_groups`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum GrpRef {
    Existing(i64),
    New(usize),
}

#[derive(Clone, Debug, PartialEq)]
struct NewGroup {
    exact: bool,
    keeper: i64,
    reason: &'static str,
    rule: Option<String>,
    members: Vec<(i64, f64)>,
}

#[derive(Clone, Debug, PartialEq)]
enum SuggOp {
    Delete(i64),
    Update(i64, Option<GrpRef>, &'static str, f64),
    Insert(i64, &'static str, Option<GrpRef>, &'static str, f64),
}

/// Utregnet diff mellom ønsket gruppering og det som ligger i databasen. Lages av
/// `plan_grouping` (bare lesing) og skrives av `Store::apply_plan`.
#[derive(Clone, Debug, PartialEq)]
pub struct GroupingPlan {
    delete_groups: Vec<i64>,
    new_groups: Vec<NewGroup>,
    sugg: Vec<SuggOp>,
    stats: GroupingStats,
    ts: i64,
}

impl GroupingPlan {
    pub fn stats(&self) -> GroupingStats {
        self.stats
    }
    /// Ingenting å skrive.
    pub fn is_empty(&self) -> bool {
        self.delete_groups.is_empty() && self.new_groups.is_empty() && self.sugg.is_empty()
    }
}

/// Leser dagens grupper og forslag fra `conn` og regner ut hva som må skrives. Kan kjøres på en
/// skrivebeskyttet tilkobling (`Store::reader`), utenfor skrivelåsen.
pub fn plan_grouping(
    conn: &Connection,
    groups: &[GroupSpec],
    lone: &[LoneSuggestion],
    ts: i64,
) -> Result<GroupingPlan, StoreError> {
    let mut stats = GroupingStats::default();
    let (existing, all_ids) = existing_groups(conn)?;
    let mut kept: HashSet<i64> = HashSet::new();
    let mut new_groups: Vec<NewGroup> = Vec::new();
    // (asset, kind, gruppe, reason, confidence) som skal finnes som ventende.
    let mut desired: Vec<(i64, SuggestionKind, Option<GrpRef>, ReasonCode, f32)> = Vec::new();
    for g in groups {
        let mut ids: Vec<AssetId> = g.members.iter().map(|m| m.0).collect();
        ids.sort();
        let reuse = existing
            .get(&(g.exact, ids))
            .filter(|(_, k)| k.is_some_and(|k| g.members.iter().any(|m| m.0 .0 as i64 == k)))
            .filter(|(_, k)| !g.keeper_locked || *k == Some(g.keeper.0 as i64));
        let (gref, keeper) = match reuse {
            Some(&(id, Some(keeper))) => {
                kept.insert(id);
                stats.kept_groups += 1;
                (GrpRef::Existing(id), keeper)
            }
            _ => {
                new_groups.push(NewGroup {
                    exact: g.exact,
                    keeper: g.keeper.0 as i64,
                    reason: g.keeper_reason.key(),
                    rule: g.keeper_rule.clone(),
                    members: g
                        .members
                        .iter()
                        .map(|(m, sc)| (m.0 as i64, *sc as f64))
                        .collect(),
                });
                (GrpRef::New(new_groups.len() - 1), g.keeper.0 as i64)
            }
        };
        stats.groups += 1;
        for (m, _) in &g.members {
            if m.0 as i64 != keeper {
                desired.push((m.0 as i64, g.kind, Some(gref), g.reason, g.confidence));
            }
        }
    }
    let delete_groups: Vec<i64> = all_ids
        .into_iter()
        .filter(|id| !kept.contains(id))
        .collect();
    desired.extend(
        lone.iter()
            .map(|l| (l.asset.0 as i64, l.kind, None, l.reason, l.confidence)),
    );
    let sugg = plan_suggestions(conn, &desired, &delete_groups, &mut stats)?;
    Ok(GroupingPlan {
        delete_groups,
        new_groups,
        sugg,
        stats,
        ts,
    })
}

impl Store {
    pub fn apply_grouping(
        &mut self,
        groups: &[GroupSpec],
        lone: &[LoneSuggestion],
        ts: i64,
    ) -> Result<GroupingStats, StoreError> {
        let plan = plan_grouping(&self.conn, groups, lone, ts)?;
        self.apply_plan(&plan)
    }

    /// Skriver en plan fra `plan_grouping` i én transaksjon. Planen må være regnet ut mot dagens
    /// tilstand (kalleren sjekker `change_counter`).
    pub fn apply_plan(&mut self, plan: &GroupingPlan) -> Result<GroupingStats, StoreError> {
        if plan.is_empty() {
            return Ok(plan.stats);
        }
        self.generation += 1;
        let tx = self.conn.transaction()?;
        let mut new_ids: Vec<i64> = Vec::with_capacity(plan.new_groups.len());
        for g in &plan.new_groups {
            tx.prepare_cached(
                "INSERT INTO grp(kind, keeper_id, created_at, keeper_reason, keeper_rule) VALUES (?1, ?2, ?3, ?4, ?5)",
            )?
            .execute(params![
                kind_str(g.exact),
                g.keeper,
                plan.ts,
                g.rule.is_none().then_some(g.reason),
                g.rule
            ])?;
            let id = tx.last_insert_rowid();
            for (rank, (m, score)) in g.members.iter().enumerate() {
                tx.prepare_cached(
                    "INSERT INTO grp_member(grp_id, asset_id, rank, score) VALUES (?1,?2,?3,?4)",
                )?
                .execute(params![id, m, rank as i64, score])?;
            }
            new_ids.push(id);
        }
        for id in &plan.delete_groups {
            tx.prepare_cached("DELETE FROM grp WHERE id = ?1")?
                .execute([id])?;
        }
        let resolve = |g: &Option<GrpRef>| -> Option<i64> {
            g.map(|r| match r {
                GrpRef::Existing(id) => id,
                GrpRef::New(i) => new_ids[i],
            })
        };
        for op in &plan.sugg {
            match op {
                SuggOp::Delete(id) => {
                    tx.prepare_cached("DELETE FROM suggestion WHERE id = ?1")?
                        .execute([id])?;
                }
                SuggOp::Update(id, grp, reason, conf) => {
                    tx.prepare_cached(
                        "UPDATE suggestion SET grp_id=?2, reason_code=?3, confidence=?4 WHERE id=?1",
                    )?
                    .execute(params![id, resolve(grp), reason, conf])?;
                }
                SuggOp::Insert(asset, kind, grp, reason, conf) => {
                    tx.prepare_cached(
                        "INSERT INTO suggestion(asset_id, kind, grp_id, reason_code, confidence) \
                         VALUES (?1,?2,?3,?4,?5)",
                    )?
                    .execute(params![
                        asset,
                        kind,
                        resolve(grp),
                        reason,
                        conf
                    ])?;
                }
            }
        }
        tx.commit()?;
        Ok(plan.stats)
    }

    /// Bytter keeper i en gruppe. Det ventende forslaget flyttes fra ny til gammel keeper (som tar
    /// over type, begrunnelse og konfidens). Avviste/aksepterte forslag røres ikke.
    pub fn set_group_keeper(
        &mut self,
        group: GroupId,
        keeper: AssetId,
    ) -> Result<bool, StoreError> {
        self.generation += 1;
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
                "UPDATE grp SET keeper_id = ?2, keeper_reason = NULL, keeper_rule = NULL WHERE id = ?1",
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

/// Regner ut operasjonene som gjør de ventende forslagene lik `desired`; andre statuser røres ikke.
/// `dying` er grupper som slettes (forslag som peker dit får `grp_id = NULL` av databasen).
fn plan_suggestions(
    conn: &Connection,
    desired: &[(i64, SuggestionKind, Option<GrpRef>, ReasonCode, f32)],
    dying: &[i64],
    stats: &mut GroupingStats,
) -> Result<Vec<SuggOp>, StoreError> {
    let existing = existing_suggestions(conn)?;
    let dying: HashSet<i64> = dying.iter().copied().collect();
    let mut ops = Vec::new();
    let want: HashSet<SuggKey> = desired
        .iter()
        .map(|d| (d.0, d.1.as_str().to_string()))
        .collect();
    for (key, (id, status, ..)) in &existing {
        if status == "pending" && !want.contains(key) {
            ops.push(SuggOp::Delete(*id));
            stats.removed_suggestions += 1;
        }
    }
    for (asset, kind, grp, reason, conf) in desired {
        match existing.get(&(*asset, kind.as_str().to_string())) {
            Some((_, status, ..)) if status != "pending" => {}
            Some((id, _, old_grp, old_reason, old_conf)) => {
                // Gammel gruppe som slettes blir NULL i databasen; sammenlign mot det.
                let cur = old_grp.filter(|g| !dying.contains(g));
                let same = match grp {
                    None => cur.is_none(),
                    Some(GrpRef::Existing(g)) => cur == Some(*g),
                    Some(GrpRef::New(_)) => false,
                } && old_reason == reason.key()
                    && (*old_conf as f32) == *conf;
                if !same {
                    ops.push(SuggOp::Update(*id, *grp, reason.key(), *conf as f64));
                }
                stats.suggestions += 1;
            }
            None => {
                ops.push(SuggOp::Insert(
                    *asset,
                    kind.as_str(),
                    *grp,
                    reason.key(),
                    *conf as f64,
                ));
                stats.suggestions += 1;
            }
        }
    }
    Ok(ops)
}
