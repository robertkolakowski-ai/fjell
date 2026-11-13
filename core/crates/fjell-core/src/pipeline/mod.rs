//! Modul `pipeline` (L3): indeksering. Se docs/mvp/ARKITEKTUR.md del 2 og PLAN-KJERNE del 8 (K7).
//!
//! Avhenger av: store, hash, quality, screenshot, cluster, suggest, model. Ingen `tokio`, ingen `unsafe`.
//! Grensesnitt (hovedpunkter):
//! - `scan(&dyn AssetSource, &H: StoreHandle, &dyn Clock, &ScanConfig, &Progress, &AtomicBool) -> ScanReport`.
//!   Steg: 1 enumerer (`list_changes` fra `scan_cursor`, `upsert_assets`, avstemming av fjernede),
//!   2 rask fase (BLAKE3 uten dekoding, så eksakte grupper og forslag), 3 full fase (`work_set`,
//!   rayon, `thumbnail` → dHash/pHash/blur/signaler, én skrivetråd via `sync_channel`), 4 klynging
//!   (etter rask fase, hver 2 000. bilde og til slutt), 5 lagre `scan_cursor` og returner rapport.
//! - `ScanConfig { mode: Quick|Full, priority: Foreground|Background, deadline }`, `ScanReport`.
//! - `refresh_groups(store, clock)`: kalles av `api` etter trash/restore/purge (actions kan ikke kalle
//!   pipeline, lag L2 mot L3). `compute` er den rene delen.
//! - `StoreHandle`: hvordan pipeline får `&mut Store` uten å eie den (`Mutex<Store>`, eller `api`s
//!   `Mutex<Actions>`). Låsen holdes bare per batch, aldri under dekoding.
//! - `Progress` (monoton, maks 4 oppdateringer/s) og `cancel` (`AtomicBool`, sjekkes mellom bilder).
//!
//! Gjenopptak skjer fordi arbeidssettet utledes fra databasen. Ingen skriving mot originaler:
//! `AssetSource` har ingen skrivemetoder (beslutning 17). Skanning endrer aldri `state` på trashed
//! bortsett fra avstemming av fjernede kilder (I13).

mod fast;
mod full;
mod groups;
mod keeper;
mod progress;
#[cfg(test)]
pub(crate) mod tests;

pub(crate) use groups::member;
pub use groups::{
    compute, compute_with, compute_with_rules, refresh_groups, GroupEngine, Grouping, LOW_INFO_BIT,
};
pub use keeper::{is_protected, pick_group, pick_keeper, Pick, RuleCtx};
pub use progress::{Phase, Progress, Snapshot, DEFAULT_INTERVAL};

use crate::model::{AssetId, AssetSource, ChangeToken, Clock, SourceError, ALGO_VER};
use crate::store::{Store, StoreError};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    /// Steg 1–2: ingen dekoding, hopper over filer som ikke er lokale.
    Quick,
    /// Alle steg.
    Full,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Priority {
    /// Alle kjerner.
    Foreground,
    /// 1–2 tråder (varme og batteri). Lading/Wi-Fi velges av plattformen.
    Background,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ScanConfig {
    pub mode: Mode,
    pub priority: Priority,
    /// Stopp ryddig etter så lang tid (iOS `BGProcessingTask`). Måles fra skanningens start.
    pub deadline: Option<Duration>,
}

impl ScanConfig {
    pub fn full() -> ScanConfig {
        ScanConfig {
            mode: Mode::Full,
            priority: Priority::Foreground,
            deadline: None,
        }
    }
    pub fn quick() -> ScanConfig {
        ScanConfig {
            mode: Mode::Quick,
            ..ScanConfig::full()
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FailCause {
    Panic,
    Corrupt,
    NotFound,
    Other(String),
}

/// Ett bilde som feilet (og ble talt mot `fail_count` hvis det var i full fase).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Failure {
    pub id: AssetId,
    pub cause: FailCause,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Timings {
    pub enumerate: Duration,
    pub quick: Duration,
    pub full: Duration,
    pub cluster: Duration,
}

/// Resultat av en skanning.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ScanReport {
    pub new: u64,
    pub changed: u64,
    pub removed: u64,
    /// Filer hashet i rask fase.
    pub hashed: u64,
    /// Bilder analysert i full fase.
    pub analyzed: u64,
    /// `Unavailable` (f.eks. iCloud ikke lastet ned): hoppet over uten feiltelling.
    pub skipped_unavailable: u64,
    pub failures: Vec<Failure>,
    pub cancelled: bool,
    pub deadline_hit: bool,
    pub timings: Timings,
}

#[derive(Debug)]
pub enum PipelineError {
    Source(SourceError),
    Store(StoreError),
    Pool(String),
}

impl From<StoreError> for PipelineError {
    fn from(e: StoreError) -> Self {
        PipelineError::Store(e)
    }
}

impl From<SourceError> for PipelineError {
    fn from(e: SourceError) -> Self {
        PipelineError::Source(e)
    }
}

impl std::fmt::Display for PipelineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for PipelineError {}

/// Tilgang til den ene skrivetilkoblingen uten å eie den. Implementasjonen låser og slipper per kall.
pub trait StoreHandle: Sync {
    fn with_store<R>(&self, f: impl FnOnce(&mut Store) -> R) -> R;
    /// Buffer for inkrementell klynging som overlever mellom utregninger (`None`: bygges på nytt hver gang).
    fn engine(&self) -> Option<&Mutex<GroupEngine>> {
        None
    }
}

impl StoreHandle for Mutex<Store> {
    fn with_store<R>(&self, f: impl FnOnce(&mut Store) -> R) -> R {
        f(&mut self.lock().unwrap_or_else(|e| e.into_inner()))
    }
}

/// Felles kontekst for stegene.
pub(crate) struct Ctx<'a, H: StoreHandle> {
    pub src: &'a dyn AssetSource,
    pub store: &'a H,
    pub clock: &'a dyn Clock,
    pub cfg: &'a ScanConfig,
    pub progress: &'a Progress,
    pub cancel: &'a AtomicBool,
    pub started: Instant,
    pub algo: crate::model::AlgoVer,
}

impl<H: StoreHandle> Ctx<'_, H> {
    /// Avbrutt av bruker eller fristen er ute.
    pub fn stop(&self) -> bool {
        self.cancel.load(Ordering::SeqCst) || self.deadline_hit()
    }
    pub fn deadline_hit(&self) -> bool {
        self.cfg
            .deadline
            .is_some_and(|d| self.started.elapsed() >= d)
    }
    /// Tråder for rayon-poolen.
    pub fn threads(&self) -> usize {
        let all = std::thread::available_parallelism().map_or(2, |n| n.get());
        match self.cfg.priority {
            Priority::Foreground => all,
            Priority::Background => all.clamp(1, 2),
        }
    }
}

const CURSOR_KEY: &str = "scan_cursor";

/// Kjører en skanning. Se modulhodet. `cancel` kan settes fra en annen tråd.
pub fn scan<H: StoreHandle>(
    src: &dyn AssetSource,
    store: &H,
    clock: &dyn Clock,
    cfg: &ScanConfig,
    progress: &Progress,
    cancel: &AtomicBool,
) -> Result<ScanReport, PipelineError> {
    scan_with(src, store, clock, cfg, progress, cancel, ALGO_VER)
}

/// Som `scan`, men med eksplisitt algoritmeversjon (tester av K7-9).
pub(crate) fn scan_with<H: StoreHandle>(
    src: &dyn AssetSource,
    store: &H,
    clock: &dyn Clock,
    cfg: &ScanConfig,
    progress: &Progress,
    cancel: &AtomicBool,
    algo: crate::model::AlgoVer,
) -> Result<ScanReport, PipelineError> {
    let cx = Ctx {
        src,
        store,
        clock,
        cfg,
        progress,
        cancel,
        started: Instant::now(),
        algo,
    };
    let mut report = ScanReport::default();
    progress.reset();
    store.with_store(|s| s.begin_scan(algo))?;

    // Steg 1: enumerer og avstem.
    progress.set_phase(Phase::Listing);
    let t = Instant::now();
    let token = store
        .with_store(|s| s.setting(CURSOR_KEY))?
        .map(ChangeToken);
    let changes = src.list_changes(token.as_ref())?;
    let ts = clock.now();
    let (stats, purged) = store.with_store(|s| {
        let stats = s.upsert_assets(&changes.changed)?;
        let purged = s.reconcile_removed(&changes.removed, ts)?;
        // Endringene er varige; markøren kan flyttes selv om resten avbrytes.
        s.set_setting(CURSOR_KEY, &changes.next_token.0)?;
        Ok::<_, StoreError>((stats, purged))
    })?;
    report.new = stats.new;
    report.changed = stats.changed;
    report.removed = purged.len() as u64;
    report.timings.enumerate = t.elapsed();

    // Totalt arbeid kjent før start, slik at fremdriften er monoton.
    let (hash_total, work_total) = fast::plan_totals(&cx)?;
    let full = cfg.mode == Mode::Full;
    progress.set_total(hash_total + if full { work_total } else { 0 });

    // Steg 2: rask fase, deretter eksakte grupper uten dekoding.
    let t = Instant::now();
    fast::run(&cx, &mut report)?;
    report.timings.quick = t.elapsed();
    let t = Instant::now();
    groups::refresh_groups(store, clock)?;
    let exact = store.with_store(|s| s.exact_dup_count())?;
    progress.set_found_exact(exact);
    progress.set_phase(Phase::ExactReady);
    report.timings.cluster += t.elapsed();

    // Steg 3 og 4: full fase.
    if full && !cx.stop() {
        progress.set_phase(Phase::Analyzing);
        let t = Instant::now();
        full::run(&cx, &mut report)?;
        report.timings.full = t.elapsed();
        let t = Instant::now();
        groups::refresh_groups(store, clock)?;
        report.timings.cluster += t.elapsed();
    }

    report.cancelled = cancel.load(Ordering::SeqCst);
    report.deadline_hit = !report.cancelled && cx.deadline_hit();
    if !(report.cancelled || report.deadline_hit) {
        progress.finish();
    }
    Ok(report)
}
