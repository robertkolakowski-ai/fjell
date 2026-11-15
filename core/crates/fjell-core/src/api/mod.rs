//! Modul `api` (L4): eneste offentlige flate og UniFFI-grensen. Se docs/mvp/ARKITEKTUR.md del 2.
//!
//! Avhenger av: pipeline, actions, store, model.
//! Grensesnitt: trait `FjellApi` (synkron, ingen async). Undermoduler: `types` (lesemodeller,
//! feil), `mock` (`MemApi`, mock for UI) og `fjell`/`fjell_ops` (`Fjell`: ekte implementasjon som
//! kobler store, actions, pipeline og algoritmene; skanning kjører i egen tråd, `Fjell::wait_scan`; V1-flaten
//! ligger i `fjell_v1`).
//! - `start_scan`, `cancel`, `cards`, `decide_many` (én batch), `set_keeper`,
//!   `begin_purge` → `confirm_purge` → `finish_purge`, `undo`, `home_summary`, `trash_summary`, `settings`.
//!
//! V1-flaten (docs/v1/V1-PLAN.md): `protect_set`/`unprotect`/`protections`, `rules_list`/`rule_save`/`rule_preview`/
//! `rule_toggle`/`rule_delete`/`rule_restore`, `people_list`/`person_name`/`person_merge`/`person_unmerge`/
//! `person_protect`, `face_index_enabled`/`face_index_set`/`face_index_wipe`, `dashboard`, `series_compare`,
//! `thresholds`. Alle har standardimplementasjon (tom verdi eller `ApiError::Internal("ikke implementert")`)
//! slik at `MemApi` og eldre implementasjoner ikke brekker; `Fjell` overstyrer alle (integrasjonsfasen, `fjell_v1`).
//!
//! `Confirmed`/`PurgeTicket` lages bare her (`pub(crate)`-konstruktører i `model`).

mod fjell;
mod fjell_backup;
mod fjell_ops;
mod fjell_rules;
#[cfg(test)]
mod fjell_tests;
mod fjell_v1;
#[cfg(test)]
mod fjell_v1_tests;
#[cfg(test)]
mod hardening_tests;
pub mod mock;
#[cfg(test)]
mod rules_tests;
pub mod types;
mod types_robust;

pub use fjell::Fjell;
pub use fjell_rules::KeeperChange;
pub use mock::MemApi;
pub use types::*;
pub use types_robust::*;

use crate::dashboard::{Dashboard, Insights, NextStep, Progress};
use crate::model::{
    AssetId, BackupId, BatchId, CardId, ConsequenceCode, DashboardData, DeleteOutcome, DiagReport,
    GroupId, IntegrityStatus, PersonId, ProtectScope, PurgeTicket, RestoreAck, RuleId, RuleSpec,
    SeriesId, SuggestionKind,
};

/// Standardsvar for V1-metoder som ennå ikke er koblet til `Fjell` (gjøres i integrasjonsfasen).
fn not_implemented<T>() -> Result<T, ApiError> {
    Err(ApiError::Internal("ikke implementert".to_string()))
}

/// Standardsvar for robusthetsflaten (backup, indeksering, diagnose) som ennå ikke er koblet til `Fjell`.
/// Koden er stabil (`ROBUST_ERROR_CODES`).
fn robust_not_implemented<T>() -> Result<T, ApiError> {
    Err(ApiError::Internal("not_implemented".to_string()))
}

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

    // ---- V1 (standardimplementasjoner; overstyres av `Fjell` i integrasjonsfasen) ----

    /// Beskytt et omfang. Idempotent. Kan angres med `unprotect`.
    fn protect_set(&self, scope: ProtectScope) -> Result<(), ApiError> {
        let _ = scope;
        not_implemented()
    }
    /// Fjern en beskyttelse (konfigurasjon, ikke bildesletting). Kan angres med `protect_set`.
    fn unprotect(&self, scope: ProtectScope) -> Result<(), ApiError> {
        let _ = scope;
        not_implemented()
    }
    fn protections(&self) -> Result<Vec<ProtectionEntry>, ApiError> {
        Ok(Vec::new())
    }

    fn rules_list(&self) -> Result<Vec<RuleView>, ApiError> {
        Ok(Vec::new())
    }
    /// Oppretter (`id = None`) eller oppdaterer en regel. Tom `RuleSpec` avvises (`Internal`/valideringskode).
    fn rule_save(&self, draft: RuleDraft) -> Result<RuleId, ApiError> {
        let _ = draft;
        not_implemented()
    }
    /// Forhåndsvis treff uten å lagre eller endre noe.
    fn rule_preview(&self, spec: RuleSpec) -> Result<RulePreview, ApiError> {
        let _ = spec;
        not_implemented()
    }
    fn rule_toggle(&self, id: RuleId, enabled: bool) -> Result<(), ApiError> {
        let _ = (id, enabled);
        not_implemented()
    }
    /// Angrebar sletting (`rule.deleted_at`); regelen kan hentes tilbake med `rule_restore`.
    fn rule_delete(&self, id: RuleId) -> Result<(), ApiError> {
        let _ = id;
        not_implemented()
    }
    fn rule_restore(&self, id: RuleId) -> Result<(), ApiError> {
        let _ = id;
        not_implemented()
    }

    fn people_list(&self) -> Result<Vec<PersonView>, ApiError> {
        Ok(Vec::new())
    }
    fn person_name(&self, id: PersonId, name: Option<String>) -> Result<(), ApiError> {
        let _ = (id, name);
        not_implemented()
    }
    /// Slår `absorbed` inn i `target` (`person.merged_into`); kan angres med `person_unmerge`. Returnerer `target`.
    fn person_merge(&self, target: PersonId, absorbed: PersonId) -> Result<PersonId, ApiError> {
        let _ = (target, absorbed);
        not_implemented()
    }
    fn person_unmerge(&self, absorbed: PersonId) -> Result<(), ApiError> {
        let _ = absorbed;
        not_implemented()
    }
    /// Beskytt (`true`) eller opphev beskyttelse for alle bilder med personen (`ProtectScope::Person`).
    fn person_protect(&self, id: PersonId, protect: bool) -> Result<(), ApiError> {
        let _ = (id, protect);
        not_implemented()
    }

    /// Ansiktsindeks er av som standard.
    fn face_index_enabled(&self) -> bool {
        false
    }
    fn face_index_set(&self, enabled: bool) -> Result<(), ApiError> {
        let _ = enabled;
        not_implemented()
    }
    /// Sletter all ansiktsdata (embeddings og personer). Eksplisitt personvernvalg i UI med bekreftelse og
    /// varsel; avledet data som kan gjenskapes. Returnerer antall ansikter slettet.
    fn face_index_wipe(&self) -> Result<u64, ApiError> {
        not_implemented()
    }

    fn dashboard(&self) -> Result<DashboardData, ApiError> {
        Ok(DashboardData::default())
    }
    /// Dashboard med fremdrift, foreslått neste steg og «tømt» per måned. Standard: kontraktdata med tomme tillegg.
    fn dashboard_full(&self) -> Result<Dashboard, ApiError> {
        let data = self.dashboard()?;
        Ok(Dashboard {
            insights: Insights {
                reclaimable_count: 0,
                reclaimable_bytes: 0,
                protected_count: data.protected_count,
                progress: Progress::default(),
                next_step: NextStep::Nothing,
                freed_by_month: Vec::new(),
            },
            data,
        })
    }
    fn series_compare(&self, series: SeriesId) -> Result<SeriesCompare, ApiError> {
        let _ = series;
        not_implemented()
    }
    /// Effektive terskler for gjeldende `Settings::sensitivity`.
    fn thresholds(&self) -> Thresholds {
        Thresholds::default()
    }

    // ---- Robusthet (docs/v1/ROBUSTHET-KONTRAKT.md): standardimplementasjoner; grenene overstyrer i `Fjell` ----

    /// Tar en sikkerhetskopi av indeksen (`BackupKind::Manual`). Bare metadata i Fjells egen indeks, aldri
    /// bildefiler. Feilkoder: `backup_failed`.
    fn backup_create(&self) -> Result<BackupEntry, ApiError> {
        robust_not_implemented()
    }
    /// Alle kopier, nyeste først.
    fn backup_list(&self) -> Result<Vec<BackupEntry>, ApiError> {
        Ok(Vec::new())
    }
    /// Erstatter indeksen med kopien `ack.backup()`. Angrebar: kjernen tar først en `PreRestore`-kopi
    /// (`RestoreReport::safety`). Originalbilder røres ikke. Feilkoder: `backup_not_found`, `backup_corrupt`,
    /// `backup_failed`, `index_busy` (skanning pågår).
    fn backup_restore(&self, ack: RestoreAck) -> Result<RestoreReport, ApiError> {
        let _ = ack;
        robust_not_implemented()
    }
    /// Angrer en gjenoppretting ved å sette `safety` tilbake. Krever ingen bekreftelse (det er selve angre).
    fn backup_restore_undo(&self, safety: BackupId) -> Result<RestoreReport, ApiError> {
        let _ = safety;
        robust_not_implemented()
    }
    /// Sjekker at indeksen er hel. Standard: `Ok` (mocken har ingen ekte indeks).
    fn integrity_check(&self) -> Result<IntegrityStatus, ApiError> {
        Ok(IntegrityStatus::Ok)
    }

    /// Status for indeksering. Standard: ledig.
    fn indexing_status(&self) -> Result<IndexingStatus, ApiError> {
        Ok(IndexingStatus::default())
    }
    /// Pauser (brukervalg). Feilkode: `indexing_not_running`.
    fn indexing_pause(&self) -> Result<(), ApiError> {
        robust_not_implemented()
    }
    /// Fortsetter etter pause (ferdig analyserte bilder hoppes over).
    fn indexing_resume(&self) -> Result<(), ApiError> {
        robust_not_implemented()
    }
    /// Avbryter. Det som er funnet beholdes. Feilkode: `indexing_not_running`.
    fn indexing_cancel(&self) -> Result<(), ApiError> {
        robust_not_implemented()
    }
    fn indexing_settings(&self) -> Result<IndexingSettings, ApiError> {
        Ok(IndexingSettings::default())
    }
    /// Lagrer innstillingene. Feilkode: `indexing_invalid_settings` (`threads > MAX_INDEX_THREADS`).
    fn indexing_settings_set(&self, settings: IndexingSettings) -> Result<(), ApiError> {
        let _ = settings;
        robust_not_implemented()
    }

    /// Diagnose slik brukeren ser den før eksport: tall, koder og en redigert tekstblokk. Ingen stier, filnavn
    /// eller bilder. Standard: tom rapport.
    fn diag_preview(&self) -> Result<DiagReport, ApiError> {
        Ok(DiagReport::empty())
    }
    /// Samme innhold som `diag_preview`, til eksport. Kjernen sender aldri noe av seg selv; hvor teksten
    /// havner avgjør brukeren (åpent spørsmål i kontrakten). Feilkode: `diag_failed`.
    fn diag_export(&self) -> Result<DiagReport, ApiError> {
        robust_not_implemented()
    }
}
