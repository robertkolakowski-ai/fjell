//! Lagring for V1-grenen `people` (store, L1). Eier alle SQL-spørringer for `person` og `face_embedding`.
//!
//! Avhenger av: `super` (`Store`), `crate::model`. Filen eies av grenen `people` (docs/v1/V1-PLAN.md);
//! ingen andre grener rører den. Skjemaet er ferdig i migrering 2 (`002_v1.sql`) og endres ikke av grenen.
//! Grensesnitt: `Store::face_count` (skjelett; grenen legger til resten her).

use super::{Store, StoreError};

impl Store {
    /// Antall rader i `face_embedding`.
    pub fn face_count(&self) -> Result<u64, StoreError> {
        let n: i64 = self
            .conn
            .query_row("SELECT count(*) FROM face_embedding", [], |r| r.get(0))?;
        Ok(n as u64)
    }
}
