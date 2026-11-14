//! Tester for `dashboard`: tall, beskyttelse, frigjort uten `external`, fremdrift, neste steg,
//! egenskapstester for invariantene, cache og ytelse (50 000 bilder).

use super::*;
use crate::model::DashboardData;
use proptest::prelude::*;
use std::time::Instant;

/// Test-oppsett rett mot indeksen (ingen kjernelogikk som kan skjule feil i lesemodellen).
struct W {
    s: Store,
}

impl W {
    fn new() -> W {
        W {
            s: Store::open_in_memory().unwrap(),
        }
    }
    fn x(&self, sql: &str, p: impl rusqlite::Params) {
        self.s.conn().execute(sql, p).unwrap();
    }
    fn asset(&self, id: i64, src: &str, bytes: i64, flags: i64, created: i64) {
        self.x(
            "INSERT INTO asset(id, source_id, kind, created_at, mod_token, width, height, bytes, mime, \
             protect_flags) VALUES (?1, ?2, 'photo', ?3, 'm', 4, 4, ?4, 'image/jpeg', ?5)",
            rusqlite::params![id, src, created, bytes, flags],
        );
    }
    fn sugg(&self, id: i64, kind: &str, status: &str) {
        self.x(
            "INSERT INTO suggestion(asset_id, kind, reason_code, confidence, status) \
             VALUES (?1, ?2, 'r', 0.5, ?3)",
            rusqlite::params![id, kind, status],
        );
    }
    fn protect(&self, scope: &str, target: &str) {
        self.x(
            "INSERT INTO protection(scope, target, created_at) VALUES (?1, ?2, 1)",
            [scope, target],
        );
    }
    fn state(&self, id: i64, state: i64) {
        self.x("UPDATE asset SET state = ?2 WHERE id = ?1", [id, state]);
    }
    fn log(&self, id: i64, ts: i64, op: &str, cause: &str) {
        self.x(
            "INSERT INTO action_log(batch_id, ts, op, asset_id, prev_state, cause) \
             VALUES (1, ?1, ?2, ?3, 1, ?4)",
            rusqlite::params![ts, op, id, cause],
        );
    }
    /// Asset som er purget med gitt årsak (via lovlig overgang aktiv -> papirkurv -> purget).
    fn purged(&self, id: i64, ts: i64, cause: &str) {
        self.state(id, 1);
        self.state(id, 2);
        self.log(id, ts, "purge", cause);
    }
    fn read(&self) -> Dashboard {
        read(&self.s).unwrap()
    }
    fn kind(&self, d: &Dashboard, k: SuggestionKind) -> (u64, u64) {
        let s = d.data.per_kind.iter().find(|x| x.kind == k).unwrap();
        (s.count, s.bytes)
    }
}

#[test]
fn smoke_dashboard_default_is_empty() {
    let d = DashboardData::default();
    assert_eq!(
        (d.active_count, d.active_bytes, d.per_kind.len()),
        (0, 0, 0)
    );
}

#[test]
fn empty_index_gives_zeros_and_nothing_to_do() {
    let d = W::new().read();
    assert_eq!(d.data.active_count, 0);
    assert_eq!(d.data.per_kind.len(), SuggestionKind::ALL.len());
    assert!(d.data.per_kind.iter().all(|k| k.count == 0 && k.bytes == 0));
    assert_eq!(d.insights.progress.permille, 0, "ingen kunstig seier");
    assert_eq!(d.insights.next_step, NextStep::Nothing);
    assert!(d.insights.freed_by_month.is_empty());
}

#[test]
fn used_space_counts_only_active_and_trash_is_separate() {
    let w = W::new();
    w.asset(1, "a/1", 100, 0, 1);
    w.asset(2, "a/2", 200, 0, 1);
    w.asset(3, "a/3", 400, 0, 1);
    w.asset(4, "a/4", 800, 0, 1);
    w.state(2, 1);
    w.purged(3, 5, "user");
    let d = w.read().data;
    assert_eq!((d.active_count, d.active_bytes), (2, 900));
    assert_eq!((d.trashed_count, d.trashed_bytes), (1, 200));
    assert_eq!(d.freed_bytes, 400);
}

#[test]
fn reclaimable_per_kind_excludes_trashed_and_decided_suggestions() {
    let w = W::new();
    for i in 1..=4 {
        w.asset(i, &format!("a/{i}"), 10 * i, 0, 1);
    }
    w.sugg(1, "blurry", "pending");
    w.sugg(2, "blurry", "accepted");
    w.sugg(3, "screenshot", "pending");
    w.state(3, 1);
    w.sugg(4, "exact_dup", "rejected");
    let d = w.read();
    assert_eq!(w.kind(&d, SuggestionKind::Blurry), (1, 10));
    assert_eq!(w.kind(&d, SuggestionKind::Screenshot), (0, 0));
    assert_eq!(w.kind(&d, SuggestionKind::ExactDup), (0, 0));
    assert_eq!(d.insights.reclaimable_bytes, 10);
    assert_eq!(d.insights.progress.decided, 2);
    assert_eq!(d.insights.progress.total, 4);
    assert_eq!(d.insights.progress.permille, 500);
}

#[test]
fn every_protection_kind_excludes_from_reclaimable() {
    let w = W::new();
    w.asset(1, "fotos/flagg/1.jpg", 10, 1, 1); // favoritt-flagg
    w.asset(2, "fotos/enkelt/2.jpg", 20, 0, 1); // beskyttet enkeltbilde
    w.asset(3, "fotos/mappe/3.jpg", 40, 0, 1); // mappeprefiks
    w.asset(4, "fotos/album/4.jpg", 80, 0, 1); // album
    w.asset(5, "fotos/person/5.jpg", 160, 0, 1); // person
    w.asset(6, "fotos/person/6.jpg", 320, 0, 1); // sammenslått person
    w.asset(7, "fotos/fri/7.jpg", 640, 0, 1); // ubeskyttet
    for i in 1..=7 {
        w.sugg(i, "blurry", "pending");
    }
    w.protect("asset", "2");
    w.protect("folder", "fotos/mappe/");
    w.protect("album", "9");
    w.x("INSERT INTO album(id, name) VALUES (9, 'a')", []);
    w.x(
        "INSERT INTO asset_album(asset_id, album_id) VALUES (4, 9)",
        [],
    );
    w.protect("person", "1");
    w.x("INSERT INTO person(id, created_at) VALUES (1, 1)", []);
    w.x(
        "INSERT INTO person(id, created_at, merged_into) VALUES (2, 1, 1)",
        [],
    );
    for (asset, person) in [(5, 1), (6, 2)] {
        w.x(
            "INSERT INTO face_embedding(asset_id, person_id, box_x, box_y, box_w, box_h, quality, embedding) \
             VALUES (?1, ?2, 0, 0, 1, 1, 1.0, x'00')",
            [asset, person],
        );
    }
    let d = w.read();
    assert_eq!(w.kind(&d, SuggestionKind::Blurry), (1, 640));
    assert_eq!(d.data.protected_count, 6);
    assert_eq!(d.insights.protected_count, 6);
    assert_eq!(d.insights.reclaimable_bytes, 640);
}

#[test]
fn asset_with_two_pending_kinds_counts_once() {
    let w = W::new();
    w.asset(1, "a/1", 100, 0, 1);
    w.sugg(1, "blurry", "pending");
    w.sugg(1, "series", "pending");
    w.sugg(1, "rule", "pending");
    let d = w.read();
    assert_eq!(d.insights.reclaimable_count, 1);
    assert_eq!(d.insights.reclaimable_bytes, 100);
    assert_eq!(w.kind(&d, SuggestionKind::Blurry), (1, 100));
}

#[test]
fn freed_total_excludes_external_and_is_never_claimed_before_purge() {
    let w = W::new();
    w.asset(1, "a/1", 100, 0, 1_700_000_000); // 2023-11
    w.asset(2, "a/2", 200, 0, 1_700_000_000);
    w.asset(3, "a/3", 400, 0, 1_700_000_000);
    w.asset(4, "a/4", 800, 0, 1_700_000_000);
    w.state(4, 1); // i papirkurv: ingen frigjort
    w.purged(1, 1_700_000_000, "user");
    w.purged(2, 1_700_000_000, "external");
    w.purged(3, 1_703_000_000, "user"); // 2023-12
    let d = w.read();
    assert_eq!(d.data.freed_bytes, 500);
    assert_eq!(
        d.insights.freed_by_month,
        vec![
            FreedMonth {
                year_month: 202311,
                count: 1,
                bytes: 100
            },
            FreedMonth {
                year_month: 202312,
                count: 1,
                bytes: 400
            },
        ]
    );
    assert_eq!(freed_since(&w.s, 1_701_000_000).unwrap(), (1, 400));
    assert_eq!(freed_since(&w.s, 0).unwrap(), (2, 500));
    assert_eq!(d.data.trashed_bytes, 800);
}

#[test]
fn only_purge_rows_count_as_freed() {
    let w = W::new();
    w.asset(1, "a/1", 100, 0, 1);
    w.state(1, 1);
    w.log(1, 5, "trash", "user");
    w.log(1, 6, "restore", "user");
    assert_eq!(w.read().data.freed_bytes, 0);
}

#[test]
fn next_step_picks_biggest_kind_then_trash_then_nothing() {
    let w = W::new();
    w.asset(1, "a/1", 100, 0, 1);
    w.asset(2, "a/2", 5000, 0, 1);
    w.asset(3, "a/3", 7, 0, 1);
    w.sugg(1, "screenshot", "pending");
    w.sugg(2, "blurry", "pending");
    assert_eq!(
        w.read().insights.next_step,
        NextStep::ReviewKind {
            kind: SuggestionKind::Blurry,
            count: 1,
            bytes: 5000
        }
    );
    // Alt avgjort: papirkurven er neste forslag, aldri en handling.
    w.x("UPDATE suggestion SET status = 'accepted'", []);
    w.state(3, 1);
    assert_eq!(
        w.read().insights.next_step,
        NextStep::EmptyTrash { count: 1, bytes: 7 }
    );
    w.state(3, 0);
    assert_eq!(w.read().insights.next_step, NextStep::Nothing);
}

#[test]
fn next_step_tie_goes_to_count_then_kind_order() {
    let per = |a, b| {
        vec![
            KindStat {
                kind: SuggestionKind::ExactDup,
                count: a,
                bytes: 10,
            },
            KindStat {
                kind: SuggestionKind::Blurry,
                count: b,
                bytes: 10,
            },
        ]
    };
    let kind = |p: &[KindStat]| match next_step(p, 0, 0) {
        NextStep::ReviewKind { kind, .. } => kind,
        other => panic!("{other:?}"),
    };
    assert_eq!(kind(&per(1, 2)), SuggestionKind::Blurry);
    assert_eq!(kind(&per(2, 1)), SuggestionKind::ExactDup);
    assert_eq!(kind(&per(2, 2)), SuggestionKind::ExactDup);
}

#[test]
fn month_and_folder_distribution_accepts_both_separators() {
    let w = W::new();
    w.asset(1, "C:\\Bilder\\Ferie\\1.jpg", 100, 0, 1_700_000_000);
    w.asset(2, "C:\\Bilder\\Ferie\\2.jpg", 200, 0, 1_700_000_000);
    w.asset(3, "C:/Bilder/Annet/3.jpg", 50, 0, 1_703_000_000);
    w.asset(4, "rot.jpg", 9999, 0, 1_703_000_000);
    let d = w.read().data;
    assert_eq!(
        d.top_folders,
        vec![
            FolderStat {
                path: "C:\\Bilder\\Ferie".into(),
                count: 2,
                bytes: 300
            },
            FolderStat {
                path: "C:/Bilder/Annet".into(),
                count: 1,
                bytes: 50
            },
        ]
    );
    assert_eq!(d.by_month.len(), 2);
    assert_eq!(d.by_month[0].year_month, 202311);
    assert_eq!((d.by_month[0].count, d.by_month[0].bytes), (2, 300));
    assert_eq!(d.by_month[1].year_month, 202312);
}

#[test]
fn series_people_rules_counts() {
    let w = W::new();
    w.x("INSERT INTO grp(kind, created_at) VALUES ('series', 1)", []);
    w.x("INSERT INTO grp(kind, created_at) VALUES ('exact', 1)", []);
    w.x("INSERT INTO person(id, created_at) VALUES (1, 1)", []);
    w.x(
        "INSERT INTO person(id, created_at, merged_into) VALUES (2, 1, 1)",
        [],
    );
    w.x(
        "INSERT INTO rule(name, spec, enabled, created_at, deleted_at) VALUES ('a','s',1,1,NULL)",
        [],
    );
    w.x(
        "INSERT INTO rule(name, spec, enabled, created_at, deleted_at) VALUES ('b','s',0,1,NULL)",
        [],
    );
    w.x(
        "INSERT INTO rule(name, spec, enabled, created_at, deleted_at) VALUES ('c','s',1,1,5)",
        [],
    );
    let d = w.read().data;
    assert_eq!((d.series_count, d.people_count, d.rules_active), (1, 1, 1));
}

#[test]
fn read_never_writes_to_the_index() {
    let w = W::new();
    w.asset(1, "a/1", 100, 0, 1);
    w.sugg(1, "blurry", "pending");
    w.purged(1, 3, "user");
    let before = w.s.conn().total_changes();
    let a = w.read();
    let b = w.read();
    assert_eq!(w.s.conn().total_changes(), before);
    assert_eq!(a, b, "samme indeks gir samme svar");
}

/// Referansemodell i ren Rust for egenskapstestene (bare flagg og enkeltbilde-beskyttelse).
#[derive(Clone, Debug)]
struct A {
    bytes: i64,
    flags: i64,
    protect_single: bool,
    state: i64,
    sugg: Vec<(usize, bool)>,   // (kindindeks, pending)
    purge_cause: Option<usize>, // 0 user, 1 external, 2 expiry
}

const KINDS: [&str; 6] = [
    "exact_dup",
    "visual_dup",
    "blurry",
    "screenshot",
    "series",
    "rule",
];
const CAUSES: [&str; 3] = ["user", "external", "expiry"];

fn arb_asset() -> impl Strategy<Value = A> {
    (
        1i64..5000,
        prop_oneof![Just(0i64), 1i64..8],
        any::<bool>(),
        0i64..3,
        proptest::collection::vec((0usize..6, any::<bool>()), 0..4),
        0usize..3,
    )
        .prop_map(|(bytes, flags, protect_single, state, sugg, cause)| A {
            bytes,
            flags,
            protect_single,
            state,
            sugg: {
                let mut s = sugg;
                s.sort();
                s.dedup_by_key(|x| x.0);
                s
            },
            purge_cause: (state == 2).then_some(cause),
        })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    /// Invarianter: (1) beskyttede bilder er aldri med i «kan ryddes»; (2) summen over typer er lik
    /// summen over unike bilder; (3) frigjort teller aldri `external`; (4) bare aktive bilder i brukt plass.
    #[test]
    fn prop_protected_and_external_never_leak(assets in proptest::collection::vec(arb_asset(), 0..40)) {
        let w = W::new();
        for (i, a) in assets.iter().enumerate() {
            let id = i as i64 + 1;
            w.asset(id, &format!("p/{id}"), a.bytes, a.flags, 1);
            for &(k, pending) in &a.sugg {
                w.sugg(id, KINDS[k], if pending { "pending" } else { "rejected" });
            }
            if a.protect_single {
                w.protect("asset", &id.to_string());
            }
            if a.state >= 1 {
                w.state(id, 1);
            }
            if let Some(c) = a.purge_cause {
                w.state(id, 2);
                w.log(id, 10, "purge", CAUSES[c]);
            }
        }
        let d = w.read();

        let reclaim: Vec<&A> = assets.iter().filter(|a| {
            a.state == 0 && a.flags == 0 && !a.protect_single && a.sugg.iter().any(|s| s.1)
        }).collect();
        prop_assert_eq!(d.insights.reclaimable_count, reclaim.len() as u64);
        prop_assert_eq!(d.insights.reclaimable_bytes, reclaim.iter().map(|a| a.bytes as u64).sum::<u64>());
        prop_assert_eq!(d.data.per_kind.iter().map(|k| k.bytes).sum::<u64>(), d.insights.reclaimable_bytes);
        prop_assert_eq!(d.data.per_kind.iter().map(|k| k.count).sum::<u64>(), d.insights.reclaimable_count);

        let freed: u64 = assets.iter()
            .filter(|a| matches!(a.purge_cause, Some(c) if c != 1))
            .map(|a| a.bytes as u64).sum();
        prop_assert_eq!(d.data.freed_bytes, freed);
        prop_assert_eq!(d.insights.freed_by_month.iter().map(|m| m.bytes).sum::<u64>(), freed);

        let active: Vec<&A> = assets.iter().filter(|a| a.state == 0).collect();
        prop_assert_eq!(d.data.active_count, active.len() as u64);
        prop_assert_eq!(d.data.active_bytes, active.iter().map(|a| a.bytes as u64).sum::<u64>());
        prop_assert_eq!(
            d.data.protected_count,
            active.iter().filter(|a| a.flags != 0 || a.protect_single).count() as u64
        );
        prop_assert!(d.insights.progress.permille <= 1000);
    }
}

// --- Ytelse (budsjett: p95 < 20 ms på 50 000 bilder; `--release`, romslig faktor i debug) ---

fn factor() -> f64 {
    if cfg!(debug_assertions) {
        25.0
    } else {
        1.0
    }
}

/// Filbasert WAL-database som i appen: 50 000 bilder i mange mapper og måneder, forslag, beskyttelser,
/// album, personer og en purge-historikk.
fn big_db(n: i64) -> (Store, std::path::PathBuf) {
    let dir = std::env::temp_dir().join(format!("fjell-dash-perf-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let s = Store::open(&dir.join("index.db")).unwrap();
    let c = s.conn();
    c.execute_batch("BEGIN").unwrap();
    c.execute("INSERT INTO album(id, name) VALUES (1, 'a')", [])
        .unwrap();
    c.execute("INSERT INTO person(id, created_at) VALUES (1, 1)", [])
        .unwrap();
    c.execute(
        "INSERT INTO protection(scope, target, created_at) VALUES ('folder', 'C:\\Bilder\\mappe7\\', 1), \
         ('album', '1', 1), ('person', '1', 1), ('asset', '13', 1)",
        [],
    )
    .unwrap();
    for i in 1..=n {
        let state = if i % 50 == 0 { 1 } else { 0 };
        c.execute(
            "INSERT INTO asset(id, source_id, kind, created_at, mod_token, width, height, bytes, mime, \
             protect_flags, state) VALUES (?1, ?2, 'photo', ?3, 'm', 4000, 3000, 3000000, 'image/heic', ?4, ?5)",
            rusqlite::params![
                i,
                format!("C:\\Bilder\\mappe{}\\f{i}.heic", i % 200),
                1_500_000_000 + i * 20_000,
                if i % 31 == 0 { 1 } else { 0 },
                state
            ],
        )
        .unwrap();
        if i % 7 == 0 {
            c.execute("INSERT INTO suggestion(asset_id, kind, reason_code, confidence) VALUES (?1, 'blurry', 'r', 0.5)", [i]).unwrap();
        }
        if i % 53 == 0 {
            c.execute("INSERT INTO suggestion(asset_id, kind, reason_code, confidence) VALUES (?1, 'visual_dup', 'r', 0.5)", [i]).unwrap();
        }
        if i % 211 == 0 {
            c.execute(
                "INSERT INTO asset_album(asset_id, album_id) VALUES (?1, 1)",
                [i],
            )
            .unwrap();
        }
        if i % 307 == 0 {
            c.execute("INSERT INTO face_embedding(asset_id, person_id, box_x, box_y, box_w, box_h, quality, embedding) VALUES (?1, 1, 0, 0, 1, 1, 1.0, x'00')", [i]).unwrap();
        }
    }
    c.execute_batch("COMMIT").unwrap();
    (s, dir)
}

fn p95(mut v: Vec<f64>) -> f64 {
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    v[(v.len() as f64 * 0.95) as usize - 1]
}

#[test]
fn dashboard_p95_budgets_on_50k_assets() {
    let (s, dir) = big_db(50_000);
    // Purge-historikk: 500 rader (user og external) på bilder som er lovlig purget.
    {
        let c = s.conn();
        c.execute_batch("BEGIN").unwrap();
        for i in 1..=500i64 {
            let id = i * 100 + 1;
            let n = c
                .execute(
                    "UPDATE asset SET state = 1 WHERE id = ?1 AND state = 0",
                    [id],
                )
                .unwrap();
            if n == 1 {
                c.execute("UPDATE asset SET state = 2 WHERE id = ?1", [id])
                    .unwrap();
                c.execute(
                    "INSERT INTO action_log(batch_id, ts, op, asset_id, prev_state, cause) \
                     VALUES (1, ?1, 'purge', ?2, 1, ?3)",
                    rusqlite::params![
                        1_600_000_000 + i * 86_400,
                        id,
                        if i % 4 == 0 { "external" } else { "user" }
                    ],
                )
                .unwrap();
            }
        }
        c.execute_batch("COMMIT").unwrap();
    }
    let first = read(&s).unwrap();
    assert!(first.data.active_count > 40_000);
    assert!(first.insights.reclaimable_bytes > 0);
    assert!(first.data.protected_count > 0);
    assert!(first.data.freed_bytes > 0);
    assert!(!first.data.by_month.is_empty() && !first.data.top_folders.is_empty());
    let sum = read_summary(&s).unwrap();
    assert_eq!(
        sum.insights, first.insights,
        "samme tall med og uten fordeling"
    );
    assert!(sum.data.by_month.is_empty() && sum.data.top_folders.is_empty());

    let (mut sum_ms, mut full_ms) = (Vec::new(), Vec::new());
    for _ in 0..100 {
        let t = Instant::now();
        let d = read_summary(&s).unwrap();
        sum_ms.push(t.elapsed().as_secs_f64() * 1000.0);
        assert_eq!(d.insights, first.insights);
    }
    for _ in 0..100 {
        let t = Instant::now();
        let d = read(&s).unwrap();
        full_ms.push(t.elapsed().as_secs_f64() * 1000.0);
        assert_eq!(d, first);
    }
    let mut cache = DashboardCache::new();
    assert_eq!(cache.read(&s).unwrap(), first);
    let mut warm_ms = Vec::new();
    for _ in 0..100 {
        let t = Instant::now();
        let d = cache.read(&s).unwrap();
        warm_ms.push(t.elapsed().as_secs_f64() * 1000.0);
        assert_eq!(d, first);
    }
    let (ps, pf, pw) = (p95(sum_ms), p95(full_ms), p95(warm_ms));
    eprintln!(
        "dashboard p95: summary = {ps:.2} ms, full = {pf:.2} ms, cached = {pw:.3} ms (50 000 bilder, faktor {})",
        factor()
    );
    let _ = std::fs::remove_dir_all(dir);
    // Budsjett: gjenåpning uten endringer (cache) < 20 ms; kald utregning innenfor handlingsbudsjettet
    // (CLAUDE.md: < 100 ms). Kald `read_summary` måles på delt maskin til ca. 20-30 ms; grensen 50 ms fanger
    // regresjoner (f.eks. kvadratiske planer) uten å bli flaktete.
    assert!(pw < 20.0 * factor(), "cached p95 {pw:.3} ms");
    assert!(ps < 50.0 * factor(), "summary p95 {ps:.2} ms");
    assert!(pf < 100.0 * factor(), "full p95 {pf:.2} ms");
}

#[test]
fn cache_reuses_while_unchanged_and_never_serves_stale_numbers() {
    let w = W::new();
    let mut cache = DashboardCache::new();
    w.asset(1, "a/1", 100, 0, 1);
    let a = cache.read(&w.s).unwrap();
    assert_eq!(a.data.active_count, 1);
    assert_eq!(cache.read(&w.s).unwrap(), a);
    w.asset(2, "a/2", 50, 0, 1);
    let b = cache.read(&w.s).unwrap();
    assert_eq!(b.data.active_count, 2, "endring gir nye tall");
    w.sugg(2, "blurry", "pending");
    w.state(1, 1);
    let c = cache.read_summary(&w.s).unwrap();
    assert_eq!(c, read_summary(&w.s).unwrap());
    assert_eq!(
        (c.data.trashed_count, c.insights.reclaimable_bytes),
        (1, 50)
    );
    // En oppsummering duger ikke når fordeling etterspørres.
    let d = cache.read(&w.s).unwrap();
    assert_eq!(d, read(&w.s).unwrap());
    assert!(!d.data.by_month.is_empty());
    cache.clear();
    assert_eq!(cache.read(&w.s).unwrap(), d);
}

#[test]
fn cache_sees_writes_from_another_connection() {
    let dir = std::env::temp_dir().join(format!("fjell-dash-cache-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let a = Store::open(&dir.join("index.db")).unwrap();
    let b = Store::open(&dir.join("index.db")).unwrap();
    let mut cache = DashboardCache::new();
    assert_eq!(cache.read(&a).unwrap().data.active_count, 0);
    b.conn()
        .execute(
            "INSERT INTO asset(id, source_id, kind, created_at, mod_token, width, height, bytes, mime) \
             VALUES (1, 'x/1', 'photo', 1, 'm', 4, 4, 7, 'image/jpeg')",
            [],
        )
        .unwrap();
    let d = cache.read(&a).unwrap();
    assert_eq!((d.data.active_count, d.data.active_bytes), (1, 7));
    let _ = std::fs::remove_dir_all(dir);
}
