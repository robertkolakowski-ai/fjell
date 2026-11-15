//! Robusthetsflaten for `Fjell` (L4): sikkerhetskopi av indeksen og integritet (backup-delen). Del av `api`.
//! Indekseringsdelen (`pipeline::control`) og diagnosen legges i egne `impl`-blokker av sine grener.
//!
//! Avhenger av: `super::fjell` (`Fjell`), store (`Store::backup_*`, `check_file`, `restore_backup`), model.
//! Grensesnitt: `pub(super)` metoder på `Fjell` som `impl FjellApi for Fjell` (i `fjell_ops`) delegerer til,
//! pluss `Fjell::backup_auto_if_due` (kalles av skallet ved trygge tidspunkter).
//!
//! Indeksering (gren `indexing`): `indexing_*` styrer bakgrunnsskanningen via `pipeline::control`
//! (`ScanControl`). Innstillinger lagres i `setting`; plattformkoblinger injiseres av skallet. Tester: `fjell_backup/indexing_tests.rs`.
//! Regler (docs/v1/BACKUP.md): kopier tas via egen skrivebeskyttet tilkobling og holder aldri skrivelåsen
//! (en handling venter ikke på en kopi). Gjenoppretting krever `RestoreAck`, tar en `PreRestore`-kopi først og
//! avvises mens en skanning pågår (`index_busy`). Ingenting her rører originalbilder. Feil er koder.

use super::fjell::Fjell;
use super::types_robust::{IndexingSettings, IndexingStatus};
use super::{ApiError, BackupEntry, RestoreReport};
use crate::model::{BackupId, BackupKind, IndexPriority, IndexState, IntegrityStatus, RestoreAck};
use crate::pipeline::{ControlState, GroupEngine, Phase, PowerProbe, ScanControl, WorkerHook};
use crate::store::Store;
use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;

/// En automatisk kopi når ingen er tatt siste uke («ukentlig»).
const WEEKLY_SECS: i64 = 7 * 24 * 3600;
/// Før en stor «Tøm» tas en ny kopi hvis den siste er eldre enn dette.
const BEFORE_PURGE_SECS: i64 = 3600;

/// `BackupMeta` kan ikke navngis utenfor `store` (modulen er privat), derfor et makro i stedet for en funksjon.
macro_rules! entry {
    ($m:expr) => {{
        let m = $m;
        BackupEntry {
            id: m.id,
            kind: m.kind,
            created_at: m.created_at,
            bytes: m.bytes,
            asset_count: m.asset_count,
            integrity: m.integrity,
        }
    }};
}

fn code(c: &str) -> ApiError {
    ApiError::Internal(c.to_string())
}

impl Fjell {
    /// Indeksfilen. Låsen på indeksen holdes bare mens stien leses.
    fn backup_db(&self) -> Result<PathBuf, ApiError> {
        self.actions()
            .store_mut()
            .db_path()
            .map(|p| p.to_path_buf())
            .ok_or_else(|| code("backup_failed"))
    }

    fn scan_busy(&self) -> bool {
        !matches!(self.sh.progress.snapshot().phase, Phase::Done | Phase::Idle)
    }

    pub(super) fn backup_make(&self) -> Result<BackupEntry, ApiError> {
        self.backup_take(BackupKind::Manual, "manual")
    }

    fn backup_take(&self, kind: BackupKind, reason: &str) -> Result<BackupEntry, ApiError> {
        let db = self.backup_db()?;
        let keep = self.actions().store_mut().backup_keep_auto();
        let now = self.sh.clock.now();
        let made =
            Store::backup_create_at(&db, kind, reason, now, keep).map_err(|e| code(e.code()))?;
        Ok(entry!(&made.meta))
    }

    pub(super) fn backup_entries(&self) -> Result<Vec<BackupEntry>, ApiError> {
        let Ok(db) = self.backup_db() else {
            return Ok(Vec::new()); // minnedatabase: ingen kopier
        };
        Ok(Store::backup_list_at(&db)
            .iter()
            .map(|m| entry!(m))
            .collect())
    }

    pub(super) fn backup_restore_ack(&self, ack: RestoreAck) -> Result<RestoreReport, ApiError> {
        self.backup_swap_in(ack.backup())
    }

    /// Angre: setter `safety` tilbake. Bare `PreRestore`-kopier godtas (ellers `backup_not_found`).
    pub(super) fn backup_undo(&self, safety: BackupId) -> Result<RestoreReport, ApiError> {
        let db = self.backup_db()?;
        let is_safety = Store::backup_list_at(&db)
            .iter()
            .any(|m| m.id == safety && m.kind == BackupKind::PreRestore);
        if !is_safety {
            return Err(code("backup_not_found"));
        }
        self.backup_swap_in(safety)
    }

    fn backup_swap_in(&self, id: BackupId) -> Result<RestoreReport, ApiError> {
        if self.scan_busy() {
            return Err(code("index_busy"));
        }
        let now = self.sh.clock.now();
        // Hele byttet skjer under skrivelåsen, så ingen handling skriver til en halvt byttet indeks.
        let done = self
            .actions()
            .store_mut()
            .restore_backup(id, now)
            .map_err(|e| code(e.code()))?;
        // Gruppebufferen hører til den gamle indeksen.
        *self.sh.engine.lock().unwrap_or_else(|e| e.into_inner()) = GroupEngine::new();
        let _ = self.refresh();
        Ok(RestoreReport {
            restored: done.restored.id,
            safety: done.safety.id,
            asset_count: done.restored.asset_count,
        })
    }

    /// Full integritetssjekk av indeksfilen via egen tilkobling (blokkerer ikke skriveren). Minnedatabase: `Ok`.
    pub(super) fn integrity_now(&self) -> Result<IntegrityStatus, ApiError> {
        match self.backup_db() {
            Ok(db) => Ok(Store::check_file(&db, true).status()),
            Err(_) => Ok(IntegrityStatus::Ok),
        }
    }

    /// Automatisk kopi ved et trygt tidspunkt, bare hvis det trengs: `weekly` (ingen kopi siste 7 døgn) eller
    /// `before_purge` (ingen siste time). Gir `None` når ingenting ble tatt. Kjøres av skallet i bakgrunnen
    /// (aldri i en UI-handling). Kopien er synlig i listen og rulleres bare blant `Auto`-kopier.
    pub fn backup_auto_if_due(&self, reason: &str) -> Result<Option<BackupEntry>, ApiError> {
        let window = match reason {
            "weekly" => WEEKLY_SECS,
            "before_purge" => BEFORE_PURGE_SECS,
            _ => return Err(code("backup_failed")),
        };
        let now = self.sh.clock.now();
        let recent = self
            .backup_entries()?
            .iter()
            .any(|b| b.kind != BackupKind::PreRestore && b.created_at > now - window);
        if recent {
            return Ok(None);
        }
        self.backup_take(BackupKind::Auto, reason).map(Some)
    }
}

#[cfg(test)]
mod indexing_tests;
#[cfg(test)]
mod tests;

const KEY_THREADS: &str = "indexing.threads";
const KEY_PRIORITY: &str = "indexing.priority";
const KEY_BATTERY: &str = "indexing.pause_on_battery";

fn not_running<T>() -> Result<T, ApiError> {
    Err(ApiError::Internal("indexing_not_running".to_string()))
}

impl Fjell {
    /// Skallet kobler strømstatus (plattform) til kjernen. Uten kilde pauses det aldri på batteri.
    pub fn set_power_probe(&self, probe: Arc<dyn PowerProbe>) {
        self.sh.control.set_power_probe(probe);
    }

    /// Skallet kobler plattformkode som kjøres først på hver arbeidstråd (lav OS-prioritet på Windows).
    pub fn set_worker_hook(&self, hook: WorkerHook) {
        self.sh.control.set_worker_hook(hook);
    }

    /// Tidsavbrudd per kall mot kilden (standard 60 s; `None` slår det av).
    pub fn set_source_call_timeout(&self, t: Option<Duration>) {
        self.sh.control.set_call_timeout(t);
    }

    /// Ventemodus for frakoblet disk (standard av): `Some(intervall)` lar indekseringen stå i tilstanden «frakoblet disk»
    /// og prøve igjen, i stedet for å avslutte med feil. Av som standard fordi skall og tester venter på at en skanning
    /// avsluttes; slås på av skallet når Hjem kan vise og avbryte ventingen (se docs/v1/INDEKSERING.md).
    pub fn set_offline_wait(&self, every: Option<Duration>) {
        self.sh.control.set_offline_retry(every);
    }

    /// Styringen (skall og tester).
    pub fn indexing_control(&self) -> &ScanControl {
        &self.sh.control
    }

    /// Leser lagrede innstillinger og gir dem til styringen. Ugyldige eller manglende verdier gir standard.
    pub(super) fn load_indexing_limits(&self) {
        let s = self.read_indexing_settings();
        self.sh
            .control
            .set_limits(s.threads, s.priority, s.pause_on_battery);
    }

    fn read_indexing_settings(&self) -> IndexingSettings {
        let d = IndexingSettings::default();
        let a = self.actions();
        let st = a.store();
        let get = |k: &str| st.setting(k).ok().flatten();
        let s = IndexingSettings {
            threads: get(KEY_THREADS)
                .and_then(|v| v.parse().ok())
                .unwrap_or(d.threads),
            priority: get(KEY_PRIORITY)
                .and_then(|v| IndexPriority::parse(&v))
                .unwrap_or(d.priority),
            pause_on_battery: get(KEY_BATTERY).map_or(d.pause_on_battery, |v| v != "0"),
        };
        if s.is_valid() {
            s
        } else {
            d
        }
    }

    pub(super) fn indexing_status_impl(&self) -> Result<IndexingStatus, ApiError> {
        let sh = &self.sh;
        let snap = sh.progress.snapshot();
        let st = sh.control.status(&snap);
        let phase = self.scan_progress().phase;
        if st.state == ControlState::Idle {
            return Ok(IndexingStatus {
                phase,
                ..IndexingStatus::default()
            });
        }
        Ok(IndexingStatus {
            state: match st.state {
                ControlState::Running => IndexState::Running,
                ControlState::WaitingForPower => IndexState::PausedOnBattery,
                // `IndexState` har ingen kode for frakoblet disk ennå (delt modell, meldt i integrasjonsnotatene):
                // til da vises den som «pauset». `ControlState::DiskOffline` er kilden til sannhet.
                ControlState::Paused | ControlState::DiskOffline | ControlState::Idle => {
                    IndexState::Paused
                }
            },
            phase,
            done: snap.done,
            total: snap.total,
            rate_per_sec: st.rate_per_sec,
            eta_secs: st.eta_secs,
        })
    }

    pub(super) fn indexing_pause_impl(&self) -> Result<(), ApiError> {
        if !self.sh.control.is_active() {
            return not_running();
        }
        self.sh.control.pause();
        Ok(())
    }

    pub(super) fn indexing_resume_impl(&self) -> Result<(), ApiError> {
        if !self.sh.control.is_active() {
            return not_running();
        }
        self.sh.control.resume();
        Ok(())
    }

    pub(super) fn indexing_cancel_impl(&self) -> Result<(), ApiError> {
        if !self.sh.control.is_active() {
            return not_running();
        }
        self.sh.cancel.store(true, Ordering::SeqCst);
        self.sh.control.wake();
        Ok(())
    }

    pub(super) fn indexing_settings_impl(&self) -> Result<IndexingSettings, ApiError> {
        Ok(self.read_indexing_settings())
    }

    /// Lagrer og bruker innstillingene med en gang (tråder og prioritet gjelder fra neste pool, batteri straks).
    pub(super) fn indexing_settings_set_impl(&self, s: IndexingSettings) -> Result<(), ApiError> {
        if !s.is_valid() {
            return Err(ApiError::Internal("indexing_invalid_settings".to_string()));
        }
        {
            let mut a = self.actions();
            let st = a.store_mut();
            let e = |e: crate::store::StoreError| ApiError::Internal(e.to_string());
            st.set_setting(KEY_THREADS, &s.threads.to_string())
                .map_err(e)?;
            st.set_setting(KEY_PRIORITY, s.priority.as_str())
                .map_err(e)?;
            st.set_setting(KEY_BATTERY, if s.pause_on_battery { "1" } else { "0" })
                .map_err(e)?;
        }
        self.sh
            .control
            .set_limits(s.threads, s.priority, s.pause_on_battery);
        Ok(())
    }
}
