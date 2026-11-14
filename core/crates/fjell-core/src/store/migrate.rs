//! Migreringer for `store`: hver migrering er én SQL-fil, versjon = indeks + 1 i `PRAGMA user_version`.
//!
//! Grensesnitt: `MIGRATIONS` (rekkefølgen er bindende), `migrate_to(conn, target)`, `schema_dump(conn)`.
//! Hver migrering kjøres i én transaksjon sammen med oppdatering av `user_version`.

use super::StoreError;
use rusqlite::Connection;

/// Migreringene i rekkefølge. Legg nye filer til sist; endre aldri en utgitt migrering.
pub const MIGRATIONS: [&str; 4] = [
    include_str!("001_init.sql"),
    include_str!("002_keeper_reason.sql"),
    include_str!("003_suggestion_grp_index.sql"),
    include_str!("004_keeper_rule.sql"),
];

/// Migrerer `conn` opp til `target` (1-basert, `0` = ingen). Gjør ingenting hvis versjonen allerede er nådd.
pub fn migrate_to(conn: &mut Connection, target: u32) -> Result<(), StoreError> {
    let supported = MIGRATIONS.len() as u32;
    let current: u32 = conn.pragma_query_value(None, "user_version", |r| r.get(0))?;
    if current > supported {
        return Err(StoreError::TooNew {
            found: current,
            supported,
        });
    }
    for version in current + 1..=target.min(supported) {
        let tx = conn.transaction()?;
        tx.execute_batch(MIGRATIONS[(version - 1) as usize])?;
        // `PRAGMA` tar ikke parametre; versjonen er et heltall vi selv eier.
        tx.execute_batch(&format!("PRAGMA user_version = {version}"))?;
        tx.commit()?;
    }
    Ok(())
}

/// Sortert dump av `sqlite_master` (uten interne `sqlite_*`-objekter), én linje per objekt.
pub fn schema_dump(conn: &Connection) -> Result<String, StoreError> {
    let mut stmt = conn.prepare(
        "SELECT type, name, sql FROM sqlite_master \
         WHERE name NOT LIKE 'sqlite_%' ORDER BY type, name",
    )?;
    let rows = stmt.query_map([], |r| {
        let sql: Option<String> = r.get(2)?;
        Ok(format!(
            "{}|{}|{}",
            r.get::<_, String>(0)?,
            r.get::<_, String>(1)?,
            sql.unwrap_or_default()
        ))
    })?;
    let mut out = String::new();
    for row in rows {
        out.push_str(&row?);
        out.push('\n');
    }
    Ok(out)
}
