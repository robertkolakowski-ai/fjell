//! Lesespørringer for trygg sletting (PLAN-TRYGGHET del 3 og 4). Del av `store` (L1).
//!
//! Avhenger av: `model`, `super::Store`. Ingen skriving.
//! Grensesnitt:
//! - Enkeltoppslag: `asset_row`, `group_active_members`, `pending_suggestions`, `trash_totals`,
//!   `freed_bytes`, `setting`.
//! - Mengdeoppslag (`asset_rows`, `albums_for`, `groups_for`, `active_members_of`, `reasons_for`):
//!   ett kall per mengde via `json_each(?)`, så ingen `IN (...)`-liste og ingen variabelgrense (T18).
//!   Det holder `plan(1 000 id-er)` under 10 ms.

use super::{Store, StoreError};
use crate::model::{AssetId, GroupId, ProtectFlags, State, SuggestionKind};
use rusqlite::OptionalExtension;
use std::collections::BTreeMap;

/// Rad fra `asset` med feltene trygg sletting trenger.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AssetRow {
    pub id: AssetId,
    pub state: State,
    pub trashed_at: Option<i64>,
    pub expires_at: Option<i64>,
    pub expired_notified_at: Option<i64>,
    pub bytes: u64,
    pub protect: ProtectFlags,
    /// `None` = ukjent.
    pub backup: Option<bool>,
}

/// Ventende forslag (til «forhåndsvisning av alle forslag»).
#[derive(Clone, Debug, PartialEq)]
pub struct PendingSuggestion {
    pub asset_id: AssetId,
    pub kind: SuggestionKind,
    pub reason_code: String,
    pub group: Option<GroupId>,
}

fn kind_from(s: &str) -> Option<SuggestionKind> {
    SuggestionKind::ALL.into_iter().find(|k| k.as_str() == s)
}

pub(super) fn row_of(
    tx: &rusqlite::Connection,
    id: AssetId,
) -> Result<Option<AssetRow>, StoreError> {
    let mut st = tx.prepare_cached(
        "SELECT state, trashed_at, expires_at, expired_notified_at, bytes, protect_flags, backup \
         FROM asset WHERE id = ?1",
    )?;
    let r = st
        .query_row([id.0 as i64], |r| {
            Ok(AssetRow {
                id,
                state: State::from_i64(r.get(0)?).unwrap_or(State::Purged),
                trashed_at: r.get(1)?,
                expires_at: r.get(2)?,
                expired_notified_at: r.get(3)?,
                bytes: r.get::<_, i64>(4)? as u64,
                protect: ProtectFlags(r.get::<_, i64>(5)? as u8),
                backup: r.get::<_, Option<i64>>(6)?.map(|v| v != 0),
            })
        })
        .optional()?;
    Ok(r)
}

/// JSON-liste med id-er til `json_each`.
fn ids_json(ids: &[AssetId]) -> String {
    let mut s = String::with_capacity(ids.len() * 7 + 2);
    s.push('[');
    for (i, id) in ids.iter().enumerate() {
        if i > 0 {
            s.push(',');
        }
        s.push_str(&id.0.to_string());
    }
    s.push(']');
    s
}

impl Store {
    /// Rader for `ids` i samme rekkefølge; `None` for ukjente id-er.
    pub fn asset_rows(&self, ids: &[AssetId]) -> Result<Vec<Option<AssetRow>>, StoreError> {
        let mut st = self.conn.prepare_cached(
            "SELECT a.id, a.state, a.trashed_at, a.expires_at, a.expired_notified_at, a.bytes, a.protect_flags, a.backup \
             FROM json_each(?1) j LEFT JOIN asset a ON a.id = j.value ORDER BY j.key",
        )?;
        let rows = st.query_map([ids_json(ids)], |r| {
            let Some(id) = r.get::<_, Option<i64>>(0)? else {
                return Ok(None);
            };
            Ok(Some(AssetRow {
                id: AssetId(id as u64),
                state: State::from_i64(r.get(1)?).unwrap_or(State::Purged),
                trashed_at: r.get(2)?,
                expires_at: r.get(3)?,
                expired_notified_at: r.get(4)?,
                bytes: r.get::<_, i64>(5)? as u64,
                protect: ProtectFlags(r.get::<_, i64>(6)? as u8),
                backup: r.get::<_, Option<i64>>(7)?.map(|v| v != 0),
            }))
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// (bilde, album-id, albumnavn) for alle album bildene ligger i.
    pub fn albums_for(&self, ids: &[AssetId]) -> Result<Vec<(AssetId, u64, String)>, StoreError> {
        let mut st = self.conn.prepare_cached(
            "SELECT aa.asset_id, al.id, al.name FROM json_each(?1) j \
             JOIN asset_album aa ON aa.asset_id = j.value JOIN album al ON al.id = aa.album_id",
        )?;
        let rows = st.query_map([ids_json(ids)], |r| {
            Ok((
                AssetId(r.get::<_, i64>(0)? as u64),
                r.get::<_, i64>(1)? as u64,
                r.get(2)?,
            ))
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// Alle (bilde, gruppe)-par for bildene.
    pub fn groups_for(&self, ids: &[AssetId]) -> Result<Vec<(AssetId, GroupId)>, StoreError> {
        let mut st = self.conn.prepare_cached(
            "SELECT m.asset_id, m.grp_id FROM json_each(?1) j JOIN grp_member m ON m.asset_id = j.value",
        )?;
        let rows = st.query_map([ids_json(ids)], |r| {
            Ok((
                AssetId(r.get::<_, i64>(0)? as u64),
                GroupId(r.get::<_, i64>(1)? as u64),
            ))
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// Aktive medlemmer per gruppe, beste først (keeper, deretter `rank`).
    pub fn active_members_of(
        &self,
        groups: &[GroupId],
    ) -> Result<BTreeMap<GroupId, Vec<AssetId>>, StoreError> {
        let json = format!(
            "[{}]",
            groups
                .iter()
                .map(|g| g.0.to_string())
                .collect::<Vec<_>>()
                .join(",")
        );
        let mut st = self.conn.prepare_cached(
            "SELECT m.grp_id, m.asset_id FROM json_each(?1) j JOIN grp_member m ON m.grp_id = j.value \
             JOIN asset a ON a.id = m.asset_id JOIN grp ON grp.id = m.grp_id WHERE a.state = 0 \
             ORDER BY m.grp_id, (m.asset_id = grp.keeper_id) DESC, m.rank, m.asset_id",
        )?;
        let rows = st.query_map([json], |r| {
            Ok((
                GroupId(r.get::<_, i64>(0)? as u64),
                AssetId(r.get::<_, i64>(1)? as u64),
            ))
        })?;
        let mut out: BTreeMap<GroupId, Vec<AssetId>> = BTreeMap::new();
        for row in rows {
            let (g, a) = row?;
            out.entry(g).or_default().push(a);
        }
        Ok(out)
    }

    /// Ventende forslag per bilde (mest sikre først). Brukes til `ReasonCode` i planen.
    pub fn reasons_for(
        &self,
        ids: &[AssetId],
    ) -> Result<Vec<(AssetId, SuggestionKind, String)>, StoreError> {
        let mut st = self.conn.prepare_cached(
            "SELECT s.asset_id, s.kind, s.reason_code FROM json_each(?1) j \
             JOIN suggestion s ON s.asset_id = j.value WHERE s.status = 'pending' ORDER BY s.confidence DESC, s.id",
        )?;
        let rows = st.query_map([ids_json(ids)], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
            ))
        })?;
        let mut out = Vec::new();
        for row in rows {
            let (a, k, reason) = row?;
            if let Some(kind) = kind_from(&k) {
                out.push((AssetId(a as u64), kind, reason));
            }
        }
        Ok(out)
    }

    /// Rad for `id`, eller `None` hvis ukjent.
    pub fn asset_row(&self, id: AssetId) -> Result<Option<AssetRow>, StoreError> {
        row_of(&self.conn, id)
    }

    /// Ventende forslag på aktive bilder, i stabil rekkefølge (asset-id, forslags-id).
    pub fn pending_suggestions(&self) -> Result<Vec<PendingSuggestion>, StoreError> {
        let mut st = self.conn.prepare_cached(
            "SELECT s.asset_id, s.kind, s.reason_code, s.grp_id FROM suggestion s \
             JOIN asset a ON a.id = s.asset_id WHERE s.status = 'pending' AND a.state = 0 \
             ORDER BY s.asset_id, s.id",
        )?;
        let rows = st.query_map([], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, Option<i64>>(3)?,
            ))
        })?;
        let mut out = Vec::new();
        for row in rows {
            let (a, k, reason_code, g) = row?;
            if let Some(kind) = kind_from(&k) {
                out.push(PendingSuggestion {
                    asset_id: AssetId(a as u64),
                    kind,
                    reason_code,
                    group: g.map(|g| GroupId(g as u64)),
                });
            }
        }
        Ok(out)
    }

    /// Aktive medlemmer i gruppen, beste først (keeper først, deretter `rank`).
    pub fn group_active_members(&self, g: GroupId) -> Result<Vec<AssetId>, StoreError> {
        let mut st = self.conn.prepare_cached(
            "SELECT m.asset_id FROM grp_member m JOIN asset a ON a.id = m.asset_id \
             JOIN grp ON grp.id = m.grp_id WHERE m.grp_id = ?1 AND a.state = 0 \
             ORDER BY (m.asset_id = grp.keeper_id) DESC, m.rank, m.asset_id",
        )?;
        let rows = st.query_map([g.0 as i64], |r| Ok(AssetId(r.get::<_, i64>(0)? as u64)))?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// (antall, byte, utløpte) for `trashed` ved tid `now`.
    pub fn trash_totals(&self, now: i64) -> Result<(u64, u64, u64), StoreError> {
        let r = self.conn.query_row(
            "SELECT COUNT(*), COALESCE(SUM(bytes), 0), COALESCE(SUM(expires_at <= ?1), 0) \
             FROM asset WHERE state = 1",
            [now],
            |r| {
                Ok((
                    r.get::<_, i64>(0)? as u64,
                    r.get::<_, i64>(1)? as u64,
                    r.get::<_, i64>(2)? as u64,
                ))
            },
        )?;
        Ok(r)
    }

    /// Byte frigjort av Fjell: bilder purget av brukeren via `finish_purge` (ikke `external`).
    pub fn freed_bytes(&self) -> Result<u64, StoreError> {
        Ok(self.conn.query_row(
            "SELECT COALESCE(SUM(a.bytes), 0) FROM action_log l JOIN asset a ON a.id = l.asset_id \
             WHERE l.op = 'purge' AND l.cause = 'user'",
            [],
            |r| r.get::<_, i64>(0),
        )? as u64)
    }

    pub fn setting(&self, key: &str) -> Result<Option<String>, StoreError> {
        Ok(self
            .conn
            .query_row("SELECT value FROM setting WHERE key = ?1", [key], |r| {
                r.get(0)
            })
            .optional()?)
    }

    pub fn set_setting(&mut self, key: &str, value: &str) -> Result<(), StoreError> {
        self.conn.execute(
            "INSERT INTO setting(key, value) VALUES (?1, ?2) ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            [key, value],
        )?;
        Ok(())
    }
}
