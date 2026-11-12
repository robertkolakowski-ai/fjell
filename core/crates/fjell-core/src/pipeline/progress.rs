//! Fremdrift for indeksering (PLAN-KJERNE K7 punkt 1). Del av `pipeline` (L3).
//!
//! Avhenger av: `std`. Grensesnitt:
//! - `Progress`: trådsikker teller (atomics). `done` øker bare (`advance`), `total` settes én gang
//!   per skanning (`set_total`), så `fraction()` er monoton og når 1,0 ved `finish`.
//! - `Progress::with_listener(f, min_interval)`: kaller `f` høyst hvert `min_interval` (standard
//!   250 ms = 4 oppdateringer/s). Første og siste (`finish`) oppdatering sendes alltid.
//! - `Phase`: Listing, Exact (rask fase), ExactReady (eksakte grupper skrevet), Analyzing, Done.

use std::sync::atomic::{AtomicU64, AtomicU8, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Standard minsteavstand mellom oppdateringer (maks 4 per sekund).
pub const DEFAULT_INTERVAL: Duration = Duration::from_millis(250);

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Phase {
    Idle = 0,
    Listing = 1,
    Exact = 2,
    /// Eksakte grupper og forslag er skrevet (før første miniatyr er hentet).
    ExactReady = 3,
    Analyzing = 4,
    Done = 5,
}

impl Phase {
    fn from_u8(v: u8) -> Phase {
        match v {
            1 => Phase::Listing,
            2 => Phase::Exact,
            3 => Phase::ExactReady,
            4 => Phase::Analyzing,
            5 => Phase::Done,
            _ => Phase::Idle,
        }
    }
}

/// Øyeblikksbilde som sendes til lytteren.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Snapshot {
    pub phase: Phase,
    pub done: u64,
    pub total: u64,
    pub found_exact: u64,
}

impl Snapshot {
    /// 0,0..=1,0.
    pub fn fraction(&self) -> f64 {
        if self.total == 0 {
            return if self.phase == Phase::Done { 1.0 } else { 0.0 };
        }
        (self.done.min(self.total) as f64) / self.total as f64
    }
}

type Listener = Box<dyn Fn(&Snapshot) + Send + Sync>;

pub struct Progress {
    phase: AtomicU8,
    done: AtomicU64,
    total: AtomicU64,
    found_exact: AtomicU64,
    listener: Option<Listener>,
    min_interval: Duration,
    last_emit: Mutex<Option<Instant>>,
}

impl Default for Progress {
    fn default() -> Self {
        Progress::new()
    }
}

impl Progress {
    pub fn new() -> Progress {
        Progress {
            phase: AtomicU8::new(0),
            done: AtomicU64::new(0),
            total: AtomicU64::new(0),
            found_exact: AtomicU64::new(0),
            listener: None,
            min_interval: DEFAULT_INTERVAL,
            last_emit: Mutex::new(None),
        }
    }

    pub fn with_listener(
        f: impl Fn(&Snapshot) + Send + Sync + 'static,
        min_interval: Duration,
    ) -> Progress {
        Progress {
            listener: Some(Box::new(f)),
            min_interval,
            ..Progress::new()
        }
    }

    pub fn snapshot(&self) -> Snapshot {
        Snapshot {
            phase: Phase::from_u8(self.phase.load(Ordering::SeqCst)),
            done: self.done.load(Ordering::SeqCst),
            total: self.total.load(Ordering::SeqCst),
            found_exact: self.found_exact.load(Ordering::SeqCst),
        }
    }

    /// Nullstiller for en ny skanning.
    pub(crate) fn reset(&self) {
        self.done.store(0, Ordering::SeqCst);
        self.total.store(0, Ordering::SeqCst);
        self.found_exact.store(0, Ordering::SeqCst);
        self.set_phase(Phase::Idle);
    }

    pub(crate) fn set_total(&self, total: u64) {
        self.total.store(total, Ordering::SeqCst);
        self.emit(true);
    }

    pub(crate) fn set_phase(&self, p: Phase) {
        self.phase.store(p as u8, Ordering::SeqCst);
        self.emit(true);
    }

    pub(crate) fn set_found_exact(&self, n: u64) {
        self.found_exact.store(n, Ordering::SeqCst);
    }

    pub(crate) fn advance(&self, n: u64) {
        self.done.fetch_add(n, Ordering::SeqCst);
        self.emit(false);
    }

    /// Fullført: `done = total` og fase `Done`.
    pub(crate) fn finish(&self) {
        self.done
            .store(self.total.load(Ordering::SeqCst), Ordering::SeqCst);
        self.phase.store(Phase::Done as u8, Ordering::SeqCst);
        self.emit(true);
    }

    fn emit(&self, force: bool) {
        let Some(l) = &self.listener else { return };
        let mut last = self.last_emit.lock().unwrap_or_else(|e| e.into_inner());
        let now = Instant::now();
        if !force && last.is_some_and(|t| now.duration_since(t) < self.min_interval) {
            return;
        }
        *last = Some(now);
        // Øyeblikksbildet tas under låsen, så rekkefølgen lytteren ser er monoton.
        l(&self.snapshot());
    }
}
