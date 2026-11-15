//! Indeksering (status, pause, fortsett, avbryt, innstillinger) gjennom skallets kommandolag (docs/v1/INDEKSERING.md).
//! Første del: kontrakten mot mocken (`MemApi`). Andre del: ekte kjerne (`Fjell` med syntetisk `MemSource`), der
//! pause, fortsett, avbryt og batteripause kjøres mot en faktisk skanning.
//! Kjørt på Linux; strømstatus og trådprioritet på Windows er ikke verifisert.

use fjell_core::api::{Fjell, FjellApi, ScanConfig, ScanMode};
use fjell_core::model::{
    AssetMeta, FakeClock, Kind, MemSource, ProtectFlags, SourceId, Thumb, ThumbFmt,
};
use fjell_core::pipeline::PowerProbe;
use fjell_core::store::Store;
use fjell_shell::dto_robust::IndexingSettingsDto;
use fjell_shell::Shell;
use serde_json::json;
use std::sync::atomic::{AtomicBool, Ordering::SeqCst};
use std::sync::Arc;
use std::time::{Duration, Instant};

#[test]
fn mock_status_is_idle_and_settings_are_the_defaults() {
    let shell = Shell::mock();
    let s = shell.indexing_status().unwrap();
    assert_eq!(
        serde_json::to_value(&s).unwrap(),
        json!({ "state": "idle", "phase": "idle", "done": 0, "total": 0, "ratePerSec": 0, "etaSecs": null })
    );
    assert_eq!(
        serde_json::to_value(shell.indexing_settings().unwrap()).unwrap(),
        json!({ "threads": 0, "priority": "low", "pauseOnBattery": true })
    );
}

#[test]
fn unknown_priority_is_refused_with_a_code() {
    let shell = Shell::mock();
    let bad = IndexingSettingsDto {
        threads: 2,
        priority: "turbo".into(),
        pause_on_battery: true,
    };
    let e = shell.set_indexing_settings(&bad).unwrap_err();
    assert_eq!(
        (e.code, e.detail.as_str()),
        ("core_error", "indexing_invalid_settings")
    );
}

#[test]
fn mock_control_calls_keep_the_contract_default() {
    let shell = Shell::mock();
    for e in [
        shell.indexing_pause().unwrap_err(),
        shell.indexing_resume().unwrap_err(),
        shell.indexing_cancel().unwrap_err(),
    ] {
        assert_eq!(
            (e.code, e.detail.as_str()),
            ("core_error", "not_implemented")
        );
    }
}

// --- Ekte kjerne ---

struct Battery(AtomicBool);
impl PowerProbe for Battery {
    fn on_battery(&self) -> bool {
        self.0.load(SeqCst)
    }
}

fn meta(name: &str, bytes: u64) -> AssetMeta {
    AssetMeta {
        source_id: SourceId(name.into()),
        kind: Kind::Photo,
        created_at: 1_000,
        mod_token: "t1".into(),
        width: 64,
        height: 64,
        bytes,
        mime: "image/jpeg".into(),
        platform_screenshot_hint: false,
        has_camera_exif: Some(true),
        is_local: true,
        protect_flags: ProtectFlags::default(),
        albums: vec![],
        backup: None,
    }
}

/// `n` bilder med egne miniatyrer og unike størrelser (ingen rask fase), så den fulle fasen har arbeid.
fn core_shell(n: usize) -> (Shell, Arc<Fjell>) {
    let src = MemSource::new();
    for i in 0..n {
        let px: Vec<u8> = (0..32 * 32)
            .map(|p| ((p * (i + 3) + i * 17) % 251) as u8)
            .collect();
        src.put(
            meta(&format!("i{i:03}.jpg"), 1_000 + i as u64 * 3),
            Some(Thumb::new(32, 32, ThumbFmt::Gray8, px).unwrap()),
            vec![(i % 250) as u8; 1_000 + i * 3],
        );
    }
    let f = Arc::new(Fjell::new(
        Store::open_in_memory().unwrap(),
        Arc::new(src),
        Arc::new(FakeClock::new(5_000_000)),
    ));
    f.indexing_control().set_probe_interval(Duration::ZERO);
    (Shell::new(f.clone(), "core"), f)
}

fn start(f: &Fjell) {
    f.start_scan(ScanConfig {
        mode: ScanMode::Full,
        priority: 1,
        deadline: None,
    })
    .unwrap();
}

fn wait_for(what: &str, mut c: impl FnMut() -> bool) {
    let t = Instant::now();
    while !c() {
        assert!(t.elapsed() < Duration::from_secs(30), "tidsavbrudd: {what}");
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn state(shell: &Shell) -> String {
    serde_json::to_value(shell.indexing_status().unwrap()).unwrap()["state"]
        .as_str()
        .unwrap()
        .to_string()
}

#[test]
fn idle_core_gives_idle_status_and_not_running_codes() {
    let (shell, _f) = core_shell(3);
    assert_eq!(state(&shell), "idle");
    for e in [
        shell.indexing_pause().unwrap_err(),
        shell.indexing_resume().unwrap_err(),
        shell.indexing_cancel().unwrap_err(),
    ] {
        assert_eq!(
            (e.code, e.detail.as_str()),
            ("core_error", "indexing_not_running")
        );
    }
}

#[test]
fn pause_resume_and_finish_through_the_shell() {
    let (shell, f) = core_shell(120);
    shell
        .set_indexing_settings(&IndexingSettingsDto {
            threads: 2,
            priority: "low".into(),
            pause_on_battery: true,
        })
        .unwrap();
    start(&f);
    shell.indexing_pause().unwrap();
    wait_for("pauset", || state(&shell) == "paused");
    let a = serde_json::to_value(shell.indexing_status().unwrap()).unwrap();
    std::thread::sleep(Duration::from_millis(300));
    let b = serde_json::to_value(shell.indexing_status().unwrap()).unwrap();
    assert_eq!(a["done"], b["done"], "står stille");
    assert_eq!(b["etaSecs"], json!(null));
    shell.indexing_resume().unwrap();
    let r = f.wait_scan().unwrap().unwrap();
    assert!(!r.cancelled && r.analyzed == 120);
    assert_eq!(state(&shell), "idle");
}

#[test]
fn cancel_keeps_the_work_and_a_new_scan_finishes() {
    let (shell, f) = core_shell(120);
    start(&f);
    shell.indexing_pause().unwrap();
    shell.indexing_cancel().unwrap();
    let r = f.wait_scan().unwrap().unwrap();
    assert!(r.cancelled);
    start(&f);
    let r2 = f.wait_scan().unwrap().unwrap();
    assert!(!r2.cancelled);
    assert_eq!(r.analyzed + r2.analyzed, 120, "ingen dobbeltarbeid");
}

#[test]
fn battery_shows_as_paused_on_battery_and_resume_overrides() {
    let (shell, f) = core_shell(60);
    f.set_power_probe(Arc::new(Battery(AtomicBool::new(true))));
    start(&f);
    wait_for("venter på strøm", || state(&shell) == "paused_on_battery");
    shell.indexing_resume().unwrap();
    wait_for("kjører videre", || state(&shell) != "paused_on_battery");
    let r = f.wait_scan().unwrap().unwrap();
    assert_eq!(r.analyzed, 60);
}

#[test]
fn settings_roundtrip_through_the_core() {
    let (shell, _f) = core_shell(1);
    let s = IndexingSettingsDto {
        threads: 3,
        priority: "normal".into(),
        pause_on_battery: false,
    };
    assert_eq!(shell.set_indexing_settings(&s).unwrap(), s);
    assert_eq!(shell.indexing_settings().unwrap(), s);
    let bad = IndexingSettingsDto { threads: 65, ..s };
    let e = shell.set_indexing_settings(&bad).unwrap_err();
    assert_eq!(
        (e.code, e.detail.as_str()),
        ("core_error", "indexing_invalid_settings")
    );
}
