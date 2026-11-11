//! Tester for `index` (K6-3, K6-4 delvis, I13). Ytelse (K6-4) er ikke målt her.

use super::*;
use crate::model::{Album, AlgoVer, AssetId, AssetMeta, Kind, ProtectFlags, SourceId};

fn meta(name: &str, token: &str) -> AssetMeta {
    AssetMeta {
        source_id: SourceId(name.into()),
        kind: Kind::Photo,
        created_at: 1,
        mod_token: token.into(),
        width: 10,
        height: 10,
        bytes: 100,
        mime: "image/jpeg".into(),
        platform_screenshot_hint: false,
        has_camera_exif: None,
        is_local: true,
        protect_flags: ProtectFlags::default(),
        albums: vec![Album {
            id: 7,
            name: "A".into(),
            shared: false,
        }],
        backup: None,
    }
}

fn fp(id: u64, algo: u32) -> FingerprintRow {
    FingerprintRow {
        id: AssetId(id),
        dhash: u64::MAX,
        phash: 1,
        blur: 0.5,
        signals: 0,
        algo: AlgoVer(algo),
    }
}

#[test]
fn upsert_counts_and_unchanged_rows_are_not_written() {
    let mut s = Store::open_in_memory().unwrap();
    let ms: Vec<_> = (0..1200).map(|i| meta(&format!("f{i}"), "a")).collect();
    let st = s.upsert_assets(&ms).unwrap();
    assert_eq!((st.new, st.unchanged), (1200, 0));
    let before = s.conn().total_changes();
    let st = s.upsert_assets(&ms).unwrap();
    assert_eq!(
        (st.new, st.changed, st.touched, st.unchanged),
        (0, 0, 0, 1200)
    );
    assert_eq!(s.conn().total_changes(), before, "ingen rad endret");
}

#[test]
fn k6_3_work_set_empty_after_scan_and_full_after_version_bump() {
    let mut s = Store::open_in_memory().unwrap();
    s.upsert_assets(&[meta("a", "1"), meta("b", "1")]).unwrap();
    assert_eq!(s.work_set(AlgoVer(1)).unwrap().len(), 2);
    s.put_fingerprints(&[fp(1, 1), fp(2, 1)]).unwrap();
    assert!(s.work_set(AlgoVer(1)).unwrap().is_empty());
    assert_eq!(s.work_set(AlgoVer(2)).unwrap().len(), 2);
    // dhash med toppbit satt overlever rundturen (u64 <-> i64).
    assert_eq!(s.index_rows().unwrap()[0].dhash, Some(u64::MAX));
}

#[test]
fn changed_token_resets_fingerprint_but_keeps_state_and_trash_fields() {
    let mut s = Store::open_in_memory().unwrap();
    s.upsert_assets(&[meta("a", "1")]).unwrap();
    s.put_fingerprints(&[fp(1, 1)]).unwrap();
    s.put_content_hashes(&[(AssetId(1), [3; 32])]).unwrap();
    s.conn()
        .execute(
            "UPDATE asset SET state = 1, trashed_at = 5, expires_at = 99",
            [],
        )
        .unwrap();
    let st = s.upsert_assets(&[meta("a", "2")]).unwrap();
    assert_eq!(st.changed, 1);
    let (state, t, e): (i64, i64, i64) = s
        .conn()
        .query_row("SELECT state, trashed_at, expires_at FROM asset", [], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?))
        })
        .unwrap();
    assert_eq!((state, t, e), (1, 5, 99));
    let (algo, hash): (i64, Option<Vec<u8>>) = s
        .conn()
        .query_row("SELECT algo_ver, content_hash FROM fingerprint", [], |r| {
            Ok((r.get(0)?, r.get(1)?))
        })
        .unwrap();
    assert_eq!((algo, hash), (0, None));
    // Flagg endret uten ny token: fingeravtrykket beholdes.
    s.put_fingerprints(&[fp(1, 1)]).unwrap();
    let mut m = meta("a", "2");
    m.protect_flags = ProtectFlags(ProtectFlags::FAVORITE);
    assert_eq!(s.upsert_assets(&[m]).unwrap().touched, 1);
    assert_eq!(s.index_rows().unwrap().len(), 0, "trashed er ikke aktiv");
    let algo: i64 = s
        .conn()
        .query_row("SELECT algo_ver FROM fingerprint", [], |r| r.get(0))
        .unwrap();
    assert_eq!(algo, 1);
}

#[test]
fn failures_skip_after_three_until_algo_version_changes() {
    let mut s = Store::open_in_memory().unwrap();
    s.upsert_assets(&[meta("a", "1")]).unwrap();
    s.begin_scan(AlgoVer(1)).unwrap();
    for _ in 0..3 {
        assert_eq!(s.work_set(AlgoVer(1)).unwrap().len(), 1);
        s.record_failures(&[AssetId(1)]).unwrap();
    }
    assert!(s.work_set(AlgoVer(1)).unwrap().is_empty());
    s.begin_scan(AlgoVer(1)).unwrap();
    assert!(s.work_set(AlgoVer(1)).unwrap().is_empty());
    s.begin_scan(AlgoVer(2)).unwrap();
    assert_eq!(s.work_set(AlgoVer(2)).unwrap().len(), 1);
}

#[test]
fn purged_rows_are_not_touched_by_upsert() {
    let mut s = Store::open_in_memory().unwrap();
    s.upsert_assets(&[meta("a", "1")]).unwrap();
    s.conn().execute("UPDATE asset SET state = 1", []).unwrap();
    s.conn().execute("UPDATE asset SET state = 2", []).unwrap();
    let st = s.upsert_assets(&[meta("a", "9")]).unwrap();
    assert_eq!(st.unchanged, 1);
    let tok: String = s
        .conn()
        .query_row("SELECT mod_token FROM asset", [], |r| r.get(0))
        .unwrap();
    assert_eq!(tok, "1");
}
