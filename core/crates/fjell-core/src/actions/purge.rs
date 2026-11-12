//! Purge-flyt: `begin_purge` -> `confirm_purge` -> `finish_purge` (PLAN-TRYGGHET 2.2 og 4.3).
//! Del av `actions`.
//!
//! Grensesnitt: `begin_purge(ids) -> PurgeOffer`, `confirm_purge(&offer, Option<&MassAck>) -> PurgeTicket`,
//! `check_ticket(&ticket)` (UI kaller rett før plattformens `delete`), `finish_purge(ticket, results)`.
//! Eneste vei til `purged` er `finish_purge` med `Deleted` (I2). Billetten brukes én gang, gjelder
//! 10 min og er bundet til mengden via digest (I12). Id-er uten `Deleted` forblir `trashed` (I15).

use super::{dedupe, ActionError, Actions, Result};
use crate::model::{
    AssetId, Confirmed, ConsequenceCode, DeleteOutcome, MassAck, PurgeOffer, PurgeTicket, State,
};
use crate::store::{BatchSpec, Cause, RowOp};
use std::collections::HashMap;

/// Resultat av `finish_purge`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PurgeOutcome {
    pub purged: Vec<AssetId>,
    /// Forblir `trashed` (avbrutt, feilet eller uten svar): ingen tap.
    pub kept_trashed: Vec<AssetId>,
    pub freed_bytes: u64,
}

impl Actions {
    /// Forklaringsgrunnlag før systemdialogen. Bare `trashed` kan tømmes. Skriver ingenting.
    pub fn begin_purge(&self, ids: &[AssetId]) -> Result<PurgeOffer> {
        let ids = dedupe(ids);
        if ids.is_empty() {
            return Err(ActionError::EmptySelection);
        }
        let snap = self.snapshot(&ids)?;
        let now = self.clock.now();
        let mut expired_count = 0;
        for r in &snap.rows {
            match r.state {
                State::Purged => return Err(ActionError::AlreadyPurged(r.id)),
                State::Active => return Err(ActionError::NotTrashed(r.id)),
                State::Trashed => expired_count += r.expires_at.is_some_and(|e| e <= now) as u64,
            }
        }
        let mut codes = vec![ConsequenceCode::PlatformDialogNext];
        if self.is_mass(ids.len() as u64, snap.bytes) {
            codes.push(ConsequenceCode::MassDelete);
        }
        if expired_count > 0 {
            codes.push(ConsequenceCode::ExpiredNotice);
        }
        Ok(PurgeOffer {
            ids,
            bytes: snap.bytes,
            expired_count,
            digest: snap.digest,
            codes,
        })
    }

    /// Brukerens eksplisitte bekreftelse. Mengden må være uendret siden `begin_purge`, og
    /// massesletting krever `MassAck` for samme digest.
    pub fn confirm_purge(
        &mut self,
        offer: &PurgeOffer,
        mass: Option<&MassAck>,
    ) -> Result<PurgeTicket> {
        let snap = self.snapshot(&dedupe(&offer.ids)).map_err(|e| match e {
            ActionError::UnknownAsset(_) => ActionError::PurgeStale,
            e => e,
        })?;
        if snap.digest != offer.digest
            || snap.rows.len() != offer.ids.len()
            || snap.rows.iter().any(|r| r.state != State::Trashed)
        {
            return Err(ActionError::PurgeStale);
        }
        if mass.is_some_and(|m| m.digest() != offer.digest) {
            return Err(ActionError::PurgeStale);
        }
        if self.is_mass(offer.ids.len() as u64, snap.bytes) && mass.is_none() {
            return Err(ActionError::AckRequired(vec![ConsequenceCode::MassDelete]));
        }
        let now = self.clock.now();
        self.tickets.insert((offer.digest, now));
        Ok(PurgeTicket::new(
            Confirmed::new(offer.digest),
            offer.ids.clone(),
            now,
        ))
    }

    /// Er billetten ukjent/brukt, utløpt eller mengden endret? Kalles før plattformens `delete`
    /// slik at en ugyldig billett aldri når plattformen (T16). Forbruker ikke billetten.
    pub fn check_ticket(&self, ticket: &PurgeTicket) -> Result<()> {
        let key = (ticket.confirmed().digest(), ticket.issued_at());
        if !self.tickets.contains(&key)
            || self.clock.now() - ticket.issued_at() > self.cfg.ticket_ttl_secs
        {
            return Err(ActionError::PurgeStale);
        }
        let snap = self
            .snapshot(ticket.ids())
            .map_err(|_| ActionError::PurgeStale)?;
        if snap.digest != ticket.confirmed().digest()
            || snap.rows.iter().any(|r| r.state != State::Trashed)
        {
            return Err(ActionError::PurgeStale);
        }
        Ok(())
    }

    /// Oppdaterer indeksen etter plattformens svar (ikke selve slettingen). Forbruker billetten
    /// også ved feil. Resultater for id-er utenfor billetten ignoreres; siste svar per id gjelder.
    pub fn finish_purge(
        &mut self,
        ticket: PurgeTicket,
        results: &[(AssetId, DeleteOutcome)],
    ) -> Result<PurgeOutcome> {
        let checked = self.check_ticket(&ticket);
        self.tickets
            .remove(&(ticket.confirmed().digest(), ticket.issued_at()));
        checked?;
        let outcome: HashMap<AssetId, DeleteOutcome> = results.iter().copied().collect();
        let (mut purged, mut kept) = (Vec::new(), Vec::new());
        for &id in ticket.ids() {
            match outcome.get(&id) {
                Some(DeleteOutcome::Deleted) => purged.push(id),
                _ => kept.push(id),
            }
        }
        purged.sort();
        kept.sort();
        let freed_bytes = self.snapshot(&purged)?.bytes;
        let ops = purged.iter().map(|&id| RowOp::Purge { id }).collect();
        self.store.apply_batch(&BatchSpec {
            ts: self.clock.now(),
            cause: Cause::User,
            ops,
        })?;
        Ok(PurgeOutcome {
            purged,
            kept_trashed: kept,
            freed_bytes,
        })
    }
}
