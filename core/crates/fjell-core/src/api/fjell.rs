//! `Fjell`: ekte implementasjon av `FjellApi` (L4). Koblingen mellom store, actions, pipeline og
//! algoritmene. Se docs/mvp/ARKITEKTUR.md del 2 og PLAN-KJERNE del 8.
//!
//! Avhenger av: pipeline, actions, store, model, `super` (typer). Grensesnitt:
//! - `Fjell::new(store, source, clock)`: eier indeksen (`Mutex<Actions>`), én skrivetilkobling.
//! - `FjellApi::start_scan` starter pipeline i en egen tråd og returnerer straks; `Fjell::wait_scan()`
//!   venter og gir `ScanReport` (CLI og tester), `last_report()` leser siste. `cancel()` setter flagget.
//! - Resten av `FjellApi` ligger i `fjell_ops` (kort, beslutninger, angre, purge).
//!
//! Etter trash/restore/purge kaller `Fjell` `pipeline::refresh_groups` (actions kan ikke kalle pipeline).
//! Låsen på indeksen holdes bare per kall/batch, aldri under dekoding av miniatyrer.

use super::{ApiError, ScanConfig, ScanMode, ScanPhase, ScanProgress};
use crate::actions::{ActionError, Actions};
use crate::model::{AssetSource, Clock};
use crate::pipeline::{self, Phase, PipelineError, Progress, ScanReport, StoreHandle};
use crate::store::Store;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread::JoinHandle;
use std::time::Duration;

pub(super) struct Shared {
    pub actions: Mutex<Actions>,
    pub src: Arc<dyn AssetSource>,
    pub clock: Arc<dyn Clock>,
    pub cancel: AtomicBool,
    pub progress: Progress,
    pub report: Mutex<Option<Result<ScanReport, String>>>,
}

/// `StoreHandle` over `Mutex<Actions>`: pipeline får bare skrivetilkoblingen.
pub(super) struct Handle<'a>(pub &'a Mutex<Actions>);

impl StoreHandle for Handle<'_> {
    fn with_store<R>(&self, f: impl FnOnce(&mut Store) -> R) -> R {
        f(self.0.lock().unwrap_or_else(|e| e.into_inner()).store_mut())
    }
}

/// Ekte kjerne bak `FjellApi`.
pub struct Fjell {
    pub(super) sh: Arc<Shared>,
    worker: Mutex<Option<JoinHandle<()>>>,
}

pub(super) fn act_err(e: ActionError) -> ApiError {
    match e {
        ActionError::AckRequired(c) => ApiError::AckRequired(c),
        ActionError::PlanStale => ApiError::PlanStale,
        ActionError::PurgeStale => ApiError::PurgeStale,
        ActionError::NothingToUndo => ApiError::NothingToUndo,
        ActionError::NotUndoable => ApiError::NotUndoable,
        ActionError::AlreadyPurged(a) => ApiError::AlreadyPurged(a),
        ActionError::UnknownAsset(a) => ApiError::UnknownAsset(a),
        ActionError::NotTrashed(a) => ApiError::NotTrashed(a),
        ActionError::WouldEmptyGroup(g) => ApiError::WouldEmptyGroup(g),
        other => ApiError::Internal(other.to_string()),
    }
}

pub(super) fn internal(e: impl std::fmt::Display) -> ApiError {
    ApiError::Internal(e.to_string())
}

impl Fjell {
    pub fn new(store: Store, src: Arc<dyn AssetSource>, clock: Arc<dyn Clock>) -> Fjell {
        Fjell {
            sh: Arc::new(Shared {
                actions: Mutex::new(Actions::new(store, clock.clone())),
                src,
                clock,
                cancel: AtomicBool::new(false),
                progress: Progress::new(),
                report: Mutex::new(None),
            }),
            worker: Mutex::new(None),
        }
    }

    pub(super) fn actions(&self) -> MutexGuard<'_, Actions> {
        self.sh.actions.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Regner grupper og forslag på nytt (etter trash/restore/purge).
    pub(super) fn refresh(&self) -> Result<(), ApiError> {
        pipeline::refresh_groups(&Handle(&self.sh.actions), self.sh.clock.as_ref())
            .map(|_| ())
            .map_err(internal)
    }

    /// Venter til pågående skanning er ferdig og gir rapporten (`None` hvis ingen skanning har kjørt).
    pub fn wait_scan(&self) -> Option<Result<ScanReport, ApiError>> {
        let h = self.worker.lock().unwrap_or_else(|e| e.into_inner()).take();
        if let Some(h) = h {
            let _ = h.join();
        }
        self.last_report()
    }

    /// Siste skanning sin rapport eller feil.
    pub fn last_report(&self) -> Option<Result<ScanReport, ApiError>> {
        let g = self.sh.report.lock().unwrap_or_else(|e| e.into_inner());
        g.clone().map(|r| r.map_err(ApiError::Internal))
    }

    /// Plattform-ID for id-ene, slik at UI kan kalle `PlatformDelete::delete(ticket, ids)`.
    pub fn sources_for(
        &self,
        ids: &[crate::model::AssetId],
    ) -> Vec<(crate::model::AssetId, crate::model::SourceId)> {
        self.actions().store().source_ids(ids).unwrap_or_default()
    }

    /// Lesetilgang til indeksen (diagnostikk, CLI og tester). Skriving går bare via `FjellApi`.
    pub fn with_store<R>(&self, f: impl FnOnce(&Store) -> R) -> R {
        f(self.actions().store())
    }

    pub(super) fn scan_progress(&self) -> ScanProgress {
        let s = self.sh.progress.snapshot();
        ScanProgress {
            phase: match s.phase {
                Phase::Idle => ScanPhase::Idle,
                Phase::Listing => ScanPhase::Listing,
                Phase::Exact => ScanPhase::Exact,
                Phase::ExactReady => ScanPhase::ExactReady,
                Phase::Analyzing => ScanPhase::Analyzing,
                Phase::Done => ScanPhase::Done,
            },
            done: s.done,
            total: s.total,
            found_exact: s.found_exact,
        }
    }

    pub(super) fn start(&self, config: ScanConfig) -> Result<(), ApiError> {
        let mut w = self.worker.lock().unwrap_or_else(|e| e.into_inner());
        if w.as_ref().is_some_and(|h| !h.is_finished()) {
            return Ok(()); // en skanning pågår allerede
        }
        if let Some(h) = w.take() {
            let _ = h.join();
        }
        let cfg = pipeline::ScanConfig {
            mode: match config.mode {
                ScanMode::Quick => pipeline::Mode::Quick,
                ScanMode::Full => pipeline::Mode::Full,
            },
            // 0 = bakgrunn (1–2 tråder), høyere = forgrunn.
            priority: if config.priority == 0 {
                pipeline::Priority::Background
            } else {
                pipeline::Priority::Foreground
            },
            deadline: config
                .deadline
                .map(|d| Duration::from_secs((d - self.sh.clock.now()).max(0) as u64)),
        };
        self.sh.cancel.store(false, Ordering::SeqCst);
        *self.sh.report.lock().unwrap_or_else(|e| e.into_inner()) = None;
        let sh = self.sh.clone();
        *w = Some(std::thread::spawn(move || {
            let r = pipeline::scan(
                sh.src.as_ref(),
                &Handle(&sh.actions),
                sh.clock.as_ref(),
                &cfg,
                &sh.progress,
                &sh.cancel,
            );
            let r = r.map_err(|e: PipelineError| e.to_string());
            *sh.report.lock().unwrap_or_else(|e| e.into_inner()) = Some(r);
        }));
        Ok(())
    }
}
