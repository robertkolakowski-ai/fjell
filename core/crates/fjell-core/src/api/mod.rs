//! Modul `api` (L4): eneste offentlige flate og UniFFI-grensen. Se docs/mvp/ARKITEKTUR.md del 2.
//!
//! Avhenger av: pipeline, actions, store, model.
//! Grensesnitt: trait `FjellApi` (synkron, ingen async). Undermoduler: `types` (lesemodeller,
//! feil), `mock` (`MemApi`, mock for UI) og `fjell`/`fjell_ops` (`Fjell`: ekte implementasjon som
//! kobler store, actions, pipeline og algoritmene; skanning kjører i egen tråd, `Fjell::wait_scan`).
//! - `start_scan`, `cancel`, `cards`, `decide_many` (én batch), `set_keeper`,
//!   `begin_purge` → `confirm_purge` → `finish_purge`, `undo`, `home_summary`, `trash_summary`, `settings`.
//!
//! `Confirmed`/`PurgeTicket` lages bare her (`pub(crate)`-konstruktører i `model`).

mod fjell;
mod fjell_ops;
mod fjell_rules;
#[cfg(test)]
mod fjell_tests;
#[cfg(test)]
mod hardening_tests;
pub mod mock;
#[cfg(test)]
mod rules_tests;
pub mod types;

pub use fjell::Fjell;
pub use fjell_rules::KeeperChange;
pub use mock::MemApi;
pub use types::*;

use crate::model::{
    AssetId, BatchId, CardId, ConsequenceCode, DeleteOutcome, GroupId, PurgeTicket, SuggestionKind,
};

/// Kontrakt som UI (via UniFFI), CLI og tester bygger på. Implementeres av ekte kjerne og `MemApi`.
pub trait FjellApi: Send + Sync {
    fn start_scan(&self, config: ScanConfig) -> Result<(), ApiError>;
    fn cancel(&self);
    fn cards(
        &self,
        kind: SuggestionKind,
        limit: usize,
        cursor: Option<CardCursor>,
    ) -> Result<CardPage, ApiError>;
    /// Alle beslutninger i én transaksjon og én batch (ett angre).
    fn decide_many(&self, decisions: Vec<(CardId, Decision)>) -> Result<BatchId, ApiError>;
    /// Som `decide_many`, men brukeren har bekreftet konsekvensene i `confirmed` (koder fra en tidligere
    /// `AckRequired`). Bekreftelsen gjelder bare disse kodene; mangler noen, svares `AckRequired` på nytt.
    /// Standard: samme som `decide_many` (mocken krever aldri bekreftelse).
    fn decide_many_confirmed(
        &self,
        decisions: Vec<(CardId, Decision)>,
        confirmed: Vec<ConsequenceCode>,
    ) -> Result<BatchId, ApiError> {
        let _ = confirmed;
        self.decide_many(decisions)
    }
    fn set_keeper(&self, group: GroupId, keeper: AssetId) -> Result<(), ApiError>;
    fn undo(&self) -> Result<UndoReport, ApiError>;
    fn begin_purge(&self, ids: Vec<AssetId>) -> Result<PurgeIntent, ApiError>;
    fn confirm_purge(&self, intent: &PurgeIntent) -> Result<PurgeTicket, ApiError>;
    /// Som `confirm_purge`, men med bekreftelse av koder fra `AckRequired` (massesletting).
    fn confirm_purge_confirmed(
        &self,
        intent: &PurgeIntent,
        confirmed: Vec<ConsequenceCode>,
    ) -> Result<PurgeTicket, ApiError> {
        let _ = confirmed;
        self.confirm_purge(intent)
    }
    fn finish_purge(
        &self,
        ticket: PurgeTicket,
        results: Vec<(AssetId, DeleteOutcome)>,
    ) -> Result<PurgeReport, ApiError>;
    fn home_summary(&self) -> HomeSummary;
    fn trash_summary(&self) -> TrashSummary;
    fn settings(&self) -> Settings;
    fn set_settings(&self, settings: Settings);
}
