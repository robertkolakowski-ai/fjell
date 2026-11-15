//! Robusthetsflaten for `Fjell` (L4): sikkerhetskopi av indeksen, integritet, indeksering og diagnose.
//! Del av `api`. Skall i kontraktfasen: `Fjell` bruker standardimplementasjonene i `FjellApi`
//! (`not_implemented`/tom verdi) til grenen som eier denne filen kobler den til `store::backup`
//! og `pipeline::control`. Se docs/v1/ROBUSTHET-KONTRAKT.md (eierskap og regler).
//!
//! Avhenger av: `super::fjell` (`Fjell`), store, pipeline, model. Grensesnitt: `pub(super)` metoder på `Fjell`
//! som `impl FjellApi for Fjell` (i `fjell_ops`) delegerer til når grenene kobler dem inn.
//!
//! Regler: gjenoppretting går bare med `RestoreAck` og tar alltid en `PreRestore`-kopi først (angrebar);
//! ingen kall her skriver til originalbilder; feil er koder (`ROBUST_ERROR_CODES`), aldri tekst.
//!
//! Indeksering (gren `indexing`): `indexing_*` styrer bakgrunnsskanningen via `pipeline::control`
//! (`ScanControl`). Innstillinger lagres i `setting` (`indexing.threads`, `indexing.priority`,
//! `indexing.pause_on_battery`). Plattformkoblingene (strømstatus, trådprioritet) injiseres av skallet
//! (`Fjell::set_power_probe`, `Fjell::set_worker_hook`); kjernen kjenner ingen plattform.
//! Tester: `fjell_backup/indexing_tests.rs`.

use super::fjell::Fjell;
use super::types_robust::{IndexingSettings, IndexingStatus};
use super::ApiError;
use crate::model::{IndexPriority, IndexState};
use crate::pipeline::{ControlState, PowerProbe, ScanControl, WorkerHook};
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;

#[cfg(test)]
mod indexing_tests;

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
