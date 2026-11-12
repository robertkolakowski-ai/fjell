//! Kortspørringer for `api` (PLAN-KJERNE «Kjerneplanen leverer»: `next_cards`). Del av `store` (L1).
//!
//! Avhenger av: `model` og `super::Store`. Bare lesing.
//! Grensesnitt:
//! - `pending_cards(kind, limit, after)`: ventende forslag på aktive bilder, mest sikre først
//!   (`confidence DESC, id`), med nøkkelsidering (`after` = konfidens og forslags-id fra forrige side).
//!   Samme bilde vises aldri i flere kort: prioritet exact_dup > visual_dup > screenshot > blurry.
//! - `suggestion_rows(ids)`: forslag per kort-id (kort-id = `suggestion.id`).
//! - `kind_totals()`: antall og byte per forslagstype med samme synlighetsregel som `pending_cards`.

use super::{Store, StoreError};
use crate::model::{AssetId, GroupId, ProtectFlags, SuggestionKind};
use rusqlite::params;

/// Prioritet (lavest tall vinner) som SQL-uttrykk for kolonnen `col`.
fn prio(col: &str) -> String {
    format!(
        "(CASE {col} WHEN 'exact_dup' THEN 0 WHEN 'visual_dup' THEN 1 WHEN 'screenshot' THEN 2 ELSE 3 END)"
    )
}

/// Ventende, synlig forslag: aktivt bilde og ingen ventende forslag med høyere prioritet.
fn visible() -> String {
    format!(
        "s.status = 'pending' AND a.state = 0 AND NOT EXISTS (SELECT 1 FROM suggestion s2 \
         WHERE s2.asset_id = s.asset_id AND s2.status = 'pending' AND {} < {})",
        prio("s2.kind"),
        prio("s.kind")
    )
}

/// Rad til `Card`.
#[derive(Clone, Debug, PartialEq)]
pub struct CardRow {
    pub id: u64,
    pub asset: AssetId,
    pub group: Option<GroupId>,
    pub keeper: Option<AssetId>,
    pub reason_code: String,
    pub confidence: f32,
    pub bytes: u64,
    pub protect: ProtectFlags,
}

/// Forslag slik `decide_many` slår det opp.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SuggestionRow {
    pub asset: AssetId,
    pub kind: SuggestionKind,
    pub pending: bool,
}

impl Store {
    pub fn pending_cards(
        &self,
        kind: SuggestionKind,
        limit: usize,
        after: Option<(f32, u64)>,
    ) -> Result<Vec<CardRow>, StoreError> {
        let (conf, id) = after.map_or((f64::MAX, 0i64), |(c, i)| (c as f64, i as i64));
        let sql = format!(
            "SELECT s.id, s.asset_id, s.grp_id, g.keeper_id, s.reason_code, s.confidence, a.bytes, \
             a.protect_flags FROM suggestion s JOIN asset a ON a.id = s.asset_id \
             LEFT JOIN grp g ON g.id = s.grp_id \
             WHERE s.kind = ?1 AND {} AND (s.confidence < ?2 OR (s.confidence = ?2 AND s.id > ?3)) \
             ORDER BY s.confidence DESC, s.id LIMIT ?4",
            visible()
        );
        let mut st = self.conn.prepare_cached(&sql)?;
        let rows = st.query_map(params![kind.as_str(), conf, id, limit as i64], |r| {
            Ok(CardRow {
                id: r.get::<_, i64>(0)? as u64,
                asset: AssetId(r.get::<_, i64>(1)? as u64),
                group: r.get::<_, Option<i64>>(2)?.map(|g| GroupId(g as u64)),
                keeper: r.get::<_, Option<i64>>(3)?.map(|k| AssetId(k as u64)),
                reason_code: r.get(4)?,
                confidence: r.get::<_, f64>(5)? as f32,
                bytes: r.get::<_, i64>(6)? as u64,
                protect: ProtectFlags(r.get::<_, i64>(7)? as u8),
            })
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    pub fn suggestion_rows(&self, ids: &[u64]) -> Result<Vec<Option<SuggestionRow>>, StoreError> {
        let mut st = self
            .conn
            .prepare_cached("SELECT asset_id, kind, status FROM suggestion WHERE id = ?1")?;
        let mut out = Vec::with_capacity(ids.len());
        for id in ids {
            let row = st
                .query_map([*id as i64], |r| {
                    Ok((
                        r.get::<_, i64>(0)?,
                        r.get::<_, String>(1)?,
                        r.get::<_, String>(2)?,
                    ))
                })?
                .next()
                .transpose()?;
            out.push(row.and_then(|(asset, kind, status)| {
                let kind = SuggestionKind::ALL
                    .into_iter()
                    .find(|k| k.as_str() == kind)?;
                Some(SuggestionRow {
                    asset: AssetId(asset as u64),
                    kind,
                    pending: status == "pending",
                })
            }));
        }
        Ok(out)
    }

    /// (type, antall, byte) for synlige ventende forslag.
    pub fn kind_totals(&self) -> Result<Vec<(SuggestionKind, u64, u64)>, StoreError> {
        let sql = format!(
            "SELECT s.kind, COUNT(*), COALESCE(SUM(a.bytes), 0) FROM suggestion s \
             JOIN asset a ON a.id = s.asset_id WHERE {} GROUP BY s.kind",
            visible()
        );
        let mut st = self.conn.prepare_cached(&sql)?;
        let rows = st.query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, i64>(1)? as u64,
                r.get::<_, i64>(2)? as u64,
            ))
        })?;
        let mut out = Vec::new();
        for row in rows {
            let (k, n, b) = row?;
            if let Some(kind) = SuggestionKind::ALL.into_iter().find(|x| x.as_str() == k) {
                out.push((kind, n, b));
            }
        }
        Ok(out)
    }

    /// Antall ventende eksakte duplikater (forslag, ikke keepere): «312 eksakte duplikater».
    pub fn exact_dup_count(&self) -> Result<u64, StoreError> {
        Ok(self.conn.query_row(
            "SELECT COUNT(*) FROM suggestion s JOIN asset a ON a.id = s.asset_id \
             WHERE s.kind = 'exact_dup' AND s.status = 'pending' AND a.state = 0",
            [],
            |r| r.get::<_, i64>(0),
        )? as u64)
    }

    /// Antall aktive bilder (for fremdrift og tester).
    pub fn active_count(&self) -> Result<u64, StoreError> {
        Ok(self
            .conn
            .query_row("SELECT COUNT(*) FROM asset WHERE state = 0", [], |r| {
                r.get::<_, i64>(0)
            })? as u64)
    }
}
