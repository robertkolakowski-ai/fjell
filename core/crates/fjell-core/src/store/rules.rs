//! Lagring for V1-grenen `rules` (store, L1). Eier alle SQL-spørringer for `rule` og `rule_hit`.
//!
//! Avhenger av: `super` (`Store`), `crate::model`. Filen eies av grenen `rules` (docs/v1/V1-PLAN.md);
//! ingen andre grener rører den. Skjemaet er ferdig i migrering 2 (`002_v1.sql`) og endres ikke av grenen.
//! Grensesnitt: `Store::rule_count` (skjelett; grenen legger til resten her).

use super::{Store, StoreError};

impl Store {
    /// Antall rader i `rule`.
    pub fn rule_count(&self) -> Result<u64, StoreError> {
        let n: i64 = self
            .conn
            .query_row("SELECT count(*) FROM rule", [], |r| r.get(0))?;
        Ok(n as u64)
    }
}
