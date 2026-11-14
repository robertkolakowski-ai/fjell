//! Tester for migrering (K6-1) og triggere (I2, I3, I14).

use super::*;
use rusqlite::Connection;

fn dump_of(conn: &Connection) -> String {
    migrate::schema_dump(conn).unwrap()
}

fn tmp_db(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("fjell-store-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir.join("index.db")
}

/// K6-1: fra tom database og fra hver tidligere versjon gir lik skjemadump.
#[test]
fn k6_1_migration_from_empty_and_from_previous_version_give_equal_dump() {
    let fresh = Store::open_in_memory().unwrap();
    let expected = fresh.schema_dump().unwrap();
    assert!(expected.contains("table|asset|"));
    assert!(expected.contains("trigger|action_log_no_delete|"));
    let latest = MIGRATIONS.len() as u32;
    assert_eq!(fresh.user_version().unwrap(), latest);

    for from in 0..latest {
        let mut conn = Connection::open_in_memory().unwrap();
        migrate_to(&mut conn, from).unwrap();
        assert_eq!(
            conn.pragma_query_value(None, "user_version", |r| r.get::<_, u32>(0))
                .unwrap(),
            from
        );
        migrate_to(&mut conn, latest).unwrap();
        assert_eq!(dump_of(&conn), expected, "migrert fra versjon {from}");
    }
}

/// Åpning av eksisterende fil er idempotent og lar skjemaet være uendret.
#[test]
fn reopen_keeps_schema_and_data() {
    let path = tmp_db("reopen");
    let first = Store::open(&path).unwrap();
    let dump = first.schema_dump().unwrap();
    first
        .conn()
        .execute("INSERT INTO setting(key, value) VALUES ('k', 'v')", [])
        .unwrap();
    drop(first);
    let second = Store::open(&path).unwrap();
    assert_eq!(second.schema_dump().unwrap(), dump);
    let v: String = second
        .conn()
        .query_row("SELECT value FROM setting WHERE key='k'", [], |r| r.get(0))
        .unwrap();
    assert_eq!(v, "v");
}

#[test]
fn newer_database_is_rejected() {
    let path = tmp_db("toonew");
    Connection::open(&path)
        .unwrap()
        .pragma_update(None, "user_version", 99)
        .unwrap();
    assert!(matches!(
        Store::open(&path),
        Err(StoreError::TooNew { found: 99, .. })
    ));
}

#[test]
fn file_database_uses_wal_and_separate_reader() {
    let store = Store::open(&tmp_db("wal")).unwrap();
    let mode: String = store
        .conn()
        .query_row("PRAGMA journal_mode", [], |r| r.get(0))
        .unwrap();
    assert_eq!(mode, "wal");
    let reader = store.reader().unwrap().expect("lesetilkobling");
    let n: i64 = reader
        .query_row("SELECT count(*) FROM asset", [], |r| r.get(0))
        .unwrap();
    assert_eq!(n, 0);
    assert!(reader
        .execute("INSERT INTO setting(key, value) VALUES ('a','b')", [])
        .is_err());
}

fn insert_asset(conn: &Connection, state: i64) -> i64 {
    conn.execute(
        "INSERT INTO asset(source_id, kind, created_at, mod_token, width, height, bytes, mime, state) \
         VALUES (?1, 'photo', 0, 't', 1, 1, 1, 'image/jpeg', ?2)",
        rusqlite::params![format!("src-{state}-{}", rand_suffix(conn)), state],
    )
    .unwrap();
    conn.last_insert_rowid()
}

fn rand_suffix(conn: &Connection) -> i64 {
    conn.query_row("SELECT count(*) FROM asset", [], |r| r.get(0))
        .unwrap()
}

fn set_state(conn: &Connection, id: i64, state: i64) -> rusqlite::Result<usize> {
    conn.execute(
        "UPDATE asset SET state = ?1 WHERE id = ?2",
        rusqlite::params![state, id],
    )
}

/// I2 og I3: 0 -> 2 blokkert, 1 -> 2 tillatt, purged er terminal.
#[test]
fn state_triggers_block_direct_purge_and_revival() {
    let store = Store::open_in_memory().unwrap();
    let c = store.conn();
    let active = insert_asset(c, 0);
    assert!(set_state(c, active, 2).is_err(), "0 -> 2 skal avvises");
    set_state(c, active, 1).unwrap();
    set_state(c, active, 0).unwrap();
    set_state(c, active, 1).unwrap();
    set_state(c, active, 2).unwrap();
    assert!(set_state(c, active, 0).is_err(), "purged er terminal");
    assert!(set_state(c, active, 1).is_err(), "purged er terminal");
}

/// I14: action_log er append-only.
#[test]
fn action_log_is_append_only() {
    let store = Store::open_in_memory().unwrap();
    let c = store.conn();
    let id = insert_asset(c, 0);
    c.execute(
        "INSERT INTO action_log(batch_id, ts, op, asset_id, prev_state, cause) VALUES (1, 0, 'trash', ?1, 0, 'user')",
        [id],
    )
    .unwrap();
    assert!(c
        .execute("UPDATE action_log SET op = 'restore'", [])
        .is_err());
    assert!(c.execute("DELETE FROM action_log", []).is_err());
    let n: i64 = c
        .query_row("SELECT count(*) FROM action_log", [], |r| r.get(0))
        .unwrap();
    assert_eq!(n, 1);
}

fn table_count(conn: &Connection, table: &str) -> i64 {
    conn.query_row(&format!("SELECT count(*) FROM {table}"), [], |r| r.get(0))
        .unwrap()
}

/// 005 skriver om `grp` og `suggestion` uten å miste rader eller utløse kaskade, selv med FK på.
/// Utgangspunktet er versjon 4 (main): kolonnene `keeper_reason`/`keeper_rule` og indeksene fra 002 og 003 består.
#[test]
fn v5_rewrite_keeps_rows_triggers_and_foreign_keys() {
    let mut conn = Connection::open_in_memory().unwrap();
    conn.pragma_update(None, "foreign_keys", true).unwrap();
    migrate_to(&mut conn, 4).unwrap();
    let a = insert_asset(&conn, 0);
    let b = insert_asset(&conn, 0);
    conn.execute(
        "INSERT INTO grp(id, kind, keeper_id, created_at, keeper_reason, keeper_rule) \
         VALUES (7, 'visual', ?1, 5, NULL, 'prefer_folder:C%3A/Foto')",
        [a],
    )
    .unwrap();
    for (asset, rank) in [(a, 0), (b, 1)] {
        conn.execute(
            "INSERT INTO grp_member(grp_id, asset_id, rank, score) VALUES (7, ?1, ?2, 0.5)",
            [asset, rank],
        )
        .unwrap();
    }
    conn.execute(
        "INSERT INTO suggestion(id, asset_id, kind, grp_id, reason_code, confidence, status) \
         VALUES (3, ?1, 'visual_dup', 7, 'visual_duplicate', 0.9, 'rejected')",
        [b],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO action_log(batch_id, ts, op, asset_id, prev_state, cause) VALUES (1, 0, 'keep', ?1, 0, 'user')",
        [a],
    )
    .unwrap();

    migrate_to(&mut conn, 5).unwrap();

    assert_eq!(table_count(&conn, "grp"), 1);
    let (keeper, reason, rule): (i64, Option<String>, Option<String>) = conn
        .query_row(
            "SELECT keeper_id, keeper_reason, keeper_rule FROM grp WHERE id = 7",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap();
    assert_eq!(
        (keeper, reason, rule.as_deref()),
        (a, None, Some("prefer_folder:C%3A/Foto"))
    );
    let dump = dump_of(&conn);
    for index in [
        "index|idx_suggestion_cards|",
        "index|idx_suggestion_asset|",
        "index|idx_suggestion_grp|",
        "index|idx_grp_member_asset|",
    ] {
        assert!(dump.contains(index), "{index} mangler etter 005");
    }
    assert_eq!(
        table_count(&conn, "grp_member"),
        2,
        "ingen kaskade ved DROP"
    );
    let (kind, grp_id, status): (String, i64, String) = conn
        .query_row(
            "SELECT kind, grp_id, status FROM suggestion WHERE id = 3",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap();
    assert_eq!(
        (kind.as_str(), grp_id, status.as_str()),
        ("visual_dup", 7, "rejected")
    );
    let fk: bool = conn
        .pragma_query_value(None, "foreign_keys", |r| r.get(0))
        .unwrap();
    assert!(fk, "foreign_keys gjenopprettes etter migrering");

    // Nye forslags- og gruppetyper er tillatt; ukjente er det fortsatt ikke.
    conn.execute("INSERT INTO grp(kind, created_at) VALUES ('series', 1)", [])
        .unwrap();
    assert!(conn
        .execute("INSERT INTO grp(kind, created_at) VALUES ('nope', 1)", [])
        .is_err());
    for kind in ["series", "rule"] {
        conn.execute(
            "INSERT INTO suggestion(asset_id, kind, reason_code, confidence) VALUES (?1, ?2, 'x', 0.5)",
            rusqlite::params![a, kind],
        )
        .unwrap();
    }
    assert!(conn
        .execute(
            "INSERT INTO suggestion(asset_id, kind, reason_code, confidence) VALUES (?1, 'bogus', 'x', 0.5)",
            [a],
        )
        .is_err());
    // Triggere fra 001 lever videre (I2, I14).
    assert!(set_state(&conn, a, 2).is_err());
    assert!(conn.execute("DELETE FROM action_log", []).is_err());
}

/// V1-tabellene finnes, beskyttelse er unik per (scope, target), regler slettes aldri hardt, ansiktsindeks er av.
#[test]
fn v5_tables_defaults_and_guards() {
    let store = Store::open_in_memory().unwrap();
    let c = store.conn();
    for t in [
        "protection",
        "eye_result",
        "person",
        "face_embedding",
        "rule",
        "rule_hit",
    ] {
        assert_eq!(table_count(c, t), 0, "{t}");
    }
    let v: String = c
        .query_row(
            "SELECT value FROM setting WHERE key = 'face_index_enabled'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(v, "0");
    c.execute(
        "INSERT INTO protection(scope, target, created_at) VALUES ('folder', 'C:/Foto', 1)",
        [],
    )
    .unwrap();
    assert!(c
        .execute(
            "INSERT INTO protection(scope, target, created_at) VALUES ('folder', 'C:/Foto', 2)",
            [],
        )
        .is_err());
    assert!(c
        .execute(
            "INSERT INTO protection(scope, target, created_at) VALUES ('tag', 'x', 2)",
            [],
        )
        .is_err());
    c.execute(
        "INSERT INTO rule(name, spec, created_at) VALUES ('r', 's', 1)",
        [],
    )
    .unwrap();
    assert!(c.execute("DELETE FROM rule", []).is_err());
    assert!(
        c.execute("UPDATE person SET merged_into = id", []).is_ok(),
        "ingen rader ennå"
    );
    c.execute("INSERT INTO person(id, created_at) VALUES (1, 1)", [])
        .unwrap();
    assert!(c
        .execute("UPDATE person SET merged_into = 1 WHERE id = 1", [])
        .is_err());
    assert_eq!(store.protection_count().unwrap(), 1);
    assert_eq!(store.rule_count().unwrap(), 1);
    assert_eq!(store.eye_result_count().unwrap(), 0);
    assert_eq!(store.face_count().unwrap(), 0);
    assert_eq!(store.active_asset_count().unwrap(), 0);
}

/// Stort bibliotek: `DELETE FROM grp` setter `suggestion.grp_id` til NULL (FK `ON DELETE SET NULL`); uten indeks på
/// kolonnen skanner SQLite hele forslagstabellen per slettet gruppe. Spørringsplanen skal bruke indeksen.
#[test]
fn suggestion_grp_id_has_index_for_group_deletes() {
    let store = Store::open_in_memory().unwrap();
    assert!(store
        .schema_dump()
        .unwrap()
        .contains("index|idx_suggestion_grp|"));
    let plan: Vec<String> = store
        .conn()
        .prepare("EXPLAIN QUERY PLAN SELECT id FROM suggestion WHERE grp_id = 1")
        .unwrap()
        .query_map([], |r| r.get::<_, String>(3))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert!(
        plan.iter().any(|p| p.contains("idx_suggestion_grp")),
        "{plan:?}"
    );
}

/// Rekkefølgen er bindende: en database fra main på versjon 3 (indeksen fra stort bibliotek) oppgraderes additivt
/// til versjon 4 (regelmotorens `grp.keeper_rule`) uten at indeksen eller data røres.
#[test]
fn v3_database_upgrades_to_keeper_rule_without_losing_index_or_data() {
    let mut conn = Connection::open_in_memory().unwrap();
    migrate_to(&mut conn, 3).unwrap();
    let v3 = dump_of(&conn);
    assert!(v3.contains("index|idx_suggestion_grp|"));
    assert!(!v3.contains("keeper_rule"));
    conn.execute("INSERT INTO setting(key, value) VALUES ('k', 'v')", [])
        .unwrap();

    migrate_to(&mut conn, MIGRATIONS.len() as u32).unwrap();
    let latest = dump_of(&conn);
    assert!(latest.contains("index|idx_suggestion_grp|"));
    assert!(latest.contains("keeper_rule"));
    let v: String = conn
        .query_row("SELECT value FROM setting WHERE key='k'", [], |r| r.get(0))
        .unwrap();
    assert_eq!(v, "v");
}

/// En eksisterende database fra main (versjon 4, med keeper-regler, grupper, forslag, papirkurv og logg) åpnes
/// av denne kjernen og oppgraderes til siste versjon uten tap: hver rad i hver tabell fra versjon 4 er uendret,
/// keeper-reglene (`setting.keeper_rules`, `grp.keeper_reason`, `grp.keeper_rule`) leses som før, og skjemaet er
/// likt en fersk database.
#[test]
fn main_v4_database_upgrades_to_latest_without_loss() {
    use rusqlite::OptionalExtension;
    use std::collections::BTreeMap;
    const TABLES: [(&str, &str); 7] = [
        ("asset", "id"),
        ("fingerprint", "asset_id"),
        ("grp", "id"),
        ("grp_member", "grp_id, asset_id"),
        ("suggestion", "id"),
        ("action_log", "id"),
        ("setting", "key"),
    ];
    // Alle kolonner som fantes i versjon 4, rad for rad, som tekst.
    fn rows(conn: &Connection, cols: &BTreeMap<&str, Vec<String>>) -> Vec<String> {
        let mut out = Vec::new();
        for (table, order) in TABLES {
            let list = cols[table].join(", ");
            let quoted = cols[table]
                .iter()
                .map(|c| format!("quote({c})"))
                .collect::<Vec<_>>()
                .join(" || '|' || ");
            let mut st = conn
                .prepare(&format!(
                    "SELECT {quoted} FROM (SELECT {list} FROM {table} ORDER BY {order})"
                ))
                .unwrap();
            let got: Vec<String> = st
                .query_map([], |r| r.get(0))
                .unwrap()
                .collect::<Result<_, _>>()
                .unwrap();
            out.extend(got.into_iter().map(|r| format!("{table}|{r}")));
        }
        out
    }
    let path = tmp_db("v4-upgrade");
    let keeper_rules = "prefer_folder\tC%3A/Foto\t5\nprotect_folder\tC%3A/Arkiv\nprefer_newest\n";
    let (before, cols) = {
        let mut conn = Connection::open(&path).unwrap();
        conn.pragma_update(None, "foreign_keys", true).unwrap();
        migrate_to(&mut conn, 4).unwrap();
        let a = insert_asset(&conn, 0);
        let b = insert_asset(&conn, 0);
        let c = insert_asset(&conn, 0);
        let d = insert_asset(&conn, 0);
        for id in [a, b, c] {
            conn.execute(
                "INSERT INTO fingerprint(asset_id, content_hash, dhash, phash, blur_score, shot_signals, algo_ver) \
                 VALUES (?1, zeroblob(32), 1, 2, 0.5, 0, 1)",
                [id],
            )
            .unwrap();
        }
        // Gruppe valgt av en keeper-regel, og en valgt av K5.
        conn.execute(
            "INSERT INTO grp(id, kind, keeper_id, created_at, keeper_reason, keeper_rule) \
             VALUES (7, 'exact', ?1, 5, NULL, 'prefer_folder:C%3A/Foto')",
            [a],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO grp(id, kind, keeper_id, created_at, keeper_reason, keeper_rule) \
             VALUES (8, 'visual', ?1, 6, 'sharper', NULL)",
            [c],
        )
        .unwrap();
        for (grp, asset, rank) in [(7, a, 0), (7, b, 1), (8, c, 0), (8, d, 1)] {
            conn.execute(
                "INSERT INTO grp_member(grp_id, asset_id, rank, score) VALUES (?1, ?2, ?3, 0.5)",
                [grp, asset, rank],
            )
            .unwrap();
        }
        conn.execute(
            "INSERT INTO suggestion(id, asset_id, kind, grp_id, reason_code, confidence, status) \
             VALUES (3, ?1, 'exact_dup', 7, 'exact_copy', 1.0, 'accepted')",
            [b],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO suggestion(id, asset_id, kind, grp_id, reason_code, confidence, status) \
             VALUES (4, ?1, 'visual_dup', 8, 'visual_duplicate', 0.9, 'pending')",
            [d],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO suggestion(id, asset_id, kind, grp_id, reason_code, confidence, status) \
             VALUES (5, ?1, 'blurry', NULL, 'blurry', 0.7, 'rejected')",
            [c],
        )
        .unwrap();
        // b ligger i papirkurven (angrebar), med logg.
        conn.execute(
            "INSERT INTO action_log(batch_id, ts, op, asset_id, prev_state, cause, suggestion_id) \
             VALUES (1, 10, 'trash', ?1, 0, 'user', 3)",
            [b],
        )
        .unwrap();
        set_state(&conn, b, 1).unwrap();
        conn.execute(
            "INSERT INTO setting(key, value) VALUES ('keeper_rules', ?1)",
            [keeper_rules],
        )
        .unwrap();
        let mut cols = BTreeMap::new();
        for (table, _) in TABLES {
            let names: Vec<String> = conn
                .prepare(&format!("SELECT name FROM pragma_table_info('{table}')"))
                .unwrap()
                .query_map([], |r| r.get(0))
                .unwrap()
                .collect::<Result<_, _>>()
                .unwrap();
            assert!(!names.is_empty(), "{table}");
            cols.insert(table, names);
        }
        (rows(&conn, &cols), cols)
    };
    assert!(before.len() >= 16, "testdata mangler: {before:?}");

    let store = Store::open(&path).unwrap();
    assert_eq!(store.user_version().unwrap(), MIGRATIONS.len() as u32);
    assert_eq!(
        store.schema_dump().unwrap(),
        Store::open_in_memory().unwrap().schema_dump().unwrap(),
        "oppgradert skjema er likt et ferskt"
    );
    let mut after = rows(store.conn(), &cols);
    // Eneste nye rad i tabellene fra versjon 4: standardverdien for ansiktsindeks (av).
    let added: Vec<String> = after
        .iter()
        .filter(|r| !before.contains(r))
        .cloned()
        .collect();
    assert_eq!(added, vec!["setting|'face_index_enabled'|'0'".to_string()]);
    after.retain(|r| before.contains(r));
    assert_eq!(after, before, "ingen rad endret eller tapt");
    // Keeper-reglene fra main leses uendret gjennom lagringsgrensesnittet.
    assert_eq!(
        store.setting(KEEPER_RULES_KEY).unwrap().as_deref(),
        Some(keeper_rules)
    );
    let groups = store.rule_groups().unwrap();
    assert_eq!(groups.len(), 1, "bare gruppe 8 har alle medlemmer aktive");
    assert_eq!(groups[0].id.0, 8);
    let broken: Option<String> = store
        .conn()
        .query_row("PRAGMA foreign_key_check", [], |r| r.get(0))
        .optional()
        .unwrap();
    assert_eq!(broken, None);
    // Papirkurven er urørt og fortsatt angrebar (tilstand 1 -> 0 er lov).
    let trashed: i64 = store
        .conn()
        .query_row("SELECT count(*) FROM asset WHERE state = 1", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(trashed, 1);
    // V1-tabellene er tomme: ingen forslagsregler, ingen beskyttelse, ingen ansikter.
    for t in [
        "protection",
        "eye_result",
        "person",
        "face_embedding",
        "rule",
        "rule_hit",
    ] {
        assert_eq!(table_count(store.conn(), t), 0, "{t}");
    }
}
