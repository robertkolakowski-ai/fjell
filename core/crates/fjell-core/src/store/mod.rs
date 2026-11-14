//! Modul `store` (L1). Se docs/mvp/ARKITEKTUR.md del 2 og 3.
//!
//! Avhenger av: model (ingen andre moduler).
//! Grensesnitt (hovedpunkter):
//! - `Store::open(path)` / `Store::open_in_memory()`: åpner, setter WAL og migrerer via `PRAGMA user_version`.
//! - `Store::schema_dump()`: sortert dump av `sqlite_master` (brukes av migreringstesten K6-1).
//! - `migrate::MIGRATIONS` / `migrate::migrate_to`: migreringene som SQL-filer (`NNN_*.sql`).
//! - Skriver er én tilkobling (`Store`), lesing bruker egen tilkobling (`Store::reader`).
//!
//! Status (M1): skjema og migrering 1 med triggere.
//! Status (M4, undermoduler `index`, `groups`, `cards`): K6-delene indekseringen trenger (`upsert_assets`,
//! `work_set`, `put_fingerprints`, `index_rows`), utskifting av grupper/forslag og kortspørringer.
//! Status (V1-kontrakt): migrering 2 (`002_v1.sql`) og skjelettmodulene `protection`, `series`, `people`, `rules`,
//! `dashboard` (én eier hver, docs/v1/V1-PLAN.md). `migrate_to` slår av `foreign_keys` rundt tabellomskriving.
//! Status (M3, undermodul `ops`): trygg sletting. `apply_batch` (én transaksjon per batch), `undo_top`,
//! `reconcile_removed`, lesespørringer for plan/utløp/oppsummering.

mod cards;
mod dashboard;
mod groups;
mod index;
#[cfg(test)]
mod index_tests;
mod migrate;
#[cfg(test)]
mod migrate_tests;
mod ops;
mod ops_read;
#[cfg(test)]
mod ops_tests;
mod people;
mod protection;
mod reconcile;
mod rules;
mod series;

use rusqlite::{Connection, OpenFlags};
use std::path::{Path, PathBuf};

pub use cards::{CardRow, SuggestionRow};
pub use groups::{GroupSpec, GroupingStats, LoneSuggestion};
pub use index::{FingerprintRow, HashRow, IndexRow, UpsertStats, WorkItem, MAX_FAILS};
pub use migrate::{migrate_to, MIGRATIONS};
pub use ops::{BatchSpec, Cause, RowOp, UndoResult};
pub use ops_read::{AssetRow, PendingSuggestion};
pub use people::{AssignSlot, FaceRow, PersonRow};
pub use rules::RuleRow;
pub use series::{SeriesMemberRow, SeriesMemberSpec, SeriesSpec, SeriesStats};

/// Feil fra lagringslaget.
#[derive(Debug)]
pub enum StoreError {
    Sqlite(rusqlite::Error),
    /// Databasen har nyere skjemaversjon enn denne kjernen kjenner.
    TooNew {
        found: u32,
        supported: u32,
    },
    /// Raden var ikke i forventet tilstand inne i transaksjonen (batchen er rullet tilbake).
    Conflict(crate::model::AssetId),
    /// `foreign_key_check` fant brudd etter en migrering (migreringen er rullet tilbake).
    ForeignKey {
        table: String,
    },
}

impl From<rusqlite::Error> for StoreError {
    fn from(e: rusqlite::Error) -> Self {
        StoreError::Sqlite(e)
    }
}

impl std::fmt::Display for StoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StoreError::Sqlite(e) => write!(f, "sqlite: {e}"),
            StoreError::TooNew { found, supported } => {
                write!(f, "skjemaversjon {found} er nyere enn støttet {supported}")
            }
            StoreError::Conflict(id) => write!(f, "uventet tilstand for bilde {}", id.0),
            StoreError::ForeignKey { table } => {
                write!(f, "fremmednøkkelbrudd i {table} etter migrering")
            }
        }
    }
}

impl std::error::Error for StoreError {}

/// Indeksen. Eier den ene skrivetilkoblingen.
pub struct Store {
    conn: Connection,
    path: Option<PathBuf>,
}

impl Store {
    /// Åpner (og oppretter) databasen på `path`, aktiverer WAL og migrerer til nyeste versjon.
    pub fn open(path: &Path) -> Result<Store, StoreError> {
        let conn = Connection::open(path)?;
        Self::init(conn, Some(path.to_path_buf()))
    }

    /// Database i minnet (tester). Ingen WAL, ingen separat lesetilkobling.
    pub fn open_in_memory() -> Result<Store, StoreError> {
        Self::init(Connection::open_in_memory()?, None)
    }

    fn init(mut conn: Connection, path: Option<PathBuf>) -> Result<Store, StoreError> {
        if path.is_some() {
            // `journal_mode` returnerer en rad, så den må leses.
            conn.query_row("PRAGMA journal_mode = WAL", [], |_| Ok(()))?;
            conn.pragma_update(None, "synchronous", "NORMAL")?;
        }
        conn.pragma_update(None, "foreign_keys", true)?;
        migrate_to(&mut conn, MIGRATIONS.len() as u32)?;
        Ok(Store { conn, path })
    }

    /// Skrivetilkoblingen (kun brukt fra denne modulen og tester).
    #[cfg(test)]
    pub(crate) fn conn(&self) -> &Connection {
        &self.conn
    }

    /// Egen skrivebeskyttet tilkobling for lesing (blokkeres ikke av skriver i WAL). `None` for minnedatabase.
    pub fn reader(&self) -> Result<Option<Connection>, StoreError> {
        match &self.path {
            None => Ok(None),
            Some(p) => {
                let flags = OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX;
                Ok(Some(Connection::open_with_flags(p, flags)?))
            }
        }
    }

    /// Gjeldende `PRAGMA user_version`.
    pub fn user_version(&self) -> Result<u32, StoreError> {
        Ok(self
            .conn
            .pragma_query_value(None, "user_version", |r| r.get(0))?)
    }

    /// Sortert dump av alle skjemaobjekter (`type|name|sql`), uavhengig av opprettelsesrekkefølge.
    pub fn schema_dump(&self) -> Result<String, StoreError> {
        migrate::schema_dump(&self.conn)
    }
}
