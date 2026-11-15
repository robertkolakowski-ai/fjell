//! Tester for indekseringsstyringen: pause midt i batch, gjenopptak uten dobbeltarbeid, avbrudd, batteri,
//! frakoblet disk og tidsavbrudd mot hengende kilder. Kjørt på Linux med syntetiske kilder.

use super::super::tests::{dump, scene, Counting, Fx};
use super::*;
use crate::model::{
    AssetSource, ChangeSet, ChangeToken, IndexPriority, SourceError, SourceId, Thumb,
};
use crate::pipeline::Phase;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering::SeqCst};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

const N: usize = 60;

fn wait_until(what: &str, mut f: impl FnMut() -> bool) {
    let t = Instant::now();
    while !f() {
        assert!(t.elapsed() < Duration::from_secs(20), "tidsavbrudd: {what}");
        thread::sleep(Duration::from_millis(5));
    }
}

fn fixture(n: usize) -> Fx {
    let mut fx = Fx::new();
    for i in 0..n {
        fx.uniq(&format!("u{i:03}.jpg"), scene(10 + i as u64));
    }
    fx
}

fn active(fx: &Fx) -> i64 {
    fx.with(|s| {
        s.conn()
            .query_row("SELECT COUNT(*) FROM asset WHERE state = 0", [], |r| {
                r.get(0)
            })
            .unwrap()
    })
}

fn analyzed(fx: &Fx) -> i64 {
    fx.with(|s| {
        s.conn()
            .query_row(
                "SELECT COUNT(*) FROM fingerprint WHERE algo_ver > 0 AND dhash IS NOT NULL",
                [],
                |r| r.get(0),
            )
            .unwrap()
    })
}

fn ctl2() -> Arc<ScanControl> {
    let c = Arc::new(ScanControl::new());
    c.set_limits(2, IndexPriority::Low, true);
    c.set_probe_interval(Duration::ZERO);
    c
}

fn run(fx: &Fx, ctl: &ScanControl, p: &Progress) -> ScanReport {
    let _g = ctl.begin();
    scan_controlled(
        fx.src.as_ref(),
        fx.store.as_ref(),
        &fx.clock,
        &ScanConfig::full(),
        p,
        &fx.cancel,
        ctl,
    )
    .unwrap()
}

struct Probe(AtomicBool);
impl PowerProbe for Probe {
    fn on_battery(&self) -> bool {
        self.0.load(SeqCst)
    }
}

/// Pause midt i en batch: trådene stopper etter bildet de er i, ingenting analyseres mens det står på pause,
/// og fortsett gir nøyaktig samme resultat som en uavbrutt kjøring, med hvert bilde regnet én gang.
#[test]
fn pause_mid_batch_then_resume_finishes_without_rework() {
    let baseline = {
        let fx = fixture(N);
        fx.scan(&ScanConfig::full());
        dump(&fx)
    };
    let fx = fixture(N);
    let ctl = ctl2();
    let p = Progress::new();
    let c2 = ctl.clone();
    *fx.src.hook.lock().unwrap() = Some(Arc::new(Box::new(move |n| {
        if n == 7 {
            c2.pause();
        }
    })));
    thread::scope(|sc| {
        let h = sc.spawn(|| run(&fx, &ctl, &p));
        wait_until("pause", || ctl.state() == ControlState::Paused);
        // La pågående bilder bli ferdige, og se at det så står stille.
        thread::sleep(Duration::from_millis(300));
        let t1 = fx.src.counts().0;
        let d1 = p.snapshot().done;
        thread::sleep(Duration::from_millis(400));
        assert_eq!(fx.src.counts().0, t1, "ingen nye bilder mens den er pauset");
        assert_eq!(p.snapshot().done, d1);
        assert!((8..=10).contains(&t1), "stopper straks: {t1}");
        // Ferdig arbeid er skrevet selv om batchen ikke er full (pause holder ikke igjen data i minnet).
        wait_until("flush ved pause", || analyzed(&fx) as usize == t1);
        assert_eq!(ctl.state(), ControlState::Paused);
        ctl.resume();
        let r = h.join().unwrap();
        assert!(!r.cancelled);
    });
    assert_eq!(fx.src.counts().0, N, "hvert bilde hentes én gang");
    assert_eq!(analyzed(&fx), N as i64);
    assert_eq!(dump(&fx), baseline);
    assert_eq!(ctl.state(), ControlState::Idle);
}

/// Avbrudd mens den står på pause returnerer straks, og neste kjøring gjør bare resten.
#[test]
fn cancel_while_paused_then_rerun_only_does_the_rest() {
    let fx = fixture(N);
    let ctl = ctl2();
    let p = Progress::new();
    let c2 = ctl.clone();
    *fx.src.hook.lock().unwrap() = Some(Arc::new(Box::new(move |n| {
        if n == 10 {
            c2.pause();
        }
    })));
    let rep = thread::scope(|sc| {
        let h = sc.spawn(|| run(&fx, &ctl, &p));
        wait_until("pause", || ctl.state() == ControlState::Paused);
        let t = Instant::now();
        fx.cancel.store(true, SeqCst);
        let r = h.join().unwrap();
        assert!(t.elapsed() < Duration::from_secs(2), "avbrudd er prompte");
        r
    });
    assert!(rep.cancelled);
    let done_first = analyzed(&fx) as usize;
    assert!((10..N).contains(&done_first));
    assert_eq!(active(&fx), N as i64, "avbrudd sletter ingenting");
    // Ny kjøring (ingen pause): resten, uten dobbeltarbeid.
    *fx.src.hook.lock().unwrap() = None;
    fx.cancel.store(false, SeqCst);
    let before = fx.src.counts().0;
    let r2 = run(&fx, &ctl, &Progress::new());
    assert!(!r2.cancelled);
    assert_eq!(fx.src.counts().0 - before, N - done_first);
    assert_eq!(analyzed(&fx), N as i64);
}

/// Batteri: indekseringen venter på strøm, og går videre av seg selv når strømmen er tilbake.
#[test]
fn battery_pauses_and_ac_resumes_by_itself() {
    let fx = fixture(20);
    let ctl = ctl2();
    let probe = Arc::new(Probe(AtomicBool::new(true)));
    ctl.set_power_probe(probe.clone());
    let p = Progress::new();
    thread::scope(|sc| {
        let h = sc.spawn(|| run(&fx, &ctl, &p));
        wait_until("venter på strøm", || {
            ctl.state() == ControlState::WaitingForPower
        });
        thread::sleep(Duration::from_millis(300));
        assert_eq!(fx.src.counts().0, 0, "ingenting analyseres på batteri");
        assert_eq!(
            ctl.status(&p.snapshot()).state,
            ControlState::WaitingForPower
        );
        probe.0.store(false, SeqCst);
        let r = h.join().unwrap();
        assert!(!r.cancelled);
    });
    assert_eq!(analyzed(&fx), 20);
}

/// «Pause på batteri» av: ingen pause. «Fortsett» på batteri overstyrer pausen for resten av kjøringen.
#[test]
fn battery_setting_off_never_pauses_and_resume_overrides() {
    // Av.
    let fx = fixture(15);
    let ctl = ctl2();
    ctl.set_limits(2, IndexPriority::Low, false);
    ctl.set_power_probe(Arc::new(Probe(AtomicBool::new(true))));
    run(&fx, &ctl, &Progress::new());
    assert_eq!(analyzed(&fx), 15);

    // På, men brukeren trykker Fortsett.
    let fx = fixture(15);
    let ctl = ctl2();
    ctl.set_power_probe(Arc::new(Probe(AtomicBool::new(true))));
    thread::scope(|sc| {
        let h = sc.spawn(|| run(&fx, &ctl, &Progress::new()));
        wait_until("venter", || ctl.state() == ControlState::WaitingForPower);
        ctl.resume();
        assert_eq!(ctl.state(), ControlState::Running);
        h.join().unwrap();
    });
    assert_eq!(analyzed(&fx), 15);
}

/// Strømmen kommer tilbake og forsvinner igjen: overstyringen gjelder bare den ene batteriperioden.
#[test]
fn override_is_forgotten_when_power_returns() {
    let ctl = ctl2();
    let probe = Arc::new(Probe(AtomicBool::new(true)));
    ctl.set_power_probe(probe.clone());
    let _g = ctl.begin();
    assert_eq!(ctl.state(), ControlState::WaitingForPower);
    ctl.resume();
    assert_eq!(ctl.state(), ControlState::Running);
    probe.0.store(false, SeqCst);
    assert_eq!(ctl.state(), ControlState::Running);
    probe.0.store(true, SeqCst);
    assert_eq!(ctl.state(), ControlState::WaitingForPower);
}

/// Kilde som kan «trekkes ut» som en ekstern disk.
struct Flaky {
    inner: Arc<Counting>,
    offline: AtomicBool,
}

impl AssetSource for Flaky {
    fn list_changes(&self, since: Option<&ChangeToken>) -> Result<ChangeSet, SourceError> {
        if self.offline.load(SeqCst) {
            return Err(SourceError::Unavailable);
        }
        self.inner.list_changes(since)
    }
    fn thumbnail(&self, id: &SourceId) -> Result<Thumb, SourceError> {
        self.inner.thumbnail(id)
    }
    fn read_bytes(&self, id: &SourceId) -> Result<Box<dyn std::io::Read + Send>, SourceError> {
        self.inner.read_bytes(id)
    }
}

/// Frakoblet rot: tilstanden er `DiskOffline`, ingenting regnes som slettet, og kjøringen fortsetter av seg
/// selv når disken er tilbake.
#[test]
fn offline_root_waits_without_deleting_and_resumes_when_back() {
    let fx = fixture(30);
    fx.scan(&ScanConfig::quick());
    assert_eq!(active(&fx), 30);
    let flaky = Flaky {
        inner: fx.src.clone(),
        offline: AtomicBool::new(true),
    };
    let ctl = ctl2();
    ctl.set_offline_retry(Some(Duration::from_millis(20)));
    let p = Progress::new();
    thread::scope(|sc| {
        let h = sc.spawn(|| {
            let _g = ctl.begin();
            scan_controlled(
                &flaky,
                fx.store.as_ref(),
                &fx.clock,
                &ScanConfig::full(),
                &p,
                &fx.cancel,
                &ctl,
            )
            .unwrap()
        });
        wait_until("frakoblet", || ctl.state() == ControlState::DiskOffline);
        assert_eq!(ctl.state().code(), "disk_offline");
        assert_eq!(active(&fx), 30, "frakoblet disk er ikke slettede bilder");
        thread::sleep(Duration::from_millis(100));
        assert_eq!(ctl.state(), ControlState::DiskOffline);
        flaky.offline.store(false, SeqCst);
        let r = h.join().unwrap();
        assert!(!r.cancelled);
    });
    assert_eq!(active(&fx), 30);
    assert_eq!(analyzed(&fx), 30);
}

/// Avbrudd mens disken er ute gir en rapport og ingen hengende tråd.
#[test]
fn cancel_while_offline_returns_a_cancelled_report() {
    let fx = fixture(5);
    fx.scan(&ScanConfig::quick());
    let flaky = Flaky {
        inner: fx.src.clone(),
        offline: AtomicBool::new(true),
    };
    let ctl = ctl2();
    ctl.set_offline_retry(Some(Duration::from_secs(30)));
    let r = thread::scope(|sc| {
        let h = sc.spawn(|| {
            let _g = ctl.begin();
            scan_controlled(
                &flaky,
                fx.store.as_ref(),
                &fx.clock,
                &ScanConfig::full(),
                &Progress::new(),
                &fx.cancel,
                &ctl,
            )
            .unwrap()
        });
        wait_until("frakoblet", || ctl.state() == ControlState::DiskOffline);
        let t = Instant::now();
        fx.cancel.store(true, SeqCst);
        let r = h.join().unwrap();
        assert!(t.elapsed() < Duration::from_secs(2));
        r
    });
    assert!(r.cancelled);
    assert_eq!(active(&fx), 5);
    assert_eq!(ctl.state(), ControlState::Idle);
}

#[test]
fn threads_follow_settings_and_automatic_leaves_room() {
    let c = ScanControl::new();
    assert_eq!(c.threads(8), 4);
    assert_eq!(c.threads(2), 1);
    assert_eq!(c.threads(1), 1);
    assert_eq!(c.threads(64), 8, "automatisk har tak");
    c.set_limits(3, IndexPriority::Normal, true);
    assert_eq!(c.threads(16), 3);
    c.set_limits(1000, IndexPriority::Normal, true);
    assert_eq!(c.threads(16), 64);
}

#[test]
fn worker_hook_runs_on_pool_threads_only_for_low_priority() {
    let calls = Arc::new(AtomicUsize::new(0));
    let c = ScanControl::new();
    let k = calls.clone();
    c.set_worker_hook(Arc::new(move || {
        k.fetch_add(1, SeqCst);
    }));
    c.set_limits(3, IndexPriority::Low, true);
    let pool = c.build_pool(3).unwrap();
    pool.install(|| ());
    wait_until("kroken kjørte på alle tråder", || {
        calls.load(SeqCst) == 3
    });
    drop(pool);
    c.set_limits(3, IndexPriority::Normal, true);
    let pool = c.build_pool(2).unwrap();
    pool.install(|| ());
    thread::sleep(Duration::from_millis(100));
    assert_eq!(calls.load(SeqCst), 3, "normal prioritet rører ikke trådene");
}

#[test]
fn eta_is_rough_and_paused_time_does_not_count() {
    let c = ScanControl::new();
    let _g = c.begin();
    let mut snap = Snapshot {
        phase: Phase::Analyzing,
        done: 0,
        total: 10_000,
        found_exact: 0,
    };
    assert_eq!(c.status(&snap).eta_secs, None, "ingen måling ennå");
    thread::sleep(Duration::from_millis(250));
    snap.done = 500;
    let s = c.status(&snap);
    assert_eq!(s.state, ControlState::Running);
    assert!(s.rate_per_sec >= 1000, "{}", s.rate_per_sec);
    let eta = s.eta_secs.unwrap();
    assert!(eta.is_multiple_of(10) && eta > 0);
    c.pause();
    let p = c.status(&snap);
    assert_eq!((p.state, p.eta_secs), (ControlState::Paused, None));
    assert_eq!(rough(7), 10);
    assert_eq!(rough(601), 660);
}

// --- GuardedSource ---

/// Kilde der `thumbnail` henger til den slippes.
struct Hang {
    release: Arc<AtomicBool>,
    calls: AtomicUsize,
    hang_on: Mutex<Option<SourceId>>,
    inner: Arc<Counting>,
}

impl AssetSource for Hang {
    fn list_changes(&self, since: Option<&ChangeToken>) -> Result<ChangeSet, SourceError> {
        self.inner.list_changes(since)
    }
    fn thumbnail(&self, id: &SourceId) -> Result<Thumb, SourceError> {
        self.calls.fetch_add(1, SeqCst);
        let hang = self
            .hang_on
            .lock()
            .unwrap()
            .as_ref()
            .is_none_or(|h| h == id);
        while hang && !self.release.load(SeqCst) {
            thread::sleep(Duration::from_millis(5));
        }
        self.inner.thumbnail(id)
    }
    fn read_bytes(&self, id: &SourceId) -> Result<Box<dyn std::io::Read + Send>, SourceError> {
        self.inner.read_bytes(id)
    }
}

fn hang_fixture(n: usize) -> (Fx, Arc<Hang>, Arc<ScanControl>, Arc<AtomicBool>) {
    let fx = fixture(n);
    let hang = Arc::new(Hang {
        release: Arc::new(AtomicBool::new(false)),
        calls: AtomicUsize::new(0),
        hang_on: Mutex::new(None),
        inner: fx.src.clone(),
    });
    let ctl = ctl2();
    ctl.set_call_timeout(Some(Duration::from_millis(80)));
    (fx, hang, ctl, Arc::new(AtomicBool::new(false)))
}

#[test]
fn hanging_call_times_out_with_a_code_and_the_thread_is_released_later() {
    let (_fx, hang, ctl, cancel) = hang_fixture(1);
    let g = GuardedSource::new(hang.clone(), ctl.clone(), cancel, 4);
    let t = Instant::now();
    let r = g.thumbnail(&SourceId("u000.jpg".into()));
    assert_eq!(r.unwrap_err(), SourceError::Unavailable);
    assert!(t.elapsed() >= Duration::from_millis(80) && t.elapsed() < Duration::from_secs(2));
    assert_eq!(ctl.timeouts(), 1);
    assert_eq!(g.stuck(), 1);
    hang.release.store(true, SeqCst);
    wait_until("tråden er sluppet", || g.stuck() == 0);
    // Raske kall går som vanlig etterpå.
    assert!(g.thumbnail(&SourceId("u000.jpg".into())).is_ok());
}

#[test]
fn too_many_stuck_calls_stop_new_calls_so_threads_never_pile_up() {
    let (_fx, hang, ctl, cancel) = hang_fixture(5);
    let g = GuardedSource::new(hang.clone(), ctl.clone(), cancel, 2);
    for i in 0..2 {
        assert!(g.thumbnail(&SourceId(format!("u{i:03}.jpg"))).is_err());
    }
    assert_eq!(g.stuck(), 2);
    let calls = hang.calls.load(SeqCst);
    let t = Instant::now();
    for i in 2..5 {
        assert_eq!(
            g.thumbnail(&SourceId(format!("u{i:03}.jpg"))).unwrap_err(),
            SourceError::Unavailable
        );
    }
    assert!(
        t.elapsed() < Duration::from_millis(50),
        "nektes uten å vente"
    );
    assert_eq!(
        hang.calls.load(SeqCst),
        calls,
        "ingen nye tråder mot en død disk"
    );
    hang.release.store(true, SeqCst);
    wait_until("alle sluppet", || g.stuck() == 0);
}

#[test]
fn cancel_returns_from_a_hanging_call_promptly_without_counting_a_timeout() {
    let (_fx, hang, ctl, cancel) = hang_fixture(1);
    ctl.set_call_timeout(Some(Duration::from_secs(60)));
    let g = GuardedSource::new(hang.clone(), ctl.clone(), cancel.clone(), 4);
    thread::scope(|sc| {
        let h = sc.spawn(|| g.thumbnail(&SourceId("u000.jpg".into())));
        thread::sleep(Duration::from_millis(50));
        let t = Instant::now();
        cancel.store(true, SeqCst);
        assert_eq!(h.join().unwrap().unwrap_err(), SourceError::Unavailable);
        assert!(t.elapsed() < Duration::from_secs(1));
    });
    assert_eq!(ctl.timeouts(), 0);
    hang.release.store(true, SeqCst);
    wait_until("sluppet", || g.stuck() == 0);
}

#[test]
fn panics_in_the_source_still_reach_the_pipeline() {
    let fx = fixture(3);
    let ctl = ctl2();
    ctl.set_call_timeout(Some(Duration::from_secs(5)));
    fx.src
        .panic_on
        .lock()
        .unwrap()
        .insert(SourceId("u001.jpg".into()));
    let g = GuardedSource::new(fx.src.clone(), ctl.clone(), fx.cancel.clone(), 4);
    let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        g.thumbnail(&SourceId("u001.jpg".into()))
    }));
    assert!(r.is_err());
    assert!(g.thumbnail(&SourceId("u000.jpg".into())).is_ok());
}

/// Hele skanningen over en kilde der ett bilde henger: den blir ferdig, bildet telles som utilgjengelig (ikke
/// som feil), og ingen tråd henger igjen etter at kilden våkner.
#[test]
fn scan_survives_one_hanging_file() {
    let (fx, hang, ctl, _c) = hang_fixture(12);
    *hang.hang_on.lock().unwrap() = Some(SourceId("u005.jpg".into()));
    let g = GuardedSource::new(hang.clone(), ctl.clone(), fx.cancel.clone(), 4);
    let p = Progress::new();
    let _a = ctl.begin();
    let t = Instant::now();
    let r = scan_controlled(
        &g,
        fx.store.as_ref(),
        &fx.clock,
        &ScanConfig::full(),
        &p,
        &fx.cancel,
        &ctl,
    )
    .unwrap();
    assert!(t.elapsed() < Duration::from_secs(10));
    assert_eq!(r.skipped_unavailable, 1);
    assert!(
        r.failures.is_empty(),
        "tidsavbrudd er ikke en feil som teller mot bildet"
    );
    assert_eq!(r.analyzed, 11);
    assert_eq!(ctl.timeouts(), 1);
    hang.release.store(true, SeqCst);
    wait_until("ingen hengende tråder", || g.stuck() == 0);
    // Neste kjøring tar det ene bildet som mangler.
    let r2 = scan_controlled(
        &g,
        fx.store.as_ref(),
        &fx.clock,
        &ScanConfig::full(),
        &Progress::new(),
        &fx.cancel,
        &ctl,
    )
    .unwrap();
    assert_eq!(r2.analyzed, 1);
    assert_eq!(analyzed(&fx), 12);
}

/// Standard: uten ventemodus avslutter skanningen med feilkoden (ingenting slettes), som før.
#[test]
fn offline_root_ends_with_the_code_by_default_and_deletes_nothing() {
    let fx = fixture(6);
    fx.scan(&ScanConfig::quick());
    let flaky = Flaky {
        inner: fx.src.clone(),
        offline: AtomicBool::new(true),
    };
    let ctl = ctl2();
    let _g = ctl.begin();
    let r = scan_controlled(
        &flaky,
        fx.store.as_ref(),
        &fx.clock,
        &ScanConfig::full(),
        &Progress::new(),
        &fx.cancel,
        &ctl,
    );
    assert!(matches!(
        r,
        Err(PipelineError::Source(SourceError::Unavailable))
    ));
    assert_eq!(active(&fx), 6);
}
