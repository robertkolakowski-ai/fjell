//! Tester for lagringsoversikten: kjent fasit, like mappenavn, tom indeks, store tall, determinisme,
//! konsistens med kortene, filbasert lesing og ytelse (100 000 rader).

use super::*;
use crate::model::{AssetMeta, Kind, ProtectFlags, SourceId};
use std::time::Instant;

fn meta(path: &str, kind: Kind, bytes: u64) -> AssetMeta {
    AssetMeta {
        source_id: SourceId(path.into()),
        kind,
        created_at: 1,
        mod_token: "m".into(),
        width: 10,
        height: 10,
        bytes,
        mime: "image/jpeg".into(),
        platform_screenshot_hint: false,
        has_camera_exif: None,
        is_local: true,
        protect_flags: ProtectFlags::default(),
        albums: vec![],
        backup: None,
    }
}

fn store_with(items: &[(&str, Kind, u64)]) -> Store {
    let mut s = Store::open_in_memory().unwrap();
    let ms: Vec<_> = items.iter().map(|(p, k, b)| meta(p, *k, *b)).collect();
    s.upsert_assets(&ms).unwrap();
    s
}

fn sql(s: &Store, q: &str) {
    s.conn().execute_batch(q).unwrap();
}

fn id_of(s: &Store, path: &str) -> i64 {
    s.conn()
        .query_row("SELECT id FROM asset WHERE source_id = ?1", [path], |r| {
            r.get(0)
        })
        .unwrap()
}

fn sum(s: &Store) -> StorageSummary {
    summarize(s, &StorageOptions::default()).unwrap()
}

#[test]
fn empty_index_gives_zero_everywhere() {
    let r = sum(&Store::open_in_memory().unwrap());
    assert_eq!((r.total_count, r.total_bytes, r.trash_bytes), (0, 0, 0));
    assert_eq!(r.per_kind.len(), 3);
    assert!(r.per_kind.iter().all(|k| k.count == 0 && k.bytes == 0));
    assert!(r.per_root.is_empty() && r.per_folder.is_empty() && r.largest_files.is_empty());
    assert_eq!(r.reclaimable_by_category.len(), 4);
    assert_eq!(r.reclaimable_total_bytes, 0);
}

#[test]
fn known_answer_totals_kinds_folders_files() {
    let s = store_with(&[
        ("a.jpg", Kind::Photo, 100),
        ("x/b.jpg", Kind::Photo, 200),
        ("x/y/c.mp4", Kind::Video, 5_000),
        ("x/y/z/d.jpg", Kind::Photo, 40),
        ("x/y/z/w/e.png", Kind::ScreenshotHint, 7),
    ]);
    let r = sum(&s);
    assert_eq!((r.total_count, r.total_bytes), (5, 5_347));
    let k = |i: usize| (r.per_kind[i].kind, r.per_kind[i].count, r.per_kind[i].bytes);
    assert_eq!(k(0), (StorageKind::Photo, 3, 340));
    assert_eq!(k(1), (StorageKind::Video, 1, 5_000));
    assert_eq!(k(2), (StorageKind::Other, 1, 7));
    assert_eq!(
        r.per_root,
        vec![RootUsage {
            root: 0,
            count: 5,
            bytes: 5_347
        }]
    );
    let f: Vec<_> = r
        .per_folder
        .iter()
        .map(|f| (f.path.as_str(), f.depth, f.count, f.bytes))
        .collect();
    // Dybde 3 er grensen: «x/y/z/w» (nivå 4) finnes ikke; filer rett i roten gir ingen mappe.
    assert_eq!(
        f,
        vec![
            ("x", 1, 4, 5_247),
            ("x/y", 2, 3, 5_047),
            ("x/y/z", 3, 2, 47)
        ]
    );
    let top: Vec<_> = r
        .largest_files
        .iter()
        .map(|f| (f.path.as_str(), f.bytes, f.kind))
        .collect();
    assert_eq!(top[0], ("x/y/c.mp4", 5_000, StorageKind::Video));
    assert_eq!(top.len(), 5);
    assert_eq!(top[4], ("x/y/z/w/e.png", 7, StorageKind::Other));
}

#[test]
fn folder_limit_and_depth_are_respected() {
    let s = store_with(&[
        ("a/b/c/d.jpg", Kind::Photo, 10),
        ("e/f.jpg", Kind::Photo, 5),
    ]);
    let o = StorageOptions {
        folder_limit: 1,
        folder_depth: 1,
        file_limit: 1,
    };
    let r = summarize(&s, &o).unwrap();
    assert_eq!(r.per_folder.len(), 1);
    assert_eq!(
        (r.per_folder[0].path.as_str(), r.per_folder[0].bytes),
        ("a", 10)
    );
    assert_eq!(r.largest_files.len(), 1);
    let none = StorageOptions {
        folder_limit: 0,
        folder_depth: 3,
        file_limit: 0,
    };
    let r = summarize(&s, &none).unwrap();
    assert!(r.per_folder.is_empty() && r.largest_files.is_empty());
    assert_eq!(r.total_bytes, 15);
}

#[test]
fn same_folder_names_in_different_parents_stay_apart() {
    let s = store_with(&[
        ("2023/Camera/a.jpg", Kind::Photo, 10),
        ("2024/Camera/b.jpg", Kind::Photo, 20),
        ("2024/Camera/c.jpg", Kind::Photo, 1),
        ("Camera/d.jpg", Kind::Photo, 4),
    ]);
    let r = sum(&s);
    let get = |p: &str| {
        r.per_folder
            .iter()
            .find(|f| f.path == p)
            .map(|f| (f.count, f.bytes))
    };
    assert_eq!(get("2023/Camera"), Some((1, 10)));
    assert_eq!(get("2024/Camera"), Some((2, 21)));
    assert_eq!(get("Camera"), Some((1, 4)));
    assert_eq!(get("2024"), Some((2, 21)));
}

#[test]
fn trashed_and_purged_are_not_active_but_trash_counts() {
    let s = store_with(&[
        ("a.jpg", Kind::Photo, 100),
        ("b.jpg", Kind::Photo, 30),
        ("c.jpg", Kind::Video, 900),
    ]);
    sql(&s, "UPDATE asset SET state = 1, trashed_at = 1, expires_at = 99 WHERE source_id IN ('b.jpg','c.jpg')");
    sql(&s, "UPDATE asset SET state = 2 WHERE source_id = 'c.jpg'");
    let r = sum(&s);
    assert_eq!((r.total_count, r.total_bytes), (1, 100));
    assert_eq!((r.trash_count, r.trash_bytes), (1, 30));
    assert_eq!(r.largest_files.len(), 1);
}

#[test]
fn video_and_unavailable_images_count_by_size_without_fingerprints() {
    let mut m = meta("cloud/p.heic", Kind::Photo, 8_000);
    m.is_local = false;
    let mut s = Store::open_in_memory().unwrap();
    s.upsert_assets(&[m, meta("v.mov", Kind::Video, 20_000)])
        .unwrap();
    assert_eq!(
        s.conn()
            .query_row("SELECT COUNT(*) FROM fingerprint", [], |r| r
                .get::<_, i64>(0))
            .unwrap(),
        0
    );
    let r = sum(&s);
    assert_eq!(r.total_bytes, 28_000);
    assert_eq!(r.per_kind[0].bytes, 8_000);
    assert_eq!(r.per_kind[1].bytes, 20_000);
}

#[test]
fn huge_sizes_do_not_overflow_u64() {
    let h = i64::MAX as u64 / 2; // 3 * h > i64::MAX, men < u64::MAX
    let s = store_with(&[
        ("d/a.mp4", Kind::Video, h),
        ("d/b.mp4", Kind::Video, h),
        ("d/c.mp4", Kind::Video, h),
    ]);
    sql(&s, "UPDATE asset SET state = 1 WHERE source_id = 'd/c.mp4'");
    let r = sum(&s);
    assert_eq!(r.total_bytes, 2 * h);
    let t = store_with(&[
        ("a", Kind::Video, h),
        ("b", Kind::Video, h),
        ("c", Kind::Video, h),
    ]);
    let r = sum(&t);
    assert_eq!(r.total_bytes, 3 * h);
    assert_eq!(r.per_folder.len(), 0);
    assert_eq!(r.per_root[0].bytes, 3 * h);
    // Metning i stedet for omløp ved absurde summer.
    let mut a = scan::Acc::default();
    a.add(u64::MAX);
    a.add(5);
    assert_eq!(a.bytes, u64::MAX);
}

#[test]
fn ordering_is_deterministic_on_equal_sizes() {
    let items = [
        ("m/b.jpg", Kind::Photo, 50),
        ("m/a.jpg", Kind::Photo, 50),
        ("k/z.jpg", Kind::Photo, 50),
        ("n/a.jpg", Kind::Photo, 50),
        ("big.jpg", Kind::Photo, 99),
    ];
    let mut rev = items;
    rev.reverse();
    let (a, b) = (sum(&store_with(&items)), sum(&store_with(&rev)));
    let paths = |r: &StorageSummary| {
        r.largest_files
            .iter()
            .map(|f| f.path.clone())
            .collect::<Vec<_>>()
    };
    assert_eq!(
        paths(&a),
        ["big.jpg", "k/z.jpg", "m/a.jpg", "m/b.jpg", "n/a.jpg"]
    );
    assert_eq!(paths(&a), paths(&b));
    let folders = |r: &StorageSummary| {
        r.per_folder
            .iter()
            .map(|f| f.path.clone())
            .collect::<Vec<_>>()
    };
    assert_eq!(folders(&a), ["m", "k", "n"]); // m har 100, så k og n på sti
    assert_eq!(folders(&a), folders(&b));
    // Topp-N med uavgjort ved grensen velger lavest sti, uavhengig av innsettingsrekkefølge.
    let two = |it: &[(&str, Kind, u64)]| {
        let s = store_with(it);
        largest_files_on(s.read_conn(), 2)
            .unwrap()
            .into_iter()
            .map(|f| f.path)
            .collect::<Vec<_>>()
    };
    assert_eq!(two(&items), ["big.jpg", "k/z.jpg"]);
    assert_eq!(two(&rev), ["big.jpg", "k/z.jpg"]);
}

fn add_suggestion(s: &Store, path: &str, kind: &str, status: &str) {
    let id = id_of(s, path);
    sql(
        s,
        &format!("INSERT INTO suggestion(asset_id, kind, reason_code, confidence, status) VALUES ({id}, '{kind}', 'r', 0.9, '{status}')"),
    );
}

#[test]
fn reclaimable_matches_cards_rule_and_kind_totals() {
    let s = store_with(&[
        ("a.jpg", Kind::Photo, 10),
        ("b.jpg", Kind::Photo, 20),
        ("c.jpg", Kind::Photo, 30),
        ("d.jpg", Kind::Photo, 40),
        ("e.jpg", Kind::Photo, 50),
        ("f.jpg", Kind::Photo, 60),
    ]);
    add_suggestion(&s, "a.jpg", "exact_dup", "pending");
    add_suggestion(&s, "a.jpg", "blurry", "pending"); // skjult av høyere prioritet
    add_suggestion(&s, "b.jpg", "visual_dup", "pending");
    add_suggestion(&s, "c.jpg", "blurry", "pending");
    add_suggestion(&s, "d.jpg", "screenshot", "pending");
    add_suggestion(&s, "e.jpg", "exact_dup", "rejected"); // ikke ventende
    add_suggestion(&s, "f.jpg", "blurry", "pending");
    sql(&s, "UPDATE asset SET state = 1 WHERE source_id = 'f.jpg'"); // i papirkurv
    let r = sum(&s);
    let got: Vec<_> = r
        .reclaimable_by_category
        .iter()
        .map(|c| (c.kind, c.count, c.bytes))
        .collect();
    assert_eq!(
        got,
        vec![
            (SuggestionKind::ExactDup, 1, 10),
            (SuggestionKind::VisualDup, 1, 20),
            (SuggestionKind::Blurry, 1, 30),
            (SuggestionKind::Screenshot, 1, 40),
        ]
    );
    assert_eq!(r.reclaimable_total_bytes, 100);
    let mut kt = s.kind_totals().unwrap();
    kt.sort_by_key(|x| x.0.as_str());
    let mut mine: Vec<_> = got.clone();
    mine.sort_by_key(|x| x.0.as_str());
    assert_eq!(kt, mine);
}

#[test]
fn file_backed_index_is_read_through_separate_connection() {
    let dir = std::env::temp_dir().join(format!("fjell-lagring-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let mut s = Store::open(&dir.join("index.db")).unwrap();
    s.upsert_assets(&[
        meta("a/x.jpg", Kind::Photo, 11),
        meta("b/y.mp4", Kind::Video, 22),
    ])
    .unwrap();
    let rd = s.reader().unwrap().unwrap();
    let via_reader = summarize_on(&rd, &StorageOptions::default()).unwrap();
    assert_eq!(via_reader, sum(&s));
    assert_eq!(via_reader.total_bytes, 33);
    drop(rd);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn every_kind_maps_to_a_storage_kind() {
    for (k, want) in [
        (Kind::Photo, StorageKind::Photo),
        (Kind::Video, StorageKind::Video),
        (Kind::ScreenshotHint, StorageKind::Other),
    ] {
        assert_eq!(scan::storage_kind(k.as_str()), want);
    }
}

fn factor() -> f64 {
    if cfg!(debug_assertions) {
        25.0
    } else {
        1.0
    }
}

/// Mål: < 150 ms ved 100 000 rader (release; i debug faktor 25 som i kjernens øvrige perf-tester).
#[test]
fn perf_100k_rows_under_budget() {
    let dir = std::env::temp_dir().join(format!("fjell-lagring-perf-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let s = Store::open(&dir.join("index.db")).unwrap();
    {
        let tx = s.conn().unchecked_transaction().unwrap();
        {
            let mut st = tx
                .prepare(
                    "INSERT INTO asset(source_id, kind, created_at, mod_token, width, height, bytes, mime, state) \
                     VALUES (?1, ?2, 1, 'm', 1, 1, ?3, 'image/jpeg', ?4)",
                )
                .unwrap();
            for i in 0..100_000u64 {
                let path = format!("{}/{}/{}/img{i:06}.jpg", 2000 + i % 25, i % 12, i % 7);
                let kind = ["photo", "photo", "video", "screenshot_hint"][(i % 4) as usize];
                let state = i64::from(i % 20 == 0);
                st.execute(rusqlite::params![
                    path,
                    kind,
                    1_000_000 + (i * 7919 % 5_000_000) as i64,
                    state
                ])
                .unwrap();
            }
        }
        tx.execute_batch(
            "INSERT INTO suggestion(asset_id, kind, reason_code, confidence, status) \
             SELECT id, CASE id % 4 WHEN 0 THEN 'exact_dup' WHEN 1 THEN 'visual_dup' WHEN 2 THEN 'blurry' ELSE 'screenshot' END, \
             'r', 0.5, 'pending' FROM asset WHERE id % 3 = 0;
             INSERT INTO suggestion(asset_id, kind, reason_code, confidence, status) \
             SELECT id, 'blurry', 'r', 0.4, 'pending' FROM asset WHERE id % 6 = 0 AND id % 4 != 2;",
        )
        .unwrap();
        tx.commit().unwrap();
    }
    let rd = s.reader().unwrap().unwrap();
    let mut times = Vec::new();
    let mut last = None;
    for _ in 0..5 {
        let t = Instant::now();
        last = Some(summarize_on(&rd, &StorageOptions::default()).unwrap());
        times.push(t.elapsed().as_secs_f64() * 1000.0);
    }
    times.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let r = last.unwrap();
    assert_eq!(r.total_count + r.trash_count, 100_000);
    assert_eq!(r.largest_files.len(), 20);
    assert_eq!(r.per_folder.len(), 20);
    eprintln!(
        "LAGRING: summarize 100 000 rader, median {:.1} ms, verste {:.1} ms (budsjett 150 ms, faktor {})",
        times[2], times[4], factor()
    );
    assert!(times[4] < 150.0 * factor(), "summarize {:.1} ms", times[4]);
    drop(rd);
    let _ = std::fs::remove_dir_all(&dir);
}
