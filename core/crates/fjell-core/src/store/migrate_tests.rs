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

/// 002 skriver om `grp` og `suggestion` uten å miste rader eller utløse kaskade, selv med FK på.
#[test]
fn v2_rewrite_keeps_rows_triggers_and_foreign_keys() {
    let mut conn = Connection::open_in_memory().unwrap();
    conn.pragma_update(None, "foreign_keys", true).unwrap();
    migrate_to(&mut conn, 1).unwrap();
    let a = insert_asset(&conn, 0);
    let b = insert_asset(&conn, 0);
    conn.execute(
        "INSERT INTO grp(id, kind, keeper_id, created_at) VALUES (7, 'visual', ?1, 5)",
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

    migrate_to(&mut conn, 2).unwrap();

    assert_eq!(table_count(&conn, "grp"), 1);
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
fn v2_tables_defaults_and_guards() {
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
