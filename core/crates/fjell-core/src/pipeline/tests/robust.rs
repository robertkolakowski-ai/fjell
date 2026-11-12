//! K7-5 (avbrudd), K7-6 (krasj), K7-12 (ingen `tokio`/`unsafe`), frist og avstemming.

use super::*;
use crate::store::Store;
use proptest::prelude::*;
use std::sync::atomic::Ordering::SeqCst;

const CRASH_ENV: &str = "FJELL_K7_CRASH_DB";
const BIG: usize = 300;

fn baseline() -> Dump {
    let mut fx = Fx::new();
    fx.corpus();
    fx.scan(&ScanConfig::full());
    dump(&fx)
}

fn consistent(fx: &Fx) {
    fx.with(|s| {
        let ok: String = s.conn().query_row("PRAGMA integrity_check", [], |r| r.get(0)).unwrap();
        assert_eq!(ok, "ok");
        // Ingen halvferdige rader: analysert = alle felt satt.
        let bad: i64 = s
            .conn()
            .query_row(
                "SELECT COUNT(*) FROM fingerprint WHERE algo_ver > 0 AND \
                 (dhash IS NULL OR phash IS NULL OR blur_score IS NULL OR shot_signals IS NULL)",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(bad, 0);
        let orphans: i64 = s
            .conn()
            .query_row(
                "SELECT COUNT(*) FROM grp_member m LEFT JOIN grp g ON g.id = m.grp_id WHERE g.id IS NULL",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(orphans, 0);
    });
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(24))]
    /// K7-5: avbrudd etter vilkårlig miniatyr: indeksen er konsistent, og neste skanning gir samme
    /// sluttresultat som en uavbrutt.
    #[test]
    fn k7_5_cancel_anywhere_then_resume_gives_same_result(cut in 0usize..28) {
        let mut fx = Fx::new();
        fx.corpus();
        let flag = fx.cancel.clone();
        *fx.src.hook.lock().unwrap() = Some(Arc::new(Box::new(move |n| {
            if n == cut {
                flag.store(true, SeqCst);
            }
        })));
        let r = fx.scan(&ScanConfig::full());
        prop_assert!(r.cancelled);
        prop_assert!(r.analyzed <= 28);
        consistent(&fx);
        *fx.src.hook.lock().unwrap() = None;
        fx.cancel.store(false, SeqCst);
        let r2 = fx.scan(&ScanConfig::full());
        prop_assert!(!r2.cancelled);
        prop_assert_eq!(r.analyzed + r2.analyzed, 28, "hver fil analysert nøyaktig én gang");
        consistent(&fx);
        prop_assert_eq!(dump(&fx), baseline());
    }
}

#[test]
fn cancel_before_quick_phase_stops_without_work() {
    let mut fx = Fx::new();
    fx.corpus();
    fx.cancel.store(true, SeqCst);
    let r = fx.scan(&ScanConfig::full());
    assert!(r.cancelled);
    assert_eq!((r.hashed, r.analyzed, fx.src.counts()), (0, 0, (0, 0)));
    fx.cancel.store(false, SeqCst);
    fx.scan(&ScanConfig::full());
    assert_eq!(dump(&fx), baseline());
}

#[test]
fn deadline_stops_cleanly_and_next_scan_finishes() {
    let mut fx = Fx::new();
    fx.corpus();
    let cfg = ScanConfig {
        deadline: Some(std::time::Duration::ZERO),
        ..ScanConfig::full()
    };
    let r = fx.scan(&cfg);
    assert!(r.deadline_hit && !r.cancelled);
    assert_eq!(r.analyzed, 0);
    consistent(&fx);
    fx.scan(&ScanConfig::full());
    assert_eq!(dump(&fx), baseline());
}

fn big_fx(store: Store) -> Fx {
    let mut fx = Fx::with_store(store);
    for i in 0..BIG as u64 {
        fx.uniq(&format!("p{i:03}.jpg"), scene(5_000 + i));
    }
    fx
}

/// Barneprosessen: skanner en fil-database og dør brått (`process::exit`, ingen destruktorer) midt i
/// full fase. Gjør ingenting uten miljøvariabelen (da er den bare en vanlig, tom test).
#[test]
fn crash_child() {
    let Ok(path) = std::env::var(CRASH_ENV) else {
        return;
    };
    let fx = big_fx(Store::open(std::path::Path::new(&path)).unwrap());
    *fx.src.hook.lock().unwrap() = Some(Arc::new(Box::new(|n| {
        if n == 220 {
            std::process::exit(9);
        }
    })));
    fx.scan(&ScanConfig::full());
    std::process::exit(0); // skulle ikke nås
}

#[test]
fn k7_6_crash_mid_write_leaves_no_half_batch_and_resume_completes() {
    if std::env::var(CRASH_ENV).is_ok() {
        return; // i barneprosessen kjøres bare `crash_child`
    }
    let dir = std::env::temp_dir().join(format!("fjell-k7-crash-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let db = dir.join("index.sqlite");
    let st = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "pipeline::tests::robust::crash_child",
            "--nocapture",
            "--test-threads=1",
        ])
        .env(CRASH_ENV, &db)
        .output()
        .unwrap();
    assert_eq!(
        st.status.code(),
        Some(9),
        "barneprosessen skulle krasje: {st:?}"
    );
    // Gjenåpne: databasen er hel, og bare hele batcher à 100 rader er skrevet.
    let fx = big_fx(Store::open(&db).unwrap());
    consistent(&fx);
    let done: i64 = fx.with(|s| {
        s.conn()
            .query_row(
                "SELECT COUNT(*) FROM fingerprint WHERE algo_ver > 0",
                [],
                |r| r.get(0),
            )
            .unwrap()
    });
    assert!(
        done >= 100 && done < BIG as i64 && done % 100 == 0,
        "{done} analyserte rader"
    );
    // Gjenopptak: bare resten analyseres, og resultatet er som ved uavbrutt skanning.
    let r = fx.scan(&ScanConfig::full());
    assert_eq!(r.analyzed as i64, BIG as i64 - done);
    let clean = big_fx(Store::open_in_memory().unwrap());
    clean.scan(&ScanConfig::full());
    assert_eq!(dump(&fx), dump(&clean));
    let _ = std::fs::remove_dir_all(&dir);
}

/// K7-12: ingen `tokio` i avhengighetene, ingen `unsafe` i pipeline (i tillegg til `forbid(unsafe_code)`).
#[test]
fn k7_12_no_tokio_no_unsafe_in_pipeline() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let lock = std::fs::read_to_string(root.join("../../Cargo.lock")).unwrap();
    assert!(!lock.contains("name = \"tokio\""), "tokio i Cargo.lock");
    let toml = std::fs::read_to_string(root.join("Cargo.toml")).unwrap();
    assert!(!toml.contains("tokio"));
    for f in ["mod", "fast", "full", "groups", "progress"] {
        let src = std::fs::read_to_string(root.join(format!("src/pipeline/{f}.rs"))).unwrap();
        let code: String = src
            .lines()
            .filter(|l| !l.trim_start().starts_with("//"))
            .collect();
        assert!(!code.contains("unsafe"), "unsafe i pipeline/{f}.rs");
        assert!(
            !code.contains("tokio") && !code.contains("async "),
            "async i pipeline/{f}.rs"
        );
    }
}

#[test]
fn removed_sources_are_reconciled_and_leave_groups() {
    let mut fx = Fx::new();
    fx.corpus();
    fx.scan(&ScanConfig::full());
    assert!(fx.src.inner.remove(&SourceId("e0b.jpg".into())));
    let r = fx.scan(&ScanConfig::full());
    assert_eq!((r.removed, r.analyzed, r.new), (1, 0, 0));
    let d = dump(&fx);
    assert!(d.0.iter().all(|g| !g.2.contains(&"e0b.jpg".to_string())));
    let id = fx.id("e0b.jpg").0 as i64;
    let (state, cause): (i64, String) = fx.with(|s| {
        let st = s
            .conn()
            .query_row("SELECT state FROM asset WHERE id=?1", [id], |r| r.get(0))
            .unwrap();
        let c = s
            .conn()
            .query_row(
                "SELECT cause FROM action_log WHERE asset_id=?1 AND op='purge'",
                [id],
                |r| r.get(0),
            )
            .unwrap();
        (st, c)
    });
    assert_eq!((state, cause.as_str()), (2, "external"));
}
