//! `Fjell`: ekte implementasjon av `FjellApi` (L4). Koblingen mellom store, actions, pipeline og
//! algoritmene. Se docs/mvp/ARKITEKTUR.md del 2 og PLAN-KJERNE del 8.
//!
//! Avhenger av: pipeline, actions, storage, store, model, `super` (typer). Grensesnitt:
//! - `Fjell::new(store, source, clock)`: eier indeksen (`Mutex<Actions>`), én skrivetilkobling. Uten
//!   analysatorer (Null: `Unavailable`). `Fjell::with_analyzers(store, source, clock, eye, face)` injiserer
//!   øye- og ansiktsmodeller (V1); etter hver skanning kjører `pipeline::run_analyzers`, og ansikter bare når
//!   `face_index` er på.
//! - `FjellApi::start_scan` starter pipeline i en egen tråd og returnerer straks; `Fjell::wait_scan()`
//!   venter og gir `ScanReport` (CLI og tester), `last_report()` leser siste. `cancel()` setter flagget.
//! - `Fjell::storage_summary()` / `largest_files(n)`: lagringsoversikt (lesing, egen tilkobling).
//! - Resten av `FjellApi` ligger i `fjell_ops` (kort, beslutninger, angre, purge) og `fjell_v1` (V1: beskyttelse,
//!   forslagsregler, personer, ansiktsindeks, dashboard, seriesammenligning, innstillinger). Keeper-reglene
//!   ligger i `fjell_rules`.
//!
//! Etter trash/restore/purge kaller `Fjell` `pipeline::refresh_groups` (actions kan ikke kalle pipeline), i
//! bakgrunnen og sammenslått (`Fjell::sync_groups` venter). Utregningen skjer utenfor indekslåsen.
//! Låsen på indeksen holdes bare per kall/batch, aldri under dekoding av miniatyrer.

use super::{ApiError, ScanConfig, ScanMode, ScanPhase, ScanProgress};
use crate::actions::{ActionError, Actions};
use crate::model::{
    AssetSource, Clock, EyeAnalyzer, FaceAnalyzer, NullEyeAnalyzer, NullFaceAnalyzer,
};
use crate::pipeline::{self, GroupEngine, Phase, PipelineError, Progress, ScanReport, StoreHandle};
use crate::store::Store;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread::JoinHandle;
use std::time::Duration;

/// Opptil så mange aktive bilder med fingeravtrykk regnes gruppene ut synkront etter en handling.
const SYNC_REFRESH_MAX_ROWS: usize = 2_000;

pub(super) struct Shared {
    pub actions: Mutex<Actions>,
    pub src: Arc<dyn AssetSource>,
    pub clock: Arc<dyn Clock>,
    pub eye: Arc<dyn EyeAnalyzer>,
    pub face: Arc<dyn FaceAnalyzer>,
    pub cancel: AtomicBool,
    pub progress: Progress,
    pub report: Mutex<Option<Result<ScanReport, String>>>,
    /// Buffer for inkrementell klynging (deles av skanning og oppdatering etter handlinger).
    pub engine: Mutex<GroupEngine>,
    /// Oppdatering av grupper etter handlinger kjører i bakgrunnen (se `Fjell::refresh`).
    pub dirty: AtomicBool,
    pub refreshing: AtomicBool,
    pub refresh_error: Mutex<Option<String>>,
}

/// `StoreHandle` over `Mutex<Actions>`: pipeline får bare skrivetilkoblingen.
pub(super) struct Handle<'a>(pub &'a Shared);

impl StoreHandle for Handle<'_> {
    fn with_store<R>(&self, f: impl FnOnce(&mut Store) -> R) -> R {
        f(self
            .0
            .actions
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .store_mut())
    }
    fn engine(&self) -> Option<&Mutex<GroupEngine>> {
        Some(&self.0.engine)
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
        Fjell::with_analyzers(
            store,
            src,
            clock,
            Arc::new(NullEyeAnalyzer),
            Arc::new(NullFaceAnalyzer),
        )
    }

    /// Som `new`, men med øye- og ansiktsanalysatorer (modellene ligger utenfor kjernen og injiseres).
    pub fn with_analyzers(
        store: Store,
        src: Arc<dyn AssetSource>,
        clock: Arc<dyn Clock>,
        eye: Arc<dyn EyeAnalyzer>,
        face: Arc<dyn FaceAnalyzer>,
    ) -> Fjell {
        Fjell {
            sh: Arc::new(Shared {
                actions: Mutex::new(Actions::new(store, clock.clone())),
                src,
                clock,
                eye,
                face,
                cancel: AtomicBool::new(false),
                progress: Progress::new(),
                report: Mutex::new(None),
                engine: Mutex::new(GroupEngine::new()),
                dirty: AtomicBool::new(false),
                refreshing: AtomicBool::new(false),
                refresh_error: Mutex::new(None),
            }),
            worker: Mutex::new(None),
        }
    }

    pub(super) fn actions(&self) -> MutexGuard<'_, Actions> {
        self.sh.actions.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Ber om ny utregning av grupper og forslag (etter trash/restore/purge). Utregningen er O(biblioteket)
    /// og ville brutt «handling < 100 ms» på store indekser, så den kjører i bakgrunnen og slås sammen
    /// (flere handlinger på rad gir én utregning). Handlingen selv er ferdig og varig når kallet
    /// returnerer; kortlister filtrerer allerede bort slettede bilder. `sync_groups` venter på utregningen.
    pub(super) fn refresh(&self) -> Result<(), ApiError> {
        let sh = &self.sh;
        // Små indekser regnes ferdig før kallet returnerer (billig, og kortlistene er alltid à jour).
        // Store indekser (og når en utregning allerede pågår) går i bakgrunnen.
        let small = !sh.refreshing.load(Ordering::SeqCst)
            && !sh.dirty.load(Ordering::SeqCst)
            && sh
                .engine
                .try_lock()
                .is_ok_and(|e| e.rows() <= SYNC_REFRESH_MAX_ROWS);
        if small {
            return pipeline::refresh_groups(&Handle(sh), sh.clock.as_ref())
                .map(|_| ())
                .map_err(internal);
        }
        sh.dirty.store(true, Ordering::SeqCst);
        if sh.refreshing.swap(true, Ordering::SeqCst) {
            return Ok(()); // en bakgrunnsutregning pågår og tar også denne endringen
        }
        let sh = self.sh.clone();
        std::thread::spawn(move || loop {
            while sh.dirty.swap(false, Ordering::SeqCst) {
                let r = pipeline::refresh_groups(&Handle(&sh), sh.clock.as_ref());
                *sh.refresh_error.lock().unwrap_or_else(|e| e.into_inner()) =
                    r.err().map(|e| e.to_string());
            }
            sh.refreshing.store(false, Ordering::SeqCst);
            // En endring kan ha kommet mellom siste sjekk og frigivelsen.
            if !sh.dirty.load(Ordering::SeqCst) || sh.refreshing.swap(true, Ordering::SeqCst) {
                break;
            }
        });
        Ok(())
    }

    /// Venter til bakgrunnsutregningen av grupper er ferdig (tester, CLI og avslutning).
    pub fn sync_groups(&self) -> Result<(), ApiError> {
        while self.sh.dirty.load(Ordering::SeqCst) || self.sh.refreshing.load(Ordering::SeqCst) {
            std::thread::sleep(Duration::from_millis(1));
        }
        match self
            .sh
            .refresh_error
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
        {
            Some(e) => Err(ApiError::Internal(e)),
            None => Ok(()),
        }
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

    /// Karantenen la en gjenopprettet fil på et annet navn (kollisjon): pek indeksen dit (F1).
    /// Kalles av skallet med resultatet fra `Quarantine::unstage`/`reconcile`, aldri med annet.
    pub fn relocate_source(
        &self,
        id: crate::model::AssetId,
        to: &crate::model::SourceId,
    ) -> Result<bool, ApiError> {
        let moved = self
            .actions()
            .store_mut()
            .relocate_source(id, to)
            .map_err(|e| ApiError::Internal(e.to_string()))?;
        Ok(moved)
    }

    /// Lesetilgang til indeksen (diagnostikk, CLI og tester). Skriving går bare via `FjellApi`.
    pub fn with_store<R>(&self, f: impl FnOnce(&Store) -> R) -> R {
        f(self.actions().store())
    }

    /// Lagringsoversikt (se `storage`). Leser via egen skrivebeskyttet tilkobling når indeksen er en fil,
    /// så verken skanning eller handlinger blokkeres; minnedatabase leses under indekslåsen.
    pub fn storage_summary(&self) -> Result<crate::storage::StorageSummary, ApiError> {
        self.storage_summary_with(&crate::storage::StorageOptions::default())
    }

    pub fn storage_summary_with(
        &self,
        opts: &crate::storage::StorageOptions,
    ) -> Result<crate::storage::StorageSummary, ApiError> {
        let reader = self.actions().store().reader().map_err(internal)?;
        match reader {
            Some(conn) => crate::storage::summarize_on(&conn, opts),
            None => crate::storage::summarize(self.actions().store(), opts),
        }
        .map_err(internal)
    }

    /// De `n` største aktive filene (id, sti, størrelse, type).
    pub fn largest_files(&self, n: usize) -> Result<Vec<crate::storage::FileUsage>, ApiError> {
        let reader = self.actions().store().reader().map_err(internal)?;
        match reader {
            Some(conn) => crate::storage::largest_files_on(&conn, n),
            None => crate::storage::largest_files_on(self.actions().store().read_conn(), n),
        }
        .map_err(internal)
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
                &Handle(&sh),
                sh.clock.as_ref(),
                &cfg,
                &sh.progress,
                &sh.cancel,
            );
            // Øyne og ansikter etter selve skanningen; avbrutt skanning hopper over steget.
            let r = r.and_then(|rep| {
                if !rep.cancelled && !rep.deadline_hit {
                    pipeline::run_analyzers(
                        sh.src.as_ref(),
                        &Handle(&sh),
                        sh.eye.as_ref(),
                        sh.face.as_ref(),
                        sh.clock.as_ref(),
                        &sh.cancel,
                    )?;
                }
                Ok(rep)
            });
            let r = r.map_err(|e: PipelineError| e.to_string());
            *sh.report.lock().unwrap_or_else(|e| e.into_inner()) = Some(r);
        }));
        Ok(())
    }
}
