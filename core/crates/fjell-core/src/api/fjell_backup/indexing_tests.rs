//! Indekseringsstyring gjennom `Fjell` (`FjellApi::indexing_*`): standardverdier, innstillinger, pause/fortsett/avbryt
//! mot en ekte skanning, batteri via injisert strømkilde og responstid under pågående indeksering mot et
//! syntetisk stort bibliotek. Kjørt på Linux; strømstatus og trådprioritet på Windows er ikke verifisert.

use super::*;
use crate::api::{FjellApi, ScanConfig, ScanMode, SuggestionKind, MAX_INDEX_THREADS};
use crate::model::{FakeClock, IndexPriority, IndexState};
use crate::pipeline::tests::{scene, Counting, Fx};
use crate::store::Store;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering::SeqCst};
use std::time::{Duration, Instant};

struct Probe(AtomicBool);
impl PowerProbe for Probe {
    fn on_battery(&self) -> bool {
        self.0.load(SeqCst)
    }
}

fn library(n: usize) -> (Fjell, Arc<Counting>) {
    let mut fx = Fx::new();
    for i in 0..n {
        fx.uniq(&format!("p{i:05}.jpg"), scene(1 + (i % 400) as u64));
    }
    let src = fx.src.clone();
    let f = Fjell::new(
        Store::open_in_memory().unwrap(),
        src.clone(),
        Arc::new(FakeClock::new(5_000_000)),
    );
    f.indexing_control()
        .set_probe_interval(Duration::from_millis(0));
    (f, src)
}

fn start(f: &Fjell) {
    f.start_scan(ScanConfig {
        mode: ScanMode::Full,
        priority: 1,
        deadline: None,
    })
    .unwrap();
}

fn wait_until(what: &str, mut cond: impl FnMut() -> bool) {
    let t = Instant::now();
    while !cond() {
        assert!(t.elapsed() < Duration::from_secs(60), "tidsavbrudd: {what}");
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[test]
fn idle_status_and_control_errors_are_codes() {
    let (f, _) = library(3);
    let s = f.indexing_status().unwrap();
    assert_eq!(
        (s.state, s.done, s.total, s.eta_secs),
        (IndexState::Idle, 0, 0, None)
    );
    for r in [f.indexing_pause(), f.indexing_resume(), f.indexing_cancel()] {
        assert_eq!(
            r.unwrap_err(),
            ApiError::Internal("indexing_not_running".into())
        );
    }
}

#[test]
fn settings_default_roundtrip_validate_and_persist() {
    let (f, _) = library(1);
    assert_eq!(f.indexing_settings().unwrap(), IndexingSettings::default());
    let s = IndexingSettings {
        threads: 3,
        priority: IndexPriority::Normal,
        pause_on_battery: false,
    };
    f.indexing_settings_set(s).unwrap();
    assert_eq!(f.indexing_settings().unwrap(), s);
    let bad = IndexingSettings {
        threads: MAX_INDEX_THREADS + 1,
        ..s
    };
    assert_eq!(
        f.indexing_settings_set(bad).unwrap_err(),
        ApiError::Internal("indexing_invalid_settings".into())
    );
    assert_eq!(
        f.indexing_settings().unwrap(),
        s,
        "ugyldig verdi endrer ingenting"
    );
}

#[test]
fn settings_reach_the_running_control() {
    let (f, _) = library(1);
    f.indexing_settings_set(IndexingSettings {
        threads: 5,
        priority: IndexPriority::Low,
        pause_on_battery: true,
    })
    .unwrap();
    assert_eq!(f.indexing_control().threads(16), 5);
}

#[test]
fn pause_resume_and_finish_through_the_api() {
    let (f, src) = library(80);
    f.indexing_settings_set(IndexingSettings {
        threads: 2,
        ..IndexingSettings::default()
    })
    .unwrap();
    start(&f);
    // Rett etter start er den aktiv, aldri «ledig».
    assert_ne!(f.indexing_status().unwrap().state, IndexState::Idle);
    f.indexing_pause().unwrap();
    wait_until("pauset", || {
        f.indexing_status().unwrap().state == IndexState::Paused
    });
    std::thread::sleep(Duration::from_millis(250));
    let t1 = src.counts().0;
    std::thread::sleep(Duration::from_millis(300));
    assert_eq!(src.counts().0, t1, "står stille");
    let s = f.indexing_status().unwrap();
    assert!(s.total > 0 && s.done <= s.total);
    assert_eq!(s.eta_secs, None, "ingen tidsanslag mens den står på pause");
    f.indexing_resume().unwrap();
    let r = f.wait_scan().unwrap().unwrap();
    assert!(!r.cancelled && r.analyzed == 80, "{r:?}");
    // Analysatorsteget (uten modell) prøver ett bilde før det svarer `Unavailable`; derfor 80 eller 81.
    assert!(
        (80..=81).contains(&src.counts().0),
        "hvert bilde analysert én gang: {}",
        src.counts().0
    );
    assert_eq!(f.indexing_status().unwrap().state, IndexState::Idle);
}

#[test]
fn cancel_through_the_api_keeps_what_was_found_and_a_new_run_finishes() {
    let (f, src) = library(80);
    f.indexing_settings_set(IndexingSettings {
        threads: 2,
        ..IndexingSettings::default()
    })
    .unwrap();
    start(&f);
    wait_until("noe er analysert", || src.counts().0 >= 5);
    f.indexing_pause().unwrap();
    f.indexing_cancel().unwrap();
    let r = f.wait_scan().unwrap().unwrap();
    assert!(r.cancelled);
    let first = src.counts().0;
    assert!(first < 80);
    start(&f);
    let r2 = f.wait_scan().unwrap().unwrap();
    assert!(!r2.cancelled);
    assert!(
        (80..=81).contains(&src.counts().0),
        "ingen dobbeltarbeid på tvers av avbrudd ({first} + resten): {}",
        src.counts().0
    );
}

#[test]
fn battery_pauses_when_the_shell_injects_power_and_the_setting_is_on() {
    let (f, src) = library(40);
    let probe = Arc::new(Probe(AtomicBool::new(true)));
    f.set_power_probe(probe.clone());
    start(&f);
    wait_until("venter på strøm", || {
        f.indexing_status().unwrap().state == IndexState::PausedOnBattery
    });
    std::thread::sleep(Duration::from_millis(250));
    assert_eq!(src.counts().0, 0);
    // Innstillingen slått av: går videre uten å vente på strøm.
    f.indexing_settings_set(IndexingSettings {
        pause_on_battery: false,
        ..IndexingSettings::default()
    })
    .unwrap();
    let r = f.wait_scan().unwrap().unwrap();
    assert_eq!(r.analyzed, 40);
}

#[test]
fn resume_on_battery_overrides_the_pause_for_this_run() {
    let (f, _) = library(30);
    f.set_power_probe(Arc::new(Probe(AtomicBool::new(true))));
    start(&f);
    wait_until("venter på strøm", || {
        f.indexing_status().unwrap().state == IndexState::PausedOnBattery
    });
    f.indexing_resume().unwrap();
    let r = f.wait_scan().unwrap().unwrap();
    assert_eq!(r.analyzed, 30);
}

#[test]
fn worker_hook_runs_for_low_priority_analysis() {
    let (f, _) = library(20);
    let n = Arc::new(AtomicUsize::new(0));
    let k = n.clone();
    f.set_worker_hook(Arc::new(move || {
        k.fetch_add(1, SeqCst);
    }));
    start(&f);
    f.wait_scan().unwrap().unwrap();
    assert!(n.load(SeqCst) >= 1, "lav prioritet er standard");
}

fn p95(mut v: Vec<Duration>) -> Duration {
    v.sort();
    v[(v.len() * 95 / 100).min(v.len() - 1)]
}

/// Ingen UI-handling (status, pause, fortsett, innstillinger, kortliste, Hjem) over 100 ms (p95) mens et stort,
/// syntetisk bibliotek indekseres. Samme budsjett som `docs/mvp/YTELSE.md`; her i debug-bygg (strengere enn release).
#[test]
fn ui_actions_stay_under_100ms_p95_while_indexing_a_large_library() {
    let n = std::env::var("FJELL_INDEXING_PERF_N")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(1_500);
    let (f, _src) = library(n);
    start(&f);
    let mut lat: Vec<(&str, Duration)> = Vec::new();
    let mut i = 0usize;
    let began = Instant::now();
    while f.sh.control.is_active() {
        let t = Instant::now();
        let _ = f.indexing_status().unwrap();
        lat.push(("status", t.elapsed()));
        let t = Instant::now();
        let _ = f.home_summary();
        lat.push(("home", t.elapsed()));
        let t = Instant::now();
        let _ = f.cards(SuggestionKind::ExactDup, 20, None);
        lat.push(("cards", t.elapsed()));
        if i % 25 == 10 {
            let t = Instant::now();
            let _ = f.indexing_pause();
            lat.push(("pause", t.elapsed()));
            std::thread::sleep(Duration::from_millis(30));
            let t = Instant::now();
            let _ = f.indexing_resume();
            lat.push(("resume", t.elapsed()));
        }
        if i % 40 == 5 {
            let t = Instant::now();
            f.indexing_settings_set(IndexingSettings::default())
                .unwrap();
            lat.push(("settings", t.elapsed()));
        }
        i += 1;
        std::thread::sleep(Duration::from_millis(4));
        assert!(began.elapsed() < Duration::from_secs(300));
    }
    let r = f.wait_scan().unwrap().unwrap();
    assert_eq!(r.analyzed as usize, n);
    assert!(lat.len() >= 30, "for få målinger: {}", lat.len());
    for kind in ["status", "home", "cards", "pause", "resume", "settings"] {
        let v: Vec<Duration> = lat
            .iter()
            .filter(|(k, _)| *k == kind)
            .map(|(_, d)| *d)
            .collect();
        if v.is_empty() {
            continue;
        }
        let p = p95(v.clone());
        eprintln!(
            "{kind}: n={} p95={p:?} maks={:?}",
            v.len(),
            v.iter().max().unwrap()
        );
        assert!(p < Duration::from_millis(100), "{kind} p95 {p:?}");
    }
}
