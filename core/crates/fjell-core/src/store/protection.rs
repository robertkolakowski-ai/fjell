//! Lagring for V1-grenen `protect` (store, L1). Eier alle SQL-spørringer for tabellen `protection`.
//!
//! Avhenger av: `super` (`Store`), `crate::model`. Filen eies av grenen `protect` (docs/v1/V1-PLAN.md);
//! ingen andre grener rører den. Skjemaet er ferdig i migrering 2 (`002_v1.sql`) og endres ikke av grenen.
//! Grensesnitt: `Store::protection_count` (skjelett; grenen legger til resten her).

use super::{Store, StoreError};

impl Store {
    /// Antall rader i `protection`.
    pub fn protection_count(&self) -> Result<u64, StoreError> {
        let n: i64 = self
            .conn
            .query_row("SELECT count(*) FROM protection", [], |r| r.get(0))?;
        Ok(n as u64)
    }
}
