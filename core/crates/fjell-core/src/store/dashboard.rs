//! Lagring for V1-grenen `dashboard` (store, L1). Eier aggregeringsspørringene for `DashboardData`.
//!
//! Avhenger av: `super` (`Store`), `crate::model`. Filen eies av grenen `dashboard` (docs/v1/V1-PLAN.md);
//! ingen andre grener rører den. Bare lesing; ingen skriving til indeksen.
//! Grensesnitt: `Store::active_asset_count` (skjelett; grenen legger til aggregatene her).

use super::{Store, StoreError};

impl Store {
    /// Antall aktive bilder (`state = 0`).
    pub fn active_asset_count(&self) -> Result<u64, StoreError> {
        let n: i64 =
            self.conn
                .query_row("SELECT count(*) FROM asset WHERE state = 0", [], |r| {
                    r.get(0)
                })?;
        Ok(n as u64)
    }
}
