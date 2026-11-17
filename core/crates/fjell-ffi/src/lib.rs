//! UniFFI-omslag rundt `fjell_core::api` (ingen logikk; bare typekonvertering og delegering).
//!
//! Avhenger av: `fjell_core` (kun `api` og `model`). Grensesnitt (`Fjell`-objektet) speiler `FjellApi`:
//! `start_scan`, `cancel`, `cards`, `decide_many(_confirmed)`, `set_keeper`, `undo`, `begin_purge`,
//! `confirm_purge(_confirmed)` → `PurgeTicket` → `finish_purge`, `home_summary`, `trash_summary`,
//! `settings`, hele V1-flaten (`protect_*`, `rule*`, `person*`, `face_index_*`, `dashboard(_full)`,
//! `series_compare`, `thresholds`), samt `wait_scan` og `sources_for` (til plattformens egen sletting).
//! `MIRRORED_API`/`NOT_MIRRORED_API` er fasit for paritetstesten mot `FjellApi` (docs/v1/FFI.md).
//! `PurgeTicket` er et ugjennomsiktig objekt: Swift kan ikke lage eller endre det (beslutning 17).
//! Plattformen leverer `AssetSourceCallback`; tiden er systemklokken.
//! Bindinger genereres med `just bindings` og sjekkes ikke inn.

mod source;
mod types;
mod v1_types;

pub use source::*;
pub use types::*;
pub use v1_types::*;

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
        self.inner.settings().into()
    }

    /// Typer som mangler i `settings.sensitivity` beholder gjeldende verdi.
    pub fn set_settings(&self, settings: Settings) {
        let merged = settings.onto(self.inner.settings());
        self.inner.set_settings(merged)
    }

    // ---- V1 ----

    pub fn protect_set(&self, scope: ProtectScope) -> Result<(), FjellError> {
        Ok(self.inner.protect_set(scope.into())?)
    }

    pub fn unprotect(&self, scope: ProtectScope) -> Result<(), FjellError> {
        Ok(self.inner.unprotect(scope.into())?)
    }

    pub fn protections(&self) -> Result<Vec<ProtectionEntry>, FjellError> {
        Ok(self
            .inner
            .protections()?
            .into_iter()
            .map(Into::into)
            .collect())
    }

    pub fn rules_list(&self) -> Result<Vec<RuleView>, FjellError> {
        Ok(self
            .inner
            .rules_list()?
            .into_iter()
            .map(Into::into)
            .collect())
    }

    pub fn rule_save(&self, draft: RuleDraft) -> Result<u64, FjellError> {
        Ok(self.inner.rule_save(draft.into())?.0)
    }

    pub fn rule_preview(&self, spec: RuleSpec) -> Result<RulePreview, FjellError> {
        Ok(self.inner.rule_preview(spec.into())?.into())
    }

    pub fn rule_toggle(&self, id: u64, enabled: bool) -> Result<(), FjellError> {
        Ok(self.inner.rule_toggle(m::RuleId(id), enabled)?)
    }

    pub fn rule_delete(&self, id: u64) -> Result<(), FjellError> {
        Ok(self.inner.rule_delete(m::RuleId(id))?)
    }

    pub fn rule_restore(&self, id: u64) -> Result<(), FjellError> {
        Ok(self.inner.rule_restore(m::RuleId(id))?)
    }

    pub fn people_list(&self) -> Result<Vec<PersonView>, FjellError> {
        Ok(self
            .inner
            .people_list()?
            .into_iter()
            .map(Into::into)
            .collect())
    }

    pub fn person_name(&self, id: u64, name: Option<String>) -> Result<(), FjellError> {
        Ok(self.inner.person_name(m::PersonId(id), name)?)
    }

    pub fn person_merge(&self, target: u64, absorbed: u64) -> Result<u64, FjellError> {
        Ok(self
            .inner
            .person_merge(m::PersonId(target), m::PersonId(absorbed))?
            .0)
    }

    pub fn person_unmerge(&self, absorbed: u64) -> Result<(), FjellError> {
        Ok(self.inner.person_unmerge(m::PersonId(absorbed))?)
    }

    pub fn person_protect(&self, id: u64, protect: bool) -> Result<(), FjellError> {
        Ok(self.inner.person_protect(m::PersonId(id), protect)?)
    }

    pub fn face_index_enabled(&self) -> bool {
        self.inner.face_index_enabled()
    }

    pub fn face_index_set(&self, enabled: bool) -> Result<(), FjellError> {
        Ok(self.inner.face_index_set(enabled)?)
    }

    /// Sletter all ansiktsdata (avledet, kan gjenskapes). Plattformen må be om bekreftelse først.
    pub fn face_index_wipe(&self) -> Result<u64, FjellError> {
        Ok(self.inner.face_index_wipe()?)
    }

    pub fn dashboard(&self) -> Result<DashboardData, FjellError> {
        Ok(self.inner.dashboard()?.into())
    }

    pub fn dashboard_full(&self) -> Result<Dashboard, FjellError> {
        Ok(self.inner.dashboard_full()?.into())
    }

    pub fn series_compare(&self, series: u64) -> Result<SeriesCompare, FjellError> {
        Ok(self.inner.series_compare(m::SeriesId(series))?.into())
    }

    pub fn thresholds(&self) -> Thresholds {
        self.inner.thresholds().into()
    }
}

/// `FjellApi`-metoder som er speilet som `pub fn` på `Fjell`-objektet. Paritetstesten
/// (`tests/parity.rs`) feiler hvis `FjellApi` får en metode som ikke står her eller i `NOT_MIRRORED_API`.
pub const MIRRORED_API: &[&str] = &[
    "start_scan",
    "cancel",
    "cards",
    "decide_many",
    "decide_many_confirmed",
    "set_keeper",
    "undo",
    "begin_purge",
    "confirm_purge",
    "confirm_purge_confirmed",
    "finish_purge",
    "home_summary",
    "trash_summary",
    "settings",
    "set_settings",
    "protect_set",
    "unprotect",
    "protections",
    "rules_list",
    "rule_save",
    "rule_preview",
    "rule_toggle",
    "rule_delete",
    "rule_restore",
    "people_list",
    "person_name",
    "person_merge",
    "person_unmerge",
    "person_protect",
    "face_index_enabled",
    "face_index_set",
    "face_index_wipe",
    "dashboard",
    "dashboard_full",
    "series_compare",
    "thresholds",
];

/// `FjellApi`-metoder som bevisst ikke er speilet, med begrunnelse. Hver ny oppføring må forklare hvorfor
/// plattformen ikke trenger metoden.
///
/// Robusthetsflaten (docs/v1/ROBUSTHET-KONTRAKT.md) er bare et desktop-behov i MVP: sikkerhetskopi av indeksen,
/// indekseringsstyring og diagnose har ingen iOS-/Android-bruker, og det bygges ingen mobilkode nå (CLAUDE.md,
/// beslutning 2026-11-11). Speiles i en egen fase hvis mobil trenger dem.
const DESKTOP_ONLY: &str =
    "robusthet (ROBUSTHET-KONTRAKT.md): bare desktop i MVP, speiles etter MVP ved behov";
pub const NOT_MIRRORED_API: &[(&str, &str)] = &[
    ("backup_create", DESKTOP_ONLY),
    ("backup_list", DESKTOP_ONLY),
    ("backup_restore", DESKTOP_ONLY),
    ("backup_restore_undo", DESKTOP_ONLY),
    ("integrity_check", DESKTOP_ONLY),
    ("indexing_status", DESKTOP_ONLY),
    ("indexing_pause", DESKTOP_ONLY),
    ("indexing_resume", DESKTOP_ONLY),
    ("indexing_cancel", DESKTOP_ONLY),
    ("indexing_settings", DESKTOP_ONLY),
    ("indexing_settings_set", DESKTOP_ONLY),
    ("diag_preview", DESKTOP_ONLY),
    ("diag_export", DESKTOP_ONLY),
];
