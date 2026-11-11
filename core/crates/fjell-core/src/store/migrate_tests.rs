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
