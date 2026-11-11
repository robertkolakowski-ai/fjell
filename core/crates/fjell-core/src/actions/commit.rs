//! `commit`, `restore`, `keep` og `undo` (PLAN-TRYGGHET 2.1, 3 og 4.2). Del av `actions`.
//!
//! Grensesnitt: `Actions::commit(&Plan, &Acks) -> Option<BatchId>`, `restore(ids)`, `keep(ids)`,
//! `undo() -> UndoOutcome`. Hver er én transaksjon; tom mengde gir ingen batch (T19).
//! `commit` bruker aldri planens egne tall: tilstand, flagg og bytes leses på nytt og må gi samme
//! digest (I12). Beskyttede bilder trengs ack per kategori (I7); massesletting trenger `MassAck`.

use super::{dedupe, ActionError, Actions, MaybeBatch, Result};
use crate::model::plan::ACK_CATEGORIES;
use crate::model::{Acks, AssetId, BatchId, ConsequenceCode, Plan, State};
use crate::store::{BatchSpec, Cause, RowOp, UndoResult};

/// Resultat av `undo`. `skipped` er id-er der tilstanden avvek fra det batchen la igjen.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UndoOutcome {
    pub undone: BatchId,
    pub inverse: BatchId,
    pub restored: u64,
    pub skipped: Vec<AssetId>,
}

impl Actions {
    pub fn commit(&mut self, plan: &Plan, acks: &Acks) -> Result<MaybeBatch> {
        if plan.items.is_empty() {
            return Ok(None);
        }
        let now = self.clock.now();
        if now - plan.created_at > self.cfg.plan_ttl_secs {
            return Err(ActionError::PlanStale);
        }
        let ids = plan.ids();
        if dedupe(&ids).len() != ids.len() {
            return Err(ActionError::PlanStale);
        }
        let snap = self.snapshot(&ids).map_err(|e| match e {
            ActionError::UnknownAsset(_) => ActionError::PlanStale,
            e => e,
        })?;
        let acks_stale = acks.protected.iter().any(|a| a.digest() != plan.digest)
            || acks
                .mass
                .as_ref()
                .is_some_and(|m| m.digest() != plan.digest);
        if snap.digest != plan.digest || acks_stale {
            return Err(ActionError::PlanStale);
        }
        // Digesten dekker tilstand og flagg, så det er nok å se at ingen rad ikke er aktiv.
        if snap.rows.iter().any(|r| r.state != State::Active) {
            return Err(ActionError::PlanStale);
        }
        let mut missing = Vec::new();
        for (flag, code) in ACK_CATEGORIES {
            let need: Vec<AssetId> = snap
                .rows
                .iter()
                .filter(|r| r.protect.has(flag))
                .map(|r| r.id)
                .collect();
            let covered = need.is_empty()
                || acks
                    .protected
                    .iter()
                    .any(|a| a.category() == flag && need.iter().all(|id| a.ids().contains(id)));
            if !covered {
                missing.push(code);
            }
        }
        if self.is_mass(snap.rows.len() as u64, snap.bytes) && acks.mass.is_none() {
            missing.push(ConsequenceCode::MassDelete);
        }
        if !missing.is_empty() {
            return Err(ActionError::AckRequired(missing));
        }
        let expires_at = now + self.trash_secs()?;
        let ops = plan
            .items
            .iter()
            .map(|i| RowOp::Trash {
                id: i.id,
                kind: i.kind,
                expires_at,
            })
            .collect();
        Ok(self.store.apply_batch(&BatchSpec {
            ts: now,
            cause: Cause::User,
            ops,
        })?)
    }

    /// Gjenoppretter `trashed` til `active` (én batch). `active` hoppes over uten loggrad (I11).
    pub fn restore(&mut self, ids: &[AssetId]) -> Result<MaybeBatch> {
        let mut ops = Vec::new();
        for row in self.snapshot(&dedupe(ids))?.rows {
            match row.state {
                State::Purged => return Err(ActionError::AlreadyPurged(row.id)),
                State::Trashed => ops.push(RowOp::Restore { id: row.id }),
                State::Active => {}
            }
        }
        self.apply(ops)
    }

    /// «Behold»: avviser ventende forslag for bildene. Ingen tilstandsendring.
    pub fn keep(&mut self, ids: &[AssetId]) -> Result<MaybeBatch> {
        let ids = dedupe(ids);
        self.snapshot(&ids)?; // ukjent id gir `UnknownAsset`
        let ops = ids.into_iter().map(|id| RowOp::Keep { id }).collect();
        self.apply(ops)
    }

    /// Inverterer siste ikke-angrede brukerbatch (`NotUndoable` hvis den inneholder purge).
    pub fn undo(&mut self) -> Result<UndoOutcome> {
        let (now, secs) = (self.clock.now(), self.trash_secs()?);
        match self.store.undo_top(now, secs)? {
            UndoResult::Nothing => Err(ActionError::NothingToUndo),
            UndoResult::NotUndoable => Err(ActionError::NotUndoable),
            UndoResult::Done {
                undone,
                inverse,
                applied,
                skipped,
            } => Ok(UndoOutcome {
                undone,
                inverse,
                restored: applied,
                skipped,
            }),
        }
    }

    fn apply(&mut self, ops: Vec<RowOp>) -> Result<MaybeBatch> {
        if ops.is_empty() {
            return Ok(None);
        }
        let spec = BatchSpec {
            ts: self.clock.now(),
            cause: Cause::User,
            ops,
        };
        Ok(self.store.apply_batch(&spec)?)
    }
}
