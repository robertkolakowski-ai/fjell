//! Utløp og oppsummering av papirkurven (PLAN-TRYGGHET del 3). Del av `actions`.
//!
//! Grensesnitt: `Actions::expire_due() -> Option<ExpiryNotice>`, `Actions::trash_summary()`.
//! `expire_due` skriver bare `expired_notified_at`; tid fører aldri til sletting (I9) og hvert
//! element varsles én gang (I10).

use super::{Actions, Result};
use crate::model::ExpiryNotice;

/// Tre tall til UI: merket, kan frigjøres, frigjort (P4).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct TrashSummary {
    pub trashed_count: u64,
    /// «Kan frigjøres».
    pub trashed_bytes: u64,
    pub expired_count: u64,
    /// «Frigjort»: bare det Fjell har fått plattformen til å slette (ikke `external`).
    pub freed_bytes: u64,
}

impl Actions {
    pub fn expire_due(&mut self) -> Result<Option<ExpiryNotice>> {
        let (count, bytes) = self.store.mark_expired_notified(self.clock.now())?;
        Ok((count > 0).then_some(ExpiryNotice { count, bytes }))
    }

    pub fn trash_summary(&self) -> Result<TrashSummary> {
        let (trashed_count, trashed_bytes, expired_count) =
            self.store.trash_totals(self.clock.now())?;
        Ok(TrashSummary {
            trashed_count,
            trashed_bytes,
            expired_count,
            freed_bytes: self.store.freed_bytes()?,
        })
    }
}
