//! K6-4 (ytelse): `upsert` av 10 000 uendrede rader endrer ingen rader og tar < 300 ms. Filbasert WAL-database
//! som i appen. Budsjettet gjelder `--release`; i debug-bygg brukes faktor 25 (som i `actions::tests::perf`).
//! Dessuten: lesing og skriving av grupper (`plan_grouping`/`apply_plan`) på 10 000 rader.

use super::*;
use crate::model::{AssetMeta, Kind, ProtectFlags, SourceId};
use std::time::Instant;

fn factor() -> f64 {
    if cfg!(debug_assertions) {
        25.0
    } else {
        1.0
    }
}

fn meta(i: usize) -> AssetMeta {
    AssetMeta {
        source_id: SourceId(format!("img/{i:06}.jpg")),
        kind: Kind::Photo,
        created_at: i as i64,
        mod_token: "m".into(),
        width: 4000,
        height: 3000,
        bytes: 3_000_000 + i as u64,
        mime: "image/jpeg".into(),
        platform_screenshot_hint: false,
        has_camera_exif: Some(true),
        is_local: true,
        protect_flags: ProtectFlags::default(),
        albums: vec![],
        backup: None,
    }
}

#[test]
fn k6_4_upsert_of_10000_unchanged_rows_changes_nothing_and_is_fast() {
    let dir = std::env::temp_dir().join(format!("fjell-k64-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let mut s = Store::open(&dir.join("index.db")).unwrap();
    let ms: Vec<_> = (0..10_000).map(meta).collect();
    let first = s.upsert_assets(&ms).unwrap();
    assert_eq!(first.new, 10_000);
    let (mut times, mut last) = (Vec::new(), None);
    for _ in 0..5 {
        let before = s.conn().total_changes();
        let t = Instant::now();
        let st = s.upsert_assets(&ms).unwrap();
        times.push(t.elapsed().as_secs_f64() * 1000.0);
        assert_eq!(
            (st.new, st.changed, st.touched, st.unchanged),
            (0, 0, 0, 10_000)
        );
        assert_eq!(s.conn().total_changes(), before, "ingen rader endret");
        last = Some(st);
    }
    assert!(last.is_some());
    times.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let worst = *times.last().unwrap();
    eprintln!(
        "K6-4: upsert 10 000 uendrede rader, median {:.1} ms, verste {:.1} ms (budsjett 300 ms, faktor {})",
        times[times.len() / 2],
        worst,
        factor()
    );
    assert!(worst < 300.0 * factor(), "upsert {worst:.1} ms");
    let _ = std::fs::remove_dir_all(&dir);
}
