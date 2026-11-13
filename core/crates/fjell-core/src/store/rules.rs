//! Lagring for keeper-regler (se `rules` og docs/mvp/REGLER.md). Del av `store` (L1).
//!
//! Avhenger av: model og `super::Store`. Kjenner ikke reglenes innhold: regellisten er en tekst i
//! `setting` (`keeper_rules`), og gruppens avgjørende regel er en tekst i `grp.keeper_rule`.
//! Grensesnitt:
//! - `setting_on(conn, key)`, `source_ids_on(conn)`: lesing på en gitt tilkobling (f.eks. `Store::reader`).
//! - `rule_groups_on(conn)`: grupper reglene kan gjelde for på «bruk regler på gjenstående grupper»:
//!   alle medlemmer aktive, bare ventende forslag, og keeper ikke valgt manuelt.
//! - `Store::set_group_keeper_by_rule(gruppe, keeper, regel, K5-begrunnelse)`: bytter keeper og lagrer regelen.

use super::{Store, StoreError};
use crate::model::{AssetId, GroupId, ReasonCode};
use rusqlite::{params, Connection, OptionalExtension};
use std::collections::BTreeMap;

pub const KEEPER_RULES_KEY: &str = "keeper_rules";

/// Gruppe som `apply_keeper_rules` kan endre.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RuleGroupRow {
    pub id: GroupId,
    pub exact: bool,
    pub keeper: AssetId,
    /// Aktive medlemmer, sortert på id.
    pub members: Vec<AssetId>,
}

pub fn setting_on(conn: &Connection, key: &str) -> Result<Option<String>, StoreError> {
    Ok(conn
        .query_row("SELECT value FROM setting WHERE key = ?1", [key], |r| {
            r.get(0)
        })
        .optional()?)
}

/// Kilde-id (stien) til alle aktive bilder.
pub fn source_ids_on(conn: &Connection) -> Result<BTreeMap<AssetId, String>, StoreError> {
    let mut st = conn.prepare_cached("SELECT id, source_id FROM asset WHERE state = 0")?;
    let rows = st.query_map([], |r| Ok((AssetId(r.get::<_, i64>(0)? as u64), r.get(1)?)))?;
    Ok(rows.collect::<Result<_, _>>()?)
}

pub fn rule_groups_on(conn: &Connection) -> Result<Vec<RuleGroupRow>, StoreError> {
    let mut st = conn.prepare_cached(
        "SELECT g.id, g.kind, g.keeper_id, m.asset_id FROM grp g \
         JOIN grp_member m ON m.grp_id = g.id JOIN asset a ON a.id = m.asset_id \
         WHERE g.keeper_id IS NOT NULL AND (g.keeper_reason IS NOT NULL OR g.keeper_rule IS NOT NULL) \
           AND NOT EXISTS (SELECT 1 FROM suggestion s WHERE s.grp_id = g.id AND s.status <> 'pending') \
           AND NOT EXISTS (SELECT 1 FROM grp_member m2 JOIN asset a2 ON a2.id = m2.asset_id \
                           WHERE m2.grp_id = g.id AND a2.state <> 0) \
         ORDER BY g.id, m.asset_id",
    )?;
    let rows = st.query_map([], |r| {
        Ok((
            r.get::<_, i64>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, i64>(2)?,
            r.get::<_, i64>(3)?,
        ))
    })?;
    let mut by: BTreeMap<i64, RuleGroupRow> = BTreeMap::new();
    for row in rows {
        let (id, kind, keeper, member) = row?;
        by.entry(id)
            .or_insert_with(|| RuleGroupRow {
                id: GroupId(id as u64),
                exact: kind == "exact",
                keeper: AssetId(keeper as u64),
                members: Vec::new(),
            })
            .members
            .push(AssetId(member as u64));
    }
    Ok(by.into_values().filter(|g| g.members.len() > 1).collect())
}

impl Store {
    /// Som `source_ids_on` på skrivetilkoblingen.
    pub fn active_paths(&self) -> Result<BTreeMap<AssetId, String>, StoreError> {
        source_ids_on(&self.conn)
    }

    /// Som `rule_groups_on` på skrivetilkoblingen.
    pub fn rule_groups(&self) -> Result<Vec<RuleGroupRow>, StoreError> {
        rule_groups_on(&self.conn)
    }

    /// Bytter keeper (som `set_group_keeper`) og lagrer regelen som avgjorde (`RuleReason::encode`;
    /// `None` = automatikken K5). Gir `false` hvis gruppen eller keeperen ikke finnes.
    pub fn set_group_keeper_by_rule(
        &mut self,
        group: GroupId,
        keeper: AssetId,
        rule: Option<&str>,
        reason: ReasonCode,
    ) -> Result<bool, StoreError> {
        if !self.set_group_keeper(group, keeper)? {
            return Ok(false);
        }
        // `set_group_keeper` nullstiller begrunnelsen (manuelt valg). Regelvalg skal ikke se manuelle ut:
        // uten regel er det K5 som valgte, så begrunnelsen er K5-koden `reason`.
        self.generation += 1;
        self.conn.execute(
            "UPDATE grp SET keeper_rule = ?2, keeper_reason = CASE WHEN ?2 IS NULL THEN ?3 ELSE NULL END \
             WHERE id = ?1",
            params![group.0 as i64, rule, reason.key()],
        )?;
        Ok(true)
    }
}
