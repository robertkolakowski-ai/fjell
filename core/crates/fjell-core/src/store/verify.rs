//! Selvkontroll av indeksen etter krasj (K7-6, PLAN-KJERNE) og i diagnostikk. Del av `store`.
//!
//! Grensesnitt: `Store::verify() -> Vec<String>` (tom = alt i orden) og `Store::batch_sizes()`.
//! Sjekker SQLite-integritet, fremmednøkler og at hver bildes `state` stemmer med siste
//! tilstandsskifte i `action_log` (halv batch ville gitt avvik). Bare lesing.

use super::{Store, StoreError};

impl Store {
    /// Liste over avvik; tom liste betyr at indeksen er konsistent.
    pub fn verify(&self) -> Result<Vec<String>, StoreError> {
        let mut problems = Vec::new();
        let ic: Vec<String> = {
            let mut st = self.conn.prepare("PRAGMA integrity_check")?;
            let rows = st.query_map([], |r| r.get::<_, String>(0))?;
            rows.collect::<Result<_, _>>()?
        };
        if ic != ["ok"] {
            problems.push(format!("integrity_check: {}", ic.join("; ")));
        }
        let fk: i64 =
            self.conn
                .query_row("SELECT COUNT(*) FROM pragma_foreign_key_check", [], |r| {
                    r.get(0)
                })?;
        if fk > 0 {
            problems.push(format!("foreign_key_check: {fk} brudd"));
        }
        // Tilstand mot siste tilstandsskifte i loggen (keep endrer ikke tilstand). Bilder uten
        // tilstandsrad i loggen skal være aktive.
        let mismatched: Vec<i64> = {
            let mut st = self.conn.prepare(
                "SELECT a.id FROM asset a WHERE a.state != COALESCE((SELECT CASE l.op \
                   WHEN 'trash' THEN 1 WHEN 'restore' THEN 0 ELSE 2 END FROM action_log l \
                   WHERE l.asset_id = a.id AND l.op != 'keep' ORDER BY l.id DESC LIMIT 1), 0) \
                 ORDER BY a.id LIMIT 20",
            )?;
            let rows = st.query_map([], |r| r.get::<_, i64>(0))?;
            rows.collect::<Result<_, _>>()?
        };
        if !mismatched.is_empty() {
            problems.push(format!(
                "tilstand stemmer ikke med action_log for bilder {mismatched:?}"
            ));
        }
        Ok(problems)
    }

    /// (batch_id, antall rader) i `action_log`, sortert på batch. En batch skal enten finnes helt eller ikke.
    pub fn batch_sizes(&self) -> Result<Vec<(i64, u64)>, StoreError> {
        let mut st = self.conn.prepare(
            "SELECT batch_id, COUNT(*) FROM action_log GROUP BY batch_id ORDER BY batch_id",
        )?;
        let rows = st.query_map([], |r| {
            Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)? as u64))
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    }
}
