//! Skrive- og lesespørringer for trygg sletting (PLAN-TRYGGHET del 2, 3 og 5). Del av `store` (L1).
//!
//! Avhenger av: `model` (typer) og `super::Store`. All logikk (hvem som får slette, ack, digest)
//! ligger i `actions`; her ligger bare SQL og transaksjonsgrenser.
//! Grensesnitt (skriving; lesing ligger i `ops_read`):
//! - `apply_batch(BatchSpec)`: alle rader i én transaksjon med nytt `batch_id`, alt eller ingenting (I4).
//! - `undo_top(ts, trash_secs)`: inverterer siste ikke-angrede brukerbatch (I5, I11, T13, T14).
//! - `reconcile_removed(removed, ts)`: borte kilder til `purged` med `cause=external` (beslutning 18).
//!
//! Skriving går per id med cachet setning og lesing av mengder via `json_each` (se `ops_read`),
//! så SQLites variabelgrense aldri nås (T18) og 50 000 id-er går i én transaksjon.

use super::ops_read::row_of;
use super::{Store, StoreError};
use crate::model::{AssetId, BatchId, SourceId, State, SuggestionKind};
use rusqlite::{params, OptionalExtension, Transaction};

/// Hvorfor en rad ble skrevet (`action_log.cause`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cause {
    User,
    Expiry,
    External,
    Undo,
}

impl Cause {
    pub fn as_str(self) -> &'static str {
        match self {
            Cause::User => "user",
            Cause::Expiry => "expiry",
            Cause::External => "external",
            Cause::Undo => "undo",
        }
    }
}

/// Én endring i en batch. Forutsetter tilstand (`Trash` krever active, `Restore`/`Purge` trashed).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RowOp {
    Trash {
        id: AssetId,
        /// Forslaget som aksepteres (None = manuelt valg).
        kind: Option<SuggestionKind>,
        expires_at: i64,
    },
    Restore {
        id: AssetId,
    },
    /// Avviser alle ventende forslag for bildet (ingen loggrad hvis det ikke finnes noen).
    Keep {
        id: AssetId,
    },
    Purge {
        id: AssetId,
    },
}

#[derive(Clone, Debug)]
pub struct BatchSpec {
    pub ts: i64,
    pub cause: Cause,
    pub ops: Vec<RowOp>,
}

/// Loggrad slik `undo_top` leser den: (id, op, asset, prev_state, forslags-id, prev_sugg_status).
type LogRow = (i64, String, i64, i64, Option<i64>, Option<String>);

/// Utfall av `undo_top`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UndoResult {
    Nothing,
    /// Øverste batch inneholder `purge`.
    NotUndoable,
    Done {
        undone: BatchId,
        inverse: BatchId,
        applied: u64,
        skipped: Vec<AssetId>,
    },
}

fn next_batch_id(conn: &rusqlite::Connection) -> Result<i64, StoreError> {
    Ok(conn.query_row(
        "SELECT COALESCE(MAX(batch_id), 0) + 1 FROM action_log",
        [],
        |r| r.get(0),
    )?)
}

/// Skriver én loggrad og returnerer dens id.
#[allow(clippy::too_many_arguments)]
fn log_row(
    tx: &Transaction,
    batch: i64,
    ts: i64,
    op: &str,
    id: AssetId,
    prev_state: State,
    sugg: Option<(i64, Option<String>)>,
    inverse_of: Option<i64>,
    cause: Cause,
) -> Result<i64, StoreError> {
    let (sid, prev) = match sugg {
        Some((s, p)) => (Some(s), p),
        None => (None, None),
    };
    let mut st = tx.prepare_cached(
        "INSERT INTO action_log(batch_id, ts, op, asset_id, prev_state, suggestion_id, inverse_of, \
         prev_sugg_status, cause) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)",
    )?;
    st.execute(params![
        batch,
        ts,
        op,
        id.0 as i64,
        prev_state.as_i64(),
        sid,
        inverse_of,
        prev,
        cause.as_str()
    ])?;
    Ok(tx.last_insert_rowid())
}

fn set_sugg_status(tx: &Transaction, sid: i64, status: &str) -> Result<(), StoreError> {
    tx.prepare_cached("UPDATE suggestion SET status = ?2 WHERE id = ?1")?
        .execute(params![sid, status])?;
    Ok(())
}

fn sugg_status(tx: &Transaction, sid: i64) -> Result<Option<String>, StoreError> {
    Ok(tx
        .prepare_cached("SELECT status FROM suggestion WHERE id = ?1")?
        .query_row([sid], |r| r.get(0))
        .optional()?)
}

impl Store {
    /// Utløpte, ikke varslede trashed-bilder; setter `expired_notified_at = now` i én transaksjon.
    /// Endrer aldri `state` (I9, I10). Returnerer (antall, byte).
    pub fn mark_expired_notified(&mut self, now: i64) -> Result<(u64, u64), StoreError> {
        let tx = self.conn.transaction()?;
        let r = tx.query_row(
            "SELECT COUNT(*), COALESCE(SUM(bytes), 0) FROM asset \
             WHERE state = 1 AND expires_at <= ?1 AND expired_notified_at IS NULL",
            [now],
            |r| Ok((r.get::<_, i64>(0)? as u64, r.get::<_, i64>(1)? as u64)),
        )?;
        if r.0 > 0 {
            tx.execute(
                "UPDATE asset SET expired_notified_at = ?1 \
                 WHERE state = 1 AND expires_at <= ?1 AND expired_notified_at IS NULL",
                [now],
            )?;
        }
        tx.commit()?;
        Ok(r)
    }

    /// Skriver hele batchen i én transaksjon. `None` hvis ingen rader ble skrevet.
    /// Ved feil (også midt i batchen) rulles alt tilbake (I4).
    pub fn apply_batch(&mut self, spec: &BatchSpec) -> Result<Option<BatchId>, StoreError> {
        let tx = self.conn.transaction()?;
        let batch = next_batch_id(&tx)?;
        let mut rows = 0u64;
        for op in &spec.ops {
            rows += apply_op(&tx, batch, spec, *op)?;
        }
        if rows == 0 {
            return Ok(None);
        }
        tx.commit()?;
        Ok(Some(BatchId(batch as u64)))
    }

    /// Inverterer siste ikke-angrede brukerbatch i én transaksjon. Rader der tilstanden ikke lenger
    /// er den batchen la igjen, hoppes over og rapporteres (T12, T14); de får en `keep`-markør
    /// slik at batchen regnes som angret.
    pub fn undo_top(&mut self, ts: i64, trash_secs: i64) -> Result<UndoResult, StoreError> {
        let tx = self.conn.transaction()?;
        let mut cursor = i64::MAX;
        let top = loop {
            let next: Option<i64> = tx
                .prepare_cached(
                    "SELECT MAX(batch_id) FROM action_log WHERE cause = 'user' AND batch_id < ?1",
                )?
                .query_row([cursor], |r| r.get(0))?;
            let Some(b) = next else {
                return Ok(UndoResult::Nothing);
            };
            let undone: bool = tx.query_row(
                "SELECT EXISTS(SELECT 1 FROM action_log i WHERE i.inverse_of IN \
                 (SELECT id FROM action_log WHERE batch_id = ?1))",
                [b],
                |r| r.get(0),
            )?;
            if !undone {
                break b;
            }
            cursor = b;
        };
        let rows: Vec<LogRow> = {
            let mut st = tx.prepare(
                "SELECT id, op, asset_id, prev_state, suggestion_id, prev_sugg_status \
                 FROM action_log WHERE batch_id = ?1 ORDER BY id",
            )?;
            let it = st.query_map([top], |r| {
                Ok((
                    r.get(0)?,
                    r.get(1)?,
                    r.get(2)?,
                    r.get(3)?,
                    r.get(4)?,
                    r.get(5)?,
                ))
            })?;
            it.collect::<Result<_, _>>()?
        };
        if rows.iter().any(|r| r.1 == "purge") {
            return Ok(UndoResult::NotUndoable);
        }
        let inv = next_batch_id(&tx)?;
        let (mut applied, mut skipped) = (0u64, Vec::new());
        for (lid, op, aid, prev_state, sid, prev_sugg) in rows {
            let id = AssetId(aid as u64);
            let cur = row_of(&tx, id)?.map(|r| r.state).unwrap_or(State::Purged);
            let sugg = |tx: &Transaction| -> Result<Option<(i64, Option<String>)>, StoreError> {
                match sid {
                    Some(s) => Ok(Some((s, sugg_status(tx, s)?))),
                    None => Ok(None),
                }
            };
            let ok = match (op.as_str(), cur) {
                ("trash", State::Trashed) => {
                    let s = sugg(&tx)?;
                    tx.prepare_cached("UPDATE asset SET state = ?2, trashed_at = NULL, expires_at = NULL WHERE id = ?1")?
                        .execute(params![aid, prev_state])?;
                    if let (Some(s), Some(p)) = (sid, prev_sugg.as_deref()) {
                        set_sugg_status(&tx, s, p)?;
                    }
                    log_row(&tx, inv, ts, "restore", id, cur, s, Some(lid), Cause::Undo)?;
                    true
                }
                ("restore", State::Active) => {
                    tx.prepare_cached("UPDATE asset SET state = 1, trashed_at = ?2, expires_at = ?3, expired_notified_at = NULL WHERE id = ?1")?
                        .execute(params![aid, ts, ts + trash_secs])?;
                    log_row(&tx, inv, ts, "trash", id, cur, None, Some(lid), Cause::Undo)?;
                    true
                }
                ("keep", _) => {
                    let s = sugg(&tx)?;
                    if let (Some(s), Some(p)) = (sid, prev_sugg.as_deref()) {
                        set_sugg_status(&tx, s, p)?;
                    }
                    log_row(&tx, inv, ts, "keep", id, cur, s, Some(lid), Cause::Undo)?;
                    true
                }
                _ => false,
            };
            if ok {
                applied += 1;
            } else {
                skipped.push(id);
                log_row(&tx, inv, ts, "keep", id, cur, None, Some(lid), Cause::Undo)?;
            }
        }
        tx.commit()?;
        Ok(UndoResult::Done {
            undone: BatchId(top as u64),
            inverse: BatchId(inv as u64),
            applied,
            skipped,
        })
    }

    /// Avstemmer borte kilder til `purged` (`cause=external`) i én batch. Ukjente og allerede purgede
    /// kilder hoppes over. Aktive bilder går via `trashed` inne i transaksjonen fordi triggeren
    /// (I2) blokkerer `active -> purged` direkte; intet logges for mellomtrinnet.
    pub fn reconcile_removed(
        &mut self,
        removed: &[SourceId],
        ts: i64,
    ) -> Result<Vec<AssetId>, StoreError> {
        let tx = self.conn.transaction()?;
        let batch = next_batch_id(&tx)?;
        let mut purged = Vec::new();
        for src in removed {
            let found: Option<i64> = tx
                .prepare_cached("SELECT id FROM asset WHERE source_id = ?1")?
                .query_row([&src.0], |r| r.get(0))
                .optional()?;
            let Some(aid) = found else { continue };
            let id = AssetId(aid as u64);
            let Some(row) = row_of(&tx, id)? else {
                continue;
            };
            if row.state == State::Purged {
                continue;
            }
            if row.state == State::Active {
                tx.execute("UPDATE asset SET state = 1 WHERE id = ?1", [aid])?;
            }
            tx.execute("UPDATE asset SET state = 2 WHERE id = ?1", [aid])?;
            log_row(
                &tx,
                batch,
                ts,
                "purge",
                id,
                row.state,
                None,
                None,
                Cause::External,
            )?;
            purged.push(id);
        }
        tx.commit()?;
        Ok(purged)
    }
}

/// Anvender én `RowOp`; returnerer antall loggrader skrevet.
fn apply_op(tx: &Transaction, batch: i64, spec: &BatchSpec, op: RowOp) -> Result<u64, StoreError> {
    let (ts, cause) = (spec.ts, spec.cause);
    match op {
        RowOp::Trash {
            id,
            kind,
            expires_at,
        } => {
            expect_state(tx, id, State::Active)?;
            let sugg = match kind {
                Some(k) => tx
                    .prepare_cached(
                        "SELECT id, status FROM suggestion WHERE asset_id = ?1 AND kind = ?2",
                    )?
                    .query_row(params![id.0 as i64, k.as_str()], |r| {
                        Ok((r.get::<_, i64>(0)?, Some(r.get::<_, String>(1)?)))
                    })
                    .optional()?,
                None => None,
            };
            tx.prepare_cached(
                "UPDATE asset SET state = 1, trashed_at = ?2, expires_at = ?3, expired_notified_at = NULL WHERE id = ?1",
            )?
            .execute(params![id.0 as i64, ts, expires_at])?;
            if let Some((sid, _)) = &sugg {
                set_sugg_status(tx, *sid, "accepted")?;
            }
            log_row(tx, batch, ts, "trash", id, State::Active, sugg, None, cause)?;
            Ok(1)
        }
        RowOp::Restore { id } => {
            expect_state(tx, id, State::Trashed)?;
            tx.prepare_cached(
                "UPDATE asset SET state = 0, trashed_at = NULL, expires_at = NULL WHERE id = ?1",
            )?
            .execute([id.0 as i64])?;
            log_row(
                tx,
                batch,
                ts,
                "restore",
                id,
                State::Trashed,
                None,
                None,
                cause,
            )?;
            Ok(1)
        }
        RowOp::Purge { id } => {
            expect_state(tx, id, State::Trashed)?;
            tx.prepare_cached("UPDATE asset SET state = 2 WHERE id = ?1")?
                .execute([id.0 as i64])?;
            log_row(
                tx,
                batch,
                ts,
                "purge",
                id,
                State::Trashed,
                None,
                None,
                cause,
            )?;
            Ok(1)
        }
        RowOp::Keep { id } => {
            let state = row_of(tx, id)?
                .map(|r| r.state)
                .ok_or(StoreError::Conflict(id))?;
            let pending: Vec<(i64, String)> = {
                let mut st = tx.prepare_cached("SELECT id, status FROM suggestion WHERE asset_id = ?1 AND status = 'pending' ORDER BY id")?;
                let it = st.query_map([id.0 as i64], |r| Ok((r.get(0)?, r.get(1)?)))?;
                it.collect::<Result<_, _>>()?
            };
            for (sid, status) in &pending {
                set_sugg_status(tx, *sid, "rejected")?;
                log_row(
                    tx,
                    batch,
                    ts,
                    "keep",
                    id,
                    state,
                    Some((*sid, Some(status.clone()))),
                    None,
                    cause,
                )?;
            }
            Ok(pending.len() as u64)
        }
    }
}

fn expect_state(tx: &Transaction, id: AssetId, want: State) -> Result<(), StoreError> {
    match row_of(tx, id)? {
        Some(r) if r.state == want => Ok(()),
        _ => Err(StoreError::Conflict(id)),
    }
}
