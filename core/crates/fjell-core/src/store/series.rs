//! Lagring for V1-grenen `series` (store, L1). Eier alle SQL-spørringer for serier (`grp` med kind 'series') og `eye_result`.
//!
//! Avhenger av: `super` (`Store`), `crate::model`. Filen eies av grenen `series` (docs/v1/V1-PLAN.md);
//! ingen andre grener rører den. Skjemaet er ferdig i migrering 2 (`002_v1.sql`) og endres ikke av grenen.
//! Grensesnitt: `Store::eye_result_count` (skjelett; grenen legger til resten her).

use super::{Store, StoreError};

impl Store {
    /// Antall rader i `eye_result`.
    pub fn eye_result_count(&self) -> Result<u64, StoreError> {
        let n: i64 = self
            .conn
            .query_row("SELECT count(*) FROM eye_result", [], |r| r.get(0))?;
        Ok(n as u64)
    }
}
