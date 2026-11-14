//! Migreringer for `store`: hver migrering er én SQL-fil, versjon = indeks + 1 i `PRAGMA user_version`.
//!
//! Grensesnitt: `MIGRATIONS` (rekkefølgen er bindende), `migrate_to(conn, target)`, `schema_dump(conn)`.
//! Hver migrering kjøres i én transaksjon sammen med oppdatering av `user_version`.
//! `foreign_keys` slås av utenfor transaksjonen (kan ikke endres inni) slik at tabellomskriving ikke
//! kaskaderer; `foreign_key_check` kjøres før commit og avbryter migreringen ved brudd.

use super::StoreError;
use rusqlite::{Connection, OptionalExtension};

/// Migreringene i rekkefølge. Legg nye filer til sist; endre aldri en utgitt migrering.
pub const MIGRATIONS: [&str; 2] = [include_str!("001_init.sql"), include_str!("002_v1.sql")];

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
    if current >= target.min(supported) {
        return Ok(());
    }
    let fk_was_on: bool = conn.pragma_query_value(None, "foreign_keys", |r| r.get(0))?;
    if fk_was_on {
        conn.pragma_update(None, "foreign_keys", false)?;
    }
    let result = run_migrations(conn, current, target.min(supported));
    if fk_was_on {
        conn.pragma_update(None, "foreign_keys", true)?;
    }
    result?;
    Ok(())
}

fn run_migrations(conn: &mut Connection, current: u32, last: u32) -> Result<(), StoreError> {
    for version in current + 1..=last {
        let tx = conn.transaction()?;
        tx.execute_batch(MIGRATIONS[(version - 1) as usize])?;
        let broken: Option<String> = tx
            .query_row("PRAGMA foreign_key_check", [], |r| r.get(0))
            .optional()?;
        if let Some(table) = broken {
            return Err(StoreError::ForeignKey { table });
        }
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
