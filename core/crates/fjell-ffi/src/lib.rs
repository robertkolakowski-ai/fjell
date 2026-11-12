//! UniFFI-omslag rundt `fjell_core::api` (ingen logikk; bare typekonvertering og delegering).
//!
//! Avhenger av: `fjell_core` (kun `api` og `model`). Grensesnitt (`Fjell`-objektet) speiler `FjellApi`:
//! `start_scan`, `cancel`, `cards`, `decide_many(_confirmed)`, `set_keeper`, `undo`, `begin_purge`,
//! `confirm_purge(_confirmed)` → `PurgeTicket` → `finish_purge`, `home_summary`, `trash_summary`,
//! `settings`, samt `wait_scan` og `sources_for` (til plattformens egen sletting).
//! `PurgeTicket` er et ugjennomsiktig objekt: Swift kan ikke lage eller endre det (beslutning 17).
//! Plattformen leverer `AssetSourceCallback`; tiden er systemklokken.
//! Bindinger genereres med `just bindings` og sjekkes ikke inn.

mod source;
mod types;

pub use source::*;
pub use types::*;

use fjell_core::api::{self as api, FjellApi};
use fjell_core::model as m;
use fjell_core::store::Store;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

uniffi::setup_scaffolding!();

/// Versjon på omslaget (samme som kjernen).
pub const VERSION: &str = fjell_core::VERSION;

/// Versjonsstreng til UI (for diagnostikk).
#[uniffi::export]
pub fn core_version() -> String {
    VERSION.to_string()
}

struct SystemClock;

impl m::Clock for SystemClock {
    fn now(&self) -> i64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0)
    }
}

/// Engangsbillett fra `confirm_purge`. Gis uendret tilbake til `finish_purge`.
#[derive(uniffi::Object)]
pub struct PurgeTicket(m::PurgeTicket);

/// Kjernen sett fra plattformen.
#[derive(uniffi::Object)]
pub struct Fjell {
    inner: api::Fjell,
}

fn ids(v: Vec<u64>) -> Vec<m::AssetId> {
    v.into_iter().map(m::AssetId).collect()
}

fn decisions(v: Vec<CardDecision>) -> Vec<(m::CardId, api::Decision)> {
    v.into_iter()
        .map(|d| (m::CardId(d.card), d.decision.into()))
        .collect()
}

fn codes(v: Vec<ConsequenceCode>) -> Vec<m::ConsequenceCode> {
    v.into_iter().map(Into::into).collect()
}

#[uniffi::export]
impl Fjell {
    /// Åpner (eller oppretter) indeksen på `db_path` og kobler til plattformens kilde.
    #[uniffi::constructor]
    pub fn open(
        db_path: String,
        source: Arc<dyn AssetSourceCallback>,
    ) -> Result<Arc<Fjell>, FjellError> {
        let store =
            Store::open(std::path::Path::new(&db_path)).map_err(|e| FjellError::Internal {
                detail: e.to_string(),
            })?;
        let src = Arc::new(source::CallbackSource(source));
        let inner = api::Fjell::new(store, src, Arc::new(SystemClock));
        Ok(Arc::new(Fjell { inner }))
    }

    pub fn start_scan(&self, config: ScanConfig) -> Result<(), FjellError> {
        Ok(self.inner.start_scan(config.into())?)
    }

    pub fn cancel(&self) {
        self.inner.cancel()
    }

    /// Blokkerer til pågående skanning er ferdig (ikke kall fra UI-tråden).
    pub fn wait_scan(&self) -> Result<(), FjellError> {
        match self.inner.wait_scan() {
            Some(Err(e)) => Err(e.into()),
            _ => Ok(()),
        }
    }

    pub fn cards(
        &self,
        kind: SuggestionKind,
        limit: u64,
        cursor: Option<u64>,
    ) -> Result<CardPage, FjellError> {
        let cur = cursor.map(api::CardCursor);
        Ok(self.inner.cards(kind.into(), limit as usize, cur)?.into())
    }

    pub fn decide_many(&self, decisions_in: Vec<CardDecision>) -> Result<u64, FjellError> {
        Ok(self.inner.decide_many(decisions(decisions_in))?.0)
    }

    pub fn decide_many_confirmed(
        &self,
        decisions_in: Vec<CardDecision>,
        confirmed: Vec<ConsequenceCode>,
    ) -> Result<u64, FjellError> {
        let r = self
            .inner
            .decide_many_confirmed(decisions(decisions_in), codes(confirmed))?;
        Ok(r.0)
    }

    pub fn set_keeper(&self, group: u64, keeper: u64) -> Result<(), FjellError> {
        Ok(self
            .inner
            .set_keeper(m::GroupId(group), m::AssetId(keeper))?)
    }

    pub fn undo(&self) -> Result<UndoReport, FjellError> {
        let r = self.inner.undo()?;
        Ok(UndoReport {
            batch: r.batch.0,
            restored: r.restored,
        })
    }

    pub fn begin_purge(&self, asset_ids: Vec<u64>) -> Result<PurgeIntent, FjellError> {
        Ok(self.inner.begin_purge(ids(asset_ids))?.into())
    }

    pub fn confirm_purge(&self, intent: PurgeIntent) -> Result<Arc<PurgeTicket>, FjellError> {
        let t = self.inner.confirm_purge(&intent.into())?;
        Ok(Arc::new(PurgeTicket(t)))
    }

    pub fn confirm_purge_confirmed(
        &self,
        intent: PurgeIntent,
        confirmed: Vec<ConsequenceCode>,
    ) -> Result<Arc<PurgeTicket>, FjellError> {
        let t = self
            .inner
            .confirm_purge_confirmed(&intent.into(), codes(confirmed))?;
        Ok(Arc::new(PurgeTicket(t)))
    }

    /// Plattform-ID for billettens asset, til plattformens egen sletting.
    pub fn ticket_sources(&self, ticket: Arc<PurgeTicket>) -> Vec<SourceRef> {
        self.sources_for(ticket.0.ids().iter().map(|i| i.0).collect())
    }

    pub fn finish_purge(
        &self,
        ticket: Arc<PurgeTicket>,
        results: Vec<DeleteResult>,
    ) -> Result<PurgeReport, FjellError> {
        let rs = results
            .into_iter()
            .map(|r| (m::AssetId(r.asset), r.outcome.into()))
            .collect();
        let r = self.inner.finish_purge(ticket.0.clone(), rs)?;
        Ok(PurgeReport {
            purged: r.purged.into_iter().map(|i| i.0).collect(),
            kept_trashed: r.kept_trashed.into_iter().map(|i| i.0).collect(),
        })
    }

    pub fn sources_for(&self, asset_ids: Vec<u64>) -> Vec<SourceRef> {
        self.inner
            .sources_for(&ids(asset_ids))
            .into_iter()
            .map(|(a, s)| SourceRef {
                asset: a.0,
                source_id: s.0,
            })
            .collect()
    }

    pub fn home_summary(&self) -> HomeSummary {
        self.inner.home_summary().into()
    }

    pub fn trash_summary(&self) -> TrashSummary {
        self.inner.trash_summary().into()
    }

    pub fn settings(&self) -> Settings {
        Settings {
            trash_days: self.inner.settings().trash_days,
        }
    }

    pub fn set_settings(&self, settings: Settings) {
        self.inner.set_settings(api::Settings {
            trash_days: settings.trash_days,
        })
    }
}
