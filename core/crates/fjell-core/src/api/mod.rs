//! Modul `api` (L4): eneste offentlige flate og UniFFI-grensen. Se docs/mvp/ARKITEKTUR.md del 2.
//!
//! Avhenger av: pipeline, actions, store, model (kontrakten bruker foreløpig bare `model`).
//! Grensesnitt: trait `FjellApi` (synkron, ingen async). Undermoduler: `types` (lesemodeller,
//! feil) og `mock` (`MemApi`, kjørbar mock for UI/CLI til ekte kjerne finnes).
//! - `start_scan`, `cancel`, `cards`, `decide_many` (én batch), `set_keeper`,
//!   `begin_purge` → `confirm_purge` → `finish_purge`, `undo`, `home_summary`, `trash_summary`, `settings`.
//!
//! `Confirmed`/`PurgeTicket` lages bare her (`pub(crate)`-konstruktører i `model`).

pub mod mock;
pub mod types;

pub use mock::MemApi;
pub use types::*;

use crate::model::{AssetId, BatchId, CardId, DeleteOutcome, GroupId, PurgeTicket, SuggestionKind};

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
    fn set_keeper(&self, group: GroupId, keeper: AssetId) -> Result<(), ApiError>;
    fn undo(&self) -> Result<UndoReport, ApiError>;
    fn begin_purge(&self, ids: Vec<AssetId>) -> Result<PurgeIntent, ApiError>;
    fn confirm_purge(&self, intent: &PurgeIntent) -> Result<PurgeTicket, ApiError>;
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
