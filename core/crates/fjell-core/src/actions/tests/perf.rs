//! Ytelse (M3-porten): `trash`/`undo` p95 < 10 ms kjernekall på database med 50 000 bilder
//! (PLAN-TRYGGHET del 7, ARKITEKTUR del 4). Filbasert WAL-database som i appen.
//!
//! Budsjettene gjelder `--release` (slik CI/nightly måler). I debug-bygg, som er mange ganger
//! tregere, brukes en romslig faktor slik at testen fortsatt fanger størrelsesorden-regresjoner.

use super::*;
use std::time::Instant;

fn factor() -> f64 {
    if cfg!(debug_assertions) {
        25.0
    } else {
        1.0
    }
}

fn p95(mut v: Vec<f64>) -> f64 {
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    v[(v.len() as f64 * 0.95) as usize - 1]
}

fn big_db(tag: &str, n: i64) -> Fx {
    let dir = std::env::temp_dir().join(format!("fjell-perf-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let fx = Fx::with_store(Store::open(&dir.join("index.db")).unwrap());
    let c = fx.a.store().conn();
    c.execute_batch("BEGIN").unwrap();
    for i in 1..=n {
        c.execute(
            "INSERT INTO asset(id, source_id, kind, created_at, mod_token, width, height, bytes, mime) \
             VALUES (?1, ?2, 'photo', ?1, 'm', 4000, 3000, 3000000, 'image/heic')",
            rusqlite::params![i, format!("s{i}")],
        )
        .unwrap();
        if i % 7 == 0 {
            c.execute("INSERT INTO suggestion(asset_id, kind, reason_code, confidence) VALUES (?1, 'blurry', 'blurry', 0.5)", [i]).unwrap();
        }
    }
    c.execute_batch("COMMIT").unwrap();
    fx
}

fn ms(t: Instant) -> f64 {
    t.elapsed().as_secs_f64() * 1000.0
}

/// M3-porten: 1 000 `trash`- og 1 000 `undo`-kall av 1-200 bilder mot 50 000 bilder.
#[test]
fn trash_and_undo_p95_under_10ms_on_50k_assets() {
    let mut fx = big_db("p95", 50_000);
    let sizes = [1usize, 1, 1, 5, 20, 200];
    let (mut trash_ms, mut undo_ms) = (vec![], vec![]);
    for k in 0..1000usize {
        let n = sizes[k % sizes.len()];
        let start = 1 + (k as u64 * 211) % 49_000;
        let ids: Vec<_> = (start..start + n as u64).map(AssetId).collect();
        let t = Instant::now();
        let plan = fx.a.plan(&PlanRequest::manual(&ids)).unwrap();
        let b = fx.a.commit(&plan, &acks_covering(&plan)).unwrap();
        trash_ms.push(ms(t));
        assert!(b.is_some());
        let t = Instant::now();
        let u = fx.a.undo().unwrap();
        undo_ms.push(ms(t));
        assert_eq!(u.restored, n as u64);
    }
    let (pt, pu) = (p95(trash_ms), p95(undo_ms));
    eprintln!(
        "p95 trash (plan+commit) = {pt:.2} ms, undo = {pu:.2} ms (50 000 bilder, faktor {})",
        factor()
    );
    assert!(pt < 10.0 * factor(), "trash p95 {pt:.2} ms");
    assert!(pu < 10.0 * factor(), "undo p95 {pu:.2} ms");
}

/// Resten av del 7: `plan` for 1 000 id-er, `trash_summary` og store batcher (> 200: hard grense
/// 100 ms ende til ende; målet 50 ms for 1 000 id-er rapporteres, men er ikke port).
#[test]
fn plan_summary_and_large_batches_within_budget() {
    let mut fx = big_db("large", 50_000);
    let ids: Vec<_> = (1..=1000).map(AssetId).collect();
    let t = Instant::now();
    fx.a.plan(&PlanRequest::manual(&ids)).unwrap();
    let plan_ms = ms(t);
    assert!(plan_ms < 10.0 * factor(), "plan(1000) {plan_ms:.2} ms");

    let plan = fx.a.plan(&PlanRequest::manual(&ids)).unwrap();
    let t = Instant::now();
    fx.a.commit(&plan, &acks_covering(&plan)).unwrap();
    let trash_ms = ms(t);
    let t = Instant::now();
    let s = fx.a.trash_summary().unwrap();
    let sum_ms = ms(t);
    assert_eq!(s.trashed_count, 1000);
    let t = Instant::now();
    fx.a.undo().unwrap();
    let undo_ms = ms(t);
    eprintln!("plan(1000) {plan_ms:.2} ms, commit(1000) {trash_ms:.2} ms, undo(1000) {undo_ms:.2} ms, summary {sum_ms:.2} ms");
    assert!(
        trash_ms < 100.0 * factor() && undo_ms < 100.0 * factor(),
        "1000 id-er: {trash_ms:.1}/{undo_ms:.1} ms"
    );
    assert!(sum_ms < 5.0 * factor(), "trash_summary {sum_ms:.2} ms");
}
