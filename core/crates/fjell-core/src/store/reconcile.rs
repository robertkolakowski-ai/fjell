//! Avstemming av kilder som er fjernet utenfor Fjell (PLAN-TRYGGHET del 2.1, I2, T11, T12, beslutning 18).
//!
//! Valg: spesifikasjonen (2.1, I2, T11/T12, ARKITEKTUR del 2) krever endepunktet `purged` med
//! `cause=external` for bilder som er borte fra kilden. Det er trygt fordi Fjell ikke sletter noe her:
//! originalen er allerede borte (plattformen eller brukeren fjernet den), ingen indeks- eller
//! loggrader slettes, og `asset`-raden beholdes (`purged` = «kilden finnes ikke lenger»). Ingen
//! plattform-`delete` kalles, og `Confirmed` omgås ikke fordi det ikke finnes noe å bekrefte.
//! Tilleggsregel her: hvert tilstandsskifte logges. Et aktivt bilde får to rader i samme batch
//! (`trash` active->trashed, så `purge` trashed->purged, begge `cause=external`), slik at
//! mellomtrinnet (som trigger I2 krever) aldri er usynlig i `action_log`. Batcher med
//! `cause=external` ligger aldri på angre-stakken.
//!
//! Avhenger av: `super::ops` (`log_row`, `next_batch_id`, `Cause`) og `super::ops_read::row_of`.

use super::ops::{log_row, next_batch_id, Cause};
use super::ops_read::row_of;
use super::{Store, StoreError};
use crate::model::{AssetId, SourceId, State};
use rusqlite::OptionalExtension;

impl Store {
    /// Avstemmer borte kilder til `purged` (`cause=external`) i én batch. Ukjente og allerede purgede
    /// kilder hoppes over. Alle tilstandsskifter logges, også `active -> trashed`.
    pub fn reconcile_removed(
        &mut self,
        removed: &[SourceId],
        ts: i64,
    ) -> Result<Vec<AssetId>, StoreError> {
        self.generation += 1;
        let tx = self.conn.transaction()?;
        let batch = next_batch_id(&tx)?;
        let mut purged = Vec::new();
        for src in removed {
            let found: Option<i64> = tx
                .prepare_cached("SELECT id FROM asset WHERE source_id = ?1")?
                .query_row([&src.0], |r| r.get(0))
                .optional()?;
            let Some(aid) = found else { continue };
            let id = AssetId(aid as u64);
            let Some(row) = row_of(&tx, id)? else {
                continue;
            };
            if row.state == State::Purged {
                continue;
            }
            if row.state == State::Active {
                tx.execute("UPDATE asset SET state = 1 WHERE id = ?1", [aid])?;
                let ext = Cause::External;
                log_row(&tx, batch, ts, "trash", id, State::Active, None, None, ext)?;
            }
            tx.execute("UPDATE asset SET state = 2 WHERE id = ?1", [aid])?;
            let ext = Cause::External;
            log_row(&tx, batch, ts, "purge", id, State::Trashed, None, None, ext)?;
            purged.push(id);
        }
        tx.commit()?;
        Ok(purged)
    }
}

impl Store {
    /// Filen ble flyttet tilbake fra karantenen til et annet navn (navnekollisjon ved gjenoppretting):
    /// indeksen skal peke på filen som faktisk har innholdet, ikke på den opprinnelige stien, der det nå
    /// ligger en annen fil (F1). Endrer bare `source_id`; `false` hvis id er ukjent. Opptatt `source_id`
    /// (annen rad) gir feil og ingen endring.
    pub fn relocate_source(&mut self, id: AssetId, to: &SourceId) -> Result<bool, StoreError> {
        let n = self.conn.execute(
            "UPDATE asset SET source_id = ?2 WHERE id = ?1 AND source_id <> ?2",
            rusqlite::params![id.0 as i64, to.0],
        )?;
        if n > 0 {
            self.generation += 1;
        }
        Ok(n > 0)
    }
}
