//! Modul `actions` (L2): trygg sletting. Se docs/mvp/PLAN-TRYGGHET.md og ARKITEKTUR del 2.
//!
//! Avhenger av: store, model. Eier `Store` og en injisert `Clock`; ingen plattformkall (I1).
//! Grensesnitt (hovedpunkter):
//! - `plan(&PlanRequest)` / `plan_pending()`: ren forhåndsvisning med `ConsequenceSummary` og digest (I6).
//! - `commit(&Plan, &Acks)`: én batch, krever ack for beskyttede og massesletting (I7, I12).
//! - `restore`, `keep`, `undo`: alt som batch; `undo` hopper over avvikende rader (T12, T14).
//! - `expire_due`: varsler én gang per element, endrer aldri `state` (I9, I10).
//! - `begin_purge` -> `confirm_purge` -> `check_ticket` -> `finish_purge`: eneste vei til `purged`
//!   (I2); billetten brukes én gang og gjelder i 10 min. Selve plattformsletting ligger utenfor.
//!
//! Ingen tekst: feil og konsekvenser er koder (`ActionError::key`, `ConsequenceCode::key`).

mod commit;
mod digest;
mod expire;
mod plan;
mod purge;
#[cfg(test)]
mod tests;

use crate::model::{AssetId, BatchId, Clock, ConsequenceCode, ReasonCode, SuggestionKind};
use crate::store::{Store, StoreError};
use std::collections::HashSet;
use std::sync::Arc;

pub use commit::UndoOutcome;
pub use expire::TrashSummary;
pub use purge::PurgeOutcome;

// Brukes av `api` (M4) og testene; ikke offentlig fordi bare kjernen lager acks.
#[allow(unused_imports)]
pub(crate) use plan::acks_covering;

/// Startverdier som kalibreres (PLAN-TRYGGHET del 4.3 og 9).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Config {
    /// En plan eldre enn dette er `PlanStale`.
    pub plan_ttl_secs: i64,
    /// En `PurgeTicket` eldre enn dette er `PurgeStale`.
    pub ticket_ttl_secs: i64,
    /// `MassDelete` fra dette antallet bilder ...
    pub mass_count: u64,
    /// ... eller fra denne størrelsen.
    pub mass_bytes: u64,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            plan_ttl_secs: 5 * 60,
            ticket_ttl_secs: 10 * 60,
            mass_count: 50,
            mass_bytes: 1 << 30,
        }
    }
}

/// Ett valgt bilde i en `PlanRequest`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Selection {
    pub id: AssetId,
    /// Forslaget valget kommer fra (None = manuelt).
    pub kind: Option<SuggestionKind>,
    /// Hvis None og `kind` er satt, hentes begrunnelsen fra forslaget.
    pub reason: Option<ReasonCode>,
}

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct PlanRequest {
    pub items: Vec<Selection>,
}

impl PlanRequest {
    /// Manuelt valg (swipe, «velg alle»).
    pub fn manual(ids: &[AssetId]) -> PlanRequest {
        PlanRequest {
            items: ids
                .iter()
                .map(|&id| Selection {
                    id,
                    kind: None,
                    reason: None,
                })
                .collect(),
        }
    }
}

/// Feil fra `actions`. Koder uten tekst; `key()` gir stabil i18n-nøkkel (`error.<key>`).
#[derive(Debug)]
pub enum ActionError {
    /// Mangler ack for disse konsekvensene.
    AckRequired(Vec<ConsequenceCode>),
    /// Planen er for gammel eller mengden er endret siden `plan`.
    PlanStale,
    /// Billett/intent er brukt, utløpt, ukjent eller mengden er endret.
    PurgeStale,
    NothingToUndo,
    NotUndoable,
    AlreadyPurged(AssetId),
    UnknownAsset(AssetId),
    /// `begin_purge` krever `trashed`.
    NotTrashed(AssetId),
    /// Planen ville fjernet alle aktive medlemmer i gruppen (I8).
    WouldEmptyGroup(crate::model::GroupId),
    EmptySelection,
    Store(StoreError),
}

impl ActionError {
    pub fn key(&self) -> &'static str {
        match self {
            ActionError::AckRequired(_) => "ack_required",
            ActionError::PlanStale => "plan_stale",
            ActionError::PurgeStale => "purge_stale",
            ActionError::NothingToUndo => "nothing_to_undo",
            ActionError::NotUndoable => "not_undoable",
            ActionError::AlreadyPurged(_) => "already_purged",
            ActionError::UnknownAsset(_) => "unknown_asset",
            ActionError::NotTrashed(_) => "not_trashed",
            ActionError::WouldEmptyGroup(_) => "would_empty_group",
            ActionError::EmptySelection => "empty_selection",
            ActionError::Store(_) => "internal",
        }
    }
}

impl From<StoreError> for ActionError {
    fn from(e: StoreError) -> Self {
        match e {
            // En rad var ikke som planen la til grunn: samme svar som ved digest-brudd.
            StoreError::Conflict(_) => ActionError::PlanStale,
            other => ActionError::Store(other),
        }
    }
}

impl std::fmt::Display for ActionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for ActionError {}

pub type Result<T> = std::result::Result<T, ActionError>;

/// Inngang til trygg sletting. Én skrivetråd: metoder som skriver tar `&mut self`.
pub struct Actions {
    store: Store,
    clock: Arc<dyn Clock>,
    cfg: Config,
    /// Utstedte, ikke brukte billetter (digest, utstedt-tid). Bare i minnet (PLAN-TRYGGHET 2.2).
    tickets: HashSet<(u64, i64)>,
}

impl Actions {
    pub fn new(store: Store, clock: Arc<dyn Clock>) -> Actions {
        Actions::with_config(store, clock, Config::default())
    }

    pub fn with_config(store: Store, clock: Arc<dyn Clock>, cfg: Config) -> Actions {
        Actions {
            store,
            clock,
            cfg,
            tickets: HashSet::new(),
        }
    }

    pub fn store(&self) -> &Store {
        &self.store
    }

    pub fn store_mut(&mut self) -> &mut Store {
        &mut self.store
    }

    pub fn config(&self) -> Config {
        self.cfg
    }

    /// Papirkurvfrist i dager (setting `trash_days`, standard 30). Gjelder bare nye elementer.
    pub fn trash_days(&self) -> Result<u32> {
        let v = self.store.setting("trash_days")?;
        Ok(v.and_then(|s| s.parse().ok())
            .filter(|d| *d >= 1)
            .unwrap_or(30))
    }

    pub fn set_trash_days(&mut self, days: u32) -> Result<()> {
        Ok(self
            .store
            .set_setting("trash_days", &days.max(1).to_string())?)
    }

    pub(crate) fn trash_secs(&self) -> Result<i64> {
        Ok(self.trash_days()? as i64 * 86_400)
    }
}

/// Unik, sortert, deduplisert id-liste (T19).
pub(crate) fn dedupe(ids: &[AssetId]) -> Vec<AssetId> {
    let mut v: Vec<AssetId> = ids
        .iter()
        .copied()
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();
    v.sort();
    v
}

/// For `commit`/`restore`/`keep`: `None` hvis ingenting ble endret (tom batch finnes ikke).
pub type MaybeBatch = Option<BatchId>;
