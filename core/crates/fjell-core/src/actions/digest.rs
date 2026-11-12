//! Digest og øyeblikksbilde av en mengde (I12). Del av `actions`.
//!
//! Grensesnitt: `Snapshot` (rader + digest + bytes) fra `Actions::snapshot(ids)`.
//! Digesten (FNV-1a 64) dekker (id, tilstand, beskyttelsesflagg) sortert på id. Den er en
//! endringsdetektor, ikke kryptografi: bekreftelsestyper kan uansett bare lages av kjernen.

use super::{ActionError, Actions, Result};
use crate::model::AssetId;
use crate::store::AssetRow;

pub(crate) struct Snapshot {
    pub rows: Vec<AssetRow>,
    pub digest: u64,
    pub bytes: u64,
}

pub(crate) fn digest_rows(rows: &[AssetRow]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    let mut eat = |v: u64| {
        for b in v.to_le_bytes() {
            h ^= b as u64;
            h = h.wrapping_mul(0x0000_0100_0000_01b3);
        }
    };
    eat(rows.len() as u64);
    for r in rows {
        eat(r.id.0);
        eat(r.state.as_i64() as u64);
        eat(r.protect.0 as u64);
    }
    h
}

impl Actions {
    /// Leser radene for `ids` (må være sortert og unike). Ukjent id gir `UnknownAsset`.
    pub(crate) fn snapshot(&self, ids: &[AssetId]) -> Result<Snapshot> {
        let mut rows = Vec::with_capacity(ids.len());
        for (row, &id) in self.store.asset_rows(ids)?.into_iter().zip(ids) {
            rows.push(row.ok_or(ActionError::UnknownAsset(id))?);
        }
        let bytes = rows.iter().map(|r| r.bytes).sum();
        Ok(Snapshot {
            digest: digest_rows(&rows),
            rows,
            bytes,
        })
    }
}
