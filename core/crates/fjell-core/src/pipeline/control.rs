//! Styring av indeksering (`pipeline`, L3): pause, fortsett, avbryt, strupet bruk, batteripause, frakoblet disk og
//! tidsavbrudd mot trege kilder. Del av `pipeline`.
//!
//! Avhenger av: `model` (`AssetSource`, `IndexPriority`, `SourceError`), `super` (`scan_with`, `Progress`,
//! `StoreHandle`). Ingen plattform og ingen async-runtime (std-tråder, `Condvar` og rayon).
//! Grensesnitt:
//! - `ScanControl`: delt styringsflate for én skanning. Tråder som analyserer parkeres mellom bilder når
//!   skanningen er satt på pause (`Ctx::stop` går via `checkpoint`), og vekkes av `resume`/avbrudd.
//!   `state()` gir `ControlState`; `status(&Snapshot)` gir tilstand, rate og grov gjenstående tid.
//! - `PowerProbe`: strømstatus som kode (`on_battery`). Skallet injiserer den fra `fjell-fs::power`; kjernen
//!   kjenner ingen plattform. `WorkerHook`: kjøres først på hver rayon-tråd (lav OS-prioritet på Windows).
//! - `scan_controlled(..)`: som `scan`, men styrt; med `set_offline_retry(Some(..))` venter den (uten å slette noe) når alle røtter er frakoblet.
//! - `GuardedSource`: omslag som gir kall mot en hengende kilde (nettverksstasjon) et tidsavbrudd og svarer
//!   `Unavailable` i stedet for å henge, og som stopper nye kall når for mange henger.
//!
//! Regler:
//! - Pause og avbrudd skriver aldri til originaler og sletter ingenting. Det som er analysert er lagret per batch;
//!   gjenopptak utledes fra databasen (`work_set`), så ingenting regnes to ganger.
//! - Frakoblet rot (`list_changes` gir `Unavailable`, beslutning 21) betyr «ikke tilgjengelig», aldri «slettet»:
//!   avstemmingen kjøres ikke. Standard: skanningen avslutter med feilkoden. Med `set_offline_retry(Some(..))` venter den
//!   i tilstanden `DiskOffline` og prøver igjen til disken er tilbake eller brukeren avbryter.
//! - «Fortsett» mens PC-en går på batteri overstyrer batteripausen for resten av kjøringen, eller til strømmen
//!   har vært tilbake (docs/v1/INDEKSERING.md). Brukerens uttrykkelige valg veier tyngre enn den automatiske
//!   pausen, og knappen blir aldri en blindvei.

use super::{PipelineError, Progress, ScanConfig, ScanReport, Snapshot, StoreHandle};
use crate::model::{AssetSource, Clock, IndexPriority, SourceError, SourceId, Thumb};
use std::io::Read;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, AtomicU8, AtomicUsize, Ordering};
use std::sync::mpsc::{channel, RecvTimeoutError};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::time::{Duration, Instant};

/// Strømstatus som kode. Implementeres av skallet (Windows: `GetSystemPowerStatus`); må være billig.
pub trait PowerProbe: Send + Sync {
    /// Går PC-en på batteri? Ukjent og stasjonær PC svarer `false` (indekseringen pauses aldri uten sikker kunnskap).
    fn on_battery(&self) -> bool;
}

/// Kjøres først på hver arbeidstråd (f.eks. lav OS-prioritet). Plattformkode injiseres, kjernen har ingen.
pub type WorkerHook = Arc<dyn Fn() + Send + Sync>;

/// Antall tråder en bruker kan velge (samme tak som `IndexingSettings`).
const MAX_THREADS: u32 = 64;
/// Hvor ofte en parkert tråd våkner og ser etter avbrudd og strømendring.
const PARK_SLICE: Duration = Duration::from_millis(100);

/// Hva styringen mener indekseringen gjør nå.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ControlState {
    Idle,
    Running,
    /// Satt på pause av brukeren.
    Paused,
    /// Venter på strøm: PC-en går på batteri og «pause på batteri» er på.
    WaitingForPower,
    /// Alle mapper/disker er frakoblet. Ingenting er slettet; prøver igjen av seg selv.
    DiskOffline,
}

impl ControlState {
    /// Stabil kode (i18n-nøkkel `indexing.state.<kode>`).
    pub fn code(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Running => "running",
            Self::Paused => "paused",
            Self::WaitingForPower => "paused_on_battery",
            Self::DiskOffline => "disk_offline",
        }
    }
}

/// Tilstand pluss tempo.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ControlStatus {
    pub state: ControlState,
    /// Bilder per sekund (glidende snitt); 0 når ukjent.
    pub rate_per_sec: u32,
    /// Grovt anslag av gjenstående tid (sekunder), `None` når det ikke kan anslås.
    pub eta_secs: Option<u64>,
}

#[derive(Default)]
struct Meter {
    last: Option<(Instant, u64)>,
    ema: f64,
}

/// Delt styring for indeksering. Opprettes én gang per `Fjell`.
pub struct ScanControl {
    active: AtomicBool,
    user_paused: AtomicBool,
    battery_override: AtomicBool,
    offline: AtomicBool,
    pause_on_battery: AtomicBool,
    low_priority: AtomicBool,
    threads: AtomicU32,
    probe: Mutex<Option<Arc<dyn PowerProbe>>>,
    probe_cache: Mutex<Option<(Instant, bool)>>,
    probe_interval: Mutex<Duration>,
    hook: Mutex<Option<WorkerHook>>,
    /// `Some`: vent og prøv igjen mot frakoblet disk. `None` (standard): skanningen avslutter med feilen, som før.
    retry: Mutex<Option<Duration>>,
    call_timeout: Mutex<Option<Duration>>,
    timeouts: AtomicU64,
    gate: Mutex<()>,
    cv: Condvar,
    meter: Mutex<Meter>,
}

impl Default for ScanControl {
    fn default() -> Self {
        ScanControl::new()
    }
}

/// Merker skanningen som aktiv mens den lever (også ved panikk).
pub struct ActiveGuard<'a>(&'a ScanControl);

impl Drop for ActiveGuard<'_> {
    fn drop(&mut self) {
        self.0.finish();
    }
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

impl ScanControl {
    /// Standard: lav prioritet, automatisk antall tråder, pause på batteri på, ingen strømkilde (pauser aldri).
    pub fn new() -> ScanControl {
        ScanControl {
            active: AtomicBool::new(false),
            user_paused: AtomicBool::new(false),
            battery_override: AtomicBool::new(false),
            offline: AtomicBool::new(false),
            pause_on_battery: AtomicBool::new(true),
            low_priority: AtomicBool::new(true),
            threads: AtomicU32::new(0),
            probe: Mutex::new(None),
            probe_cache: Mutex::new(None),
            probe_interval: Mutex::new(Duration::from_millis(500)),
            hook: Mutex::new(None),
            retry: Mutex::new(None),
            call_timeout: Mutex::new(None),
            timeouts: AtomicU64::new(0),
            gate: Mutex::new(()),
            cv: Condvar::new(),
            meter: Mutex::new(Meter::default()),
        }
    }

    /// Innstillinger: `threads` 0 = automatisk (halvparten av kjernene, 1–8), ellers høyst 64.
    pub fn set_limits(&self, threads: u32, priority: IndexPriority, pause_on_battery: bool) {
        self.threads
            .store(threads.min(MAX_THREADS), Ordering::SeqCst);
        self.low_priority
            .store(priority == IndexPriority::Low, Ordering::SeqCst);
        self.pause_on_battery
            .store(pause_on_battery, Ordering::SeqCst);
        self.cv.notify_all();
    }

    pub fn set_power_probe(&self, probe: Arc<dyn PowerProbe>) {
        *lock(&self.probe) = Some(probe);
        *lock(&self.probe_cache) = None;
    }

    /// Kroken kjøres på hver ny arbeidstråd når prioriteten er `Low`.
    pub fn set_worker_hook(&self, hook: WorkerHook) {
        *lock(&self.hook) = Some(hook);
    }

    /// Tidsavbrudd per kall mot kilden (`GuardedSource`). `None` = ingen grense.
    pub fn set_call_timeout(&self, t: Option<Duration>) {
        *lock(&self.call_timeout) = t;
    }

    pub fn call_timeout(&self) -> Option<Duration> {
        *lock(&self.call_timeout)
    }

    /// Slår på ventemodus for frakoblet disk: i stedet for å avslutte med `Unavailable` markeres `DiskOffline`
    /// og det prøves igjen etter `d` til disken er tilbake eller kjøringen avbrytes. `None` slår det av (standard).
    pub fn set_offline_retry(&self, d: Option<Duration>) {
        *lock(&self.retry) = d;
    }

    pub fn offline_retry(&self) -> Option<Duration> {
        *lock(&self.retry)
    }

    /// Hvor ofte strømkilden spørres (tester setter null).
    pub fn set_probe_interval(&self, d: Duration) {
        *lock(&self.probe_interval) = d;
        *lock(&self.probe_cache) = None;
    }

    /// Antall kall som er gitt opp på grunn av tidsavbrudd siden start.
    pub fn timeouts(&self) -> u64 {
        self.timeouts.load(Ordering::SeqCst)
    }

    /// Merker en kjøring som aktiv og nullstiller pause og tempo. Kalles før arbeidstråden startes, slik at
    /// statusen aldri viser «ledig» mellom `start_scan` og første fremdrift.
    pub fn begin(&self) -> ActiveGuard<'_> {
        self.start_run();
        ActiveGuard(self)
    }

    /// Som `begin`, men uten vakt: kalleren må kalle `finish` når kjøringen er over (også ved panikk). For
    /// kjøringer som eies av en egen tråd (`Fjell::start`).
    pub fn start_run(&self) {
        self.user_paused.store(false, Ordering::SeqCst);
        self.battery_override.store(false, Ordering::SeqCst);
        self.offline.store(false, Ordering::SeqCst);
        *lock(&self.meter) = Meter::default();
        self.active.store(true, Ordering::SeqCst);
    }

    /// Kjøringen er over: tilstanden er `Idle` igjen.
    pub fn finish(&self) {
        self.active.store(false, Ordering::SeqCst);
        self.offline.store(false, Ordering::SeqCst);
        self.cv.notify_all();
    }

    pub fn is_active(&self) -> bool {
        self.active.load(Ordering::SeqCst)
    }

    /// Setter på pause (brukervalg). Tråder stopper etter bildet de er i gang med.
    pub fn pause(&self) {
        self.user_paused.store(true, Ordering::SeqCst);
    }

    /// Fortsetter. Står indekseringen på pause på grunn av batteri, overstyrer dette batteripausen.
    pub fn resume(&self) {
        let was_battery = self.battery_paused();
        self.user_paused.store(false, Ordering::SeqCst);
        if was_battery {
            self.battery_override.store(true, Ordering::SeqCst);
        }
        self.cv.notify_all();
    }

    /// Våkner parkerte tråder (avbrudd).
    pub fn wake(&self) {
        self.cv.notify_all();
    }

    fn on_battery(&self) -> bool {
        let Some(p) = lock(&self.probe).clone() else {
            return false;
        };
        let every = *lock(&self.probe_interval);
        let mut c = lock(&self.probe_cache);
        if let Some((t, v)) = *c {
            if t.elapsed() < every {
                return v;
            }
        }
        let v = p.on_battery();
        *c = Some((Instant::now(), v));
        v
    }

    fn battery_paused(&self) -> bool {
        if !self.pause_on_battery.load(Ordering::SeqCst) {
            return false;
        }
        let batt = self.on_battery();
        if !batt {
            // Strømmen er tilbake: neste batteriperiode pauser igjen.
            self.battery_override.store(false, Ordering::SeqCst);
            return false;
        }
        !self.battery_override.load(Ordering::SeqCst)
    }

    /// Nåværende tilstand.
    pub fn state(&self) -> ControlState {
        if !self.is_active() {
            return ControlState::Idle;
        }
        if self.offline.load(Ordering::SeqCst) {
            return ControlState::DiskOffline;
        }
        if self.user_paused.load(Ordering::SeqCst) {
            return ControlState::Paused;
        }
        if self.battery_paused() {
            return ControlState::WaitingForPower;
        }
        ControlState::Running
    }

    fn halted(&self) -> bool {
        self.user_paused.load(Ordering::SeqCst) || self.battery_paused()
    }

    /// Parkerer kallende tråd så lenge skanningen står på pause. Returnerer straks ved avbrudd eller frist.
    pub fn checkpoint(&self, cancel: &AtomicBool, expired: impl Fn() -> bool) {
        while self.halted() && !cancel.load(Ordering::SeqCst) && !expired() {
            let g = lock(&self.gate);
            let _ = self
                .cv
                .wait_timeout(g, PARK_SLICE)
                .unwrap_or_else(|e| e.into_inner());
        }
    }

    /// Venter mens skanningen står på pause (brukes mellom skanning og analysatorer).
    pub fn wait_while_paused(&self, cancel: &AtomicBool) {
        self.checkpoint(cancel, || false);
    }

    /// Antall tråder. `all` = tilgjengelig parallellitet. Automatisk lar maskinen være brukbar.
    pub fn threads(&self, all: usize) -> usize {
        match self.threads.load(Ordering::SeqCst) {
            0 => (all / 2).clamp(1, 8),
            n => n as usize,
        }
    }

    /// Bygger rayon-poolen. Ved lav prioritet kjører `WorkerHook` først på hver tråd.
    pub fn build_pool(&self, threads: usize) -> Result<rayon::ThreadPool, PipelineError> {
        let hook = if self.low_priority.load(Ordering::SeqCst) {
            lock(&self.hook).clone()
        } else {
            None
        };
        let mut b = rayon::ThreadPoolBuilder::new().num_threads(threads);
        if let Some(h) = hook {
            b = b.start_handler(move |_| h());
        }
        b.build().map_err(|e| PipelineError::Pool(e.to_string()))
    }

    fn set_offline(&self, v: bool) {
        self.offline.store(v, Ordering::SeqCst);
    }

    /// Venter til neste forsøk mot en frakoblet disk. `false` = avbrutt (slutt).
    fn wait_retry(&self, cancel: &AtomicBool, every: Duration) -> bool {
        let until = Instant::now() + every;
        while Instant::now() < until {
            if cancel.load(Ordering::SeqCst) {
                return false;
            }
            let g = lock(&self.gate);
            let _ = self
                .cv
                .wait_timeout(g, PARK_SLICE.min(until - Instant::now()))
                .unwrap_or_else(|e| e.into_inner());
        }
        !cancel.load(Ordering::SeqCst)
    }

    /// Tilstand, tempo og grov gjenstående tid. Kalles av status-polling (hvert par sekunder).
    pub fn status(&self, snap: &Snapshot) -> ControlStatus {
        let state = self.state();
        let mut m = lock(&self.meter);
        if state != ControlState::Running {
            // Tid på pause teller ikke med i tempoet.
            m.last = None;
            let rate = if state == ControlState::Idle {
                0.0
            } else {
                m.ema
            };
            return ControlStatus {
                state,
                rate_per_sec: rate.round() as u32,
                eta_secs: None,
            };
        }
        let now = Instant::now();
        match m.last {
            None => m.last = Some((now, snap.done)),
            Some((t, d)) => {
                let dt = now.duration_since(t).as_secs_f64();
                if dt >= 0.2 && snap.done >= d {
                    let inst = (snap.done - d) as f64 / dt;
                    m.ema = if m.ema <= 0.0 {
                        inst
                    } else {
                        0.7 * m.ema + 0.3 * inst
                    };
                    m.last = Some((now, snap.done));
                }
            }
        }
        let remaining = snap.total.saturating_sub(snap.done);
        let eta = (m.ema >= 0.5 && remaining > 0).then(|| rough((remaining as f64 / m.ema) as u64));
        ControlStatus {
            state,
            rate_per_sec: m.ema.round() as u32,
            eta_secs: eta,
        }
    }
}

/// Grovt anslag: hele 10 sekunder under 10 minutter, ellers hele minutter (aldri falsk presisjon).
fn rough(secs: u64) -> u64 {
    if secs < 600 {
        secs.div_ceil(10) * 10
    } else {
        secs.div_ceil(60) * 60
    }
}

/// Som `scan`, men styrt av `ctl`. Står alle røtter frakoblet (`list_changes` gir `Unavailable`), markeres
/// tilstanden `DiskOffline` og det prøves igjen til disken er tilbake eller `cancel` settes. Ingenting regnes
/// som slettet (avstemmingen kjøres ikke). Avbrudd gir en rapport med `cancelled`.
pub fn scan_controlled<H: StoreHandle>(
    src: &dyn AssetSource,
    store: &H,
    clock: &dyn Clock,
    cfg: &ScanConfig,
    progress: &Progress,
    cancel: &AtomicBool,
    ctl: &ScanControl,
) -> Result<ScanReport, PipelineError> {
    loop {
        let r = super::scan_with(
            src,
            store,
            clock,
            cfg,
            progress,
            cancel,
            crate::model::ALGO_VER,
            Some(ctl),
        );
        match r {
            Err(PipelineError::Source(SourceError::Unavailable))
                if ctl.offline_retry().is_some() =>
            {
                let every = ctl.offline_retry().unwrap_or(Duration::from_secs(5));
                ctl.set_offline(true);
                if !ctl.wait_retry(cancel, every) {
                    ctl.set_offline(false);
                    return Ok(ScanReport {
                        cancelled: true,
                        ..ScanReport::default()
                    });
                }
            }
            other => {
                ctl.set_offline(false);
                return other;
            }
        }
    }
}

/// Omslag som gir kall mot kilden et tidsavbrudd. Kallet kjører på en egen tråd; når fristen er ute (eller
/// `cancel` settes) svarer omslaget `Unavailable` straks, så skanningen ikke henger. Den fastlåste tråden kan
/// ikke drepes (std har ingen måte), men den eier bare sitt eget kall og avsluttes når kilden svarer. Er
/// `max_stuck` kall fastlåst samtidig, nektes nye kall (`Unavailable`) til noen har kommet tilbake, så antall
/// hengende tråder aldri vokser uten grense. `list_changes` kjøres uten tidsavbrudd (full opplisting kan være
/// lang), men hører til samme avbruddsflagg via skanningen.
pub struct GuardedSource {
    inner: Arc<dyn AssetSource>,
    ctl: Arc<ScanControl>,
    cancel: Arc<AtomicBool>,
    stuck: Arc<AtomicUsize>,
    max_stuck: usize,
}

impl GuardedSource {
    pub fn new(
        inner: Arc<dyn AssetSource>,
        ctl: Arc<ScanControl>,
        cancel: Arc<AtomicBool>,
        max_stuck: usize,
    ) -> GuardedSource {
        GuardedSource {
            inner,
            ctl,
            cancel,
            stuck: Arc::new(AtomicUsize::new(0)),
            max_stuck: max_stuck.max(1),
        }
    }

    /// Antall kall som henger nå.
    pub fn stuck(&self) -> usize {
        self.stuck.load(Ordering::SeqCst)
    }

    fn guarded<T: Send + 'static>(
        &self,
        f: impl FnOnce(Arc<dyn AssetSource>) -> Result<T, SourceError> + Send + 'static,
    ) -> Result<T, SourceError> {
        let Some(limit) = self.ctl.call_timeout() else {
            return f(self.inner.clone());
        };
        if self.stuck.load(Ordering::SeqCst) >= self.max_stuck {
            return Err(SourceError::Unavailable);
        }
        let (tx, rx) = channel();
        let inner = self.inner.clone();
        let stuck = self.stuck.clone();
        // 0 = kjører, 1 = svaret er levert, 2 = oppgitt (ingen venter lenger).
        let state = Arc::new(AtomicU8::new(0));
        let st = state.clone();
        let spawned = std::thread::Builder::new()
            .name("fjell-source-call".into())
            .spawn(move || {
                let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| f(inner)));
                if st
                    .compare_exchange(0, 1, Ordering::SeqCst, Ordering::SeqCst)
                    .is_ok()
                {
                    let _ = tx.send(r);
                } else {
                    // Ingen venter lenger: kallet er ferdig, tråden slippes.
                    stuck.fetch_sub(1, Ordering::SeqCst);
                }
            });
        if spawned.is_err() {
            return Err(SourceError::Unavailable);
        }
        let start = Instant::now();
        loop {
            let slice = PARK_SLICE.min(limit.saturating_sub(start.elapsed()));
            match rx.recv_timeout(slice) {
                Ok(Ok(r)) => return r,
                // Panikken sendes videre, slik at `catch_unwind` i pipeline teller den som før.
                Ok(Err(p)) => std::panic::resume_unwind(p),
                Err(RecvTimeoutError::Disconnected) => return Err(SourceError::Unavailable),
                Err(RecvTimeoutError::Timeout) => {
                    let cancelled = self.cancel.load(Ordering::SeqCst);
                    if cancelled || start.elapsed() >= limit {
                        if !cancelled {
                            self.ctl.timeouts.fetch_add(1, Ordering::SeqCst);
                        }
                        // Tråden regnes som fastlåst til den faktisk er ferdig.
                        self.stuck.fetch_add(1, Ordering::SeqCst);
                        if state
                            .compare_exchange(0, 2, Ordering::SeqCst, Ordering::SeqCst)
                            .is_ok()
                        {
                            return Err(SourceError::Unavailable);
                        }
                        // Svaret kom akkurat nå: bruk det (sendingen er i gang).
                        self.stuck.fetch_sub(1, Ordering::SeqCst);
                        return match rx.recv() {
                            Ok(Ok(r)) => r,
                            Ok(Err(p)) => std::panic::resume_unwind(p),
                            Err(_) => Err(SourceError::Unavailable),
                        };
                    }
                }
            }
        }
    }
}

impl AssetSource for GuardedSource {
    fn list_changes(
        &self,
        since: Option<&crate::model::ChangeToken>,
    ) -> Result<crate::model::ChangeSet, SourceError> {
        self.inner.list_changes(since)
    }

    fn thumbnail(&self, id: &SourceId) -> Result<Thumb, SourceError> {
        let id = id.clone();
        self.guarded(move |s| s.thumbnail(&id))
    }

    fn read_bytes(&self, id: &SourceId) -> Result<Box<dyn Read + Send>, SourceError> {
        let id = id.clone();
        self.guarded(move |s| s.read_bytes(&id))
    }
}

#[cfg(test)]
mod tests;
