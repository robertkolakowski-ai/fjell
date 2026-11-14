//! Lagring for V1-grenen `protect` (store, L1). Eier alle SQL-spørringer for tabellen `protection`.
//!
//! Avhenger av: `super` (`Store`), `crate::model`. Filen eies av grenen `protect` (docs/v1/V1-PLAN.md);
//! ingen andre grener rører den. Skjemaet er ferdig i migrering 2 (`002_v1.sql`) og endres ikke av grenen.
//! Grensesnitt:
//! - `Store::protection_count`, `protection_add` (idempotent), `protection_restore` (angre av fjerning,
//!   bevarer opprinnelig tidspunkt), `protection_remove`, `protection_list`, `protection_target_exists`.
//! - `Store::protected_by_scope(ids)`: effektiv beskyttelse for en mengde bilder, ett kall via `json_each`
//!   (ingen variabelgrense). Dekker enkeltbilde, mappe (prefiks på `source_id`, på mappegrense),
//!   album og person (inkludert personer slått sammen inn i den beskyttede).
//!
//! Å fjerne en beskyttelse er konfigurasjon, ikke sletting av bilder, og kan angres (`protection_restore`).

use super::{Store, StoreError};
use crate::model::{AssetId, ProtectScope};
use rusqlite::params;

/// Effektiv beskyttelse: (bilde, omfangskode, mål) for hvert omfang som dekker bildet.
const EFFECTIVE_SQL: &str = "\
WITH RECURSIVE tree(root, id) AS ( \
    SELECT target, CAST(target AS INTEGER) FROM protection WHERE scope = 'person' \
    UNION \
    SELECT tree.root, p.id FROM person p JOIN tree ON p.merged_into = tree.id \
) \
SELECT j.value, 'asset', p.target FROM json_each(?1) j \
    JOIN protection p ON p.scope = 'asset' AND p.target = CAST(j.value AS TEXT) \
UNION ALL \
SELECT j.value, 'album', p.target FROM json_each(?1) j \
    JOIN asset_album aa ON aa.asset_id = j.value \
    JOIN protection p ON p.scope = 'album' AND p.target = CAST(aa.album_id AS TEXT) \
UNION ALL \
SELECT j.value, 'folder', p.target FROM json_each(?1) j \
    JOIN asset a ON a.id = j.value \
    JOIN protection p ON p.scope = 'folder' AND p.target <> '' AND ( \
        a.source_id = p.target OR ( \
            substr(a.source_id, 1, length(p.target)) = p.target AND ( \
                substr(p.target, -1) IN ('/', char(92)) \
                OR substr(a.source_id, length(p.target) + 1, 1) IN ('/', char(92)) \
            ))) \
UNION ALL \
SELECT j.value, 'person', tree.root FROM json_each(?1) j \
    JOIN face_embedding f ON f.asset_id = j.value \
    JOIN tree ON tree.id = f.person_id";

fn ids_json(ids: &[AssetId]) -> String {
    let mut s = String::with_capacity(ids.len() * 8 + 2);
    s.push('[');
    for (i, id) in ids.iter().enumerate() {
        if i > 0 {
            s.push(',');
        }
        s.push_str(&id.0.to_string());
    }
    s.push(']');
    s
}

impl Store {
    /// Antall rader i `protection`.
    pub fn protection_count(&self) -> Result<u64, StoreError> {
        let n: i64 = self
            .conn
            .query_row("SELECT count(*) FROM protection", [], |r| r.get(0))?;
        Ok(n as u64)
    }

    /// Legger til en beskyttelse. `true` hvis den er ny; finnes den fra før, endres ingenting.
    pub fn protection_add(&self, scope: &ProtectScope, now: i64) -> Result<bool, StoreError> {
        self.protection_restore(scope, now)
    }

    /// Setter inn raden med gitt tidspunkt (angre av `protection_remove`). `true` hvis ny.
    pub fn protection_restore(
        &self,
        scope: &ProtectScope,
        created_at: i64,
    ) -> Result<bool, StoreError> {
        let n = self.conn.execute(
            "INSERT OR IGNORE INTO protection(scope, target, created_at) VALUES (?1, ?2, ?3)",
            params![scope.scope_code(), scope.target(), created_at],
        )?;
        Ok(n > 0)
    }

    /// Fjerner beskyttelsen. Gir (omfang, opprettet) for raden som ble fjernet (for angre), eller `None` hvis den ikke fantes.
    pub fn protection_remove(
        &self,
        scope: &ProtectScope,
    ) -> Result<Option<(ProtectScope, i64)>, StoreError> {
        let created: Option<i64> = match self.conn.query_row(
            "DELETE FROM protection WHERE scope = ?1 AND target = ?2 RETURNING created_at",
            params![scope.scope_code(), scope.target()],
            |r| r.get(0),
        ) {
            Ok(v) => Some(v),
            Err(rusqlite::Error::QueryReturnedNoRows) => None,
            Err(e) => return Err(e.into()),
        };
        Ok(created.map(|created_at| (scope.clone(), created_at)))
    }

    /// Alle beskyttelser som (omfang, opprettet), eldste først. Rader med ukjent kode hoppes over (fremtidig skjema).
    pub fn protection_list(&self) -> Result<Vec<(ProtectScope, i64)>, StoreError> {
        let mut st = self.conn.prepare_cached(
            "SELECT scope, target, created_at FROM protection ORDER BY created_at, scope, target",
        )?;
        let rows = st.query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, i64>(2)?,
            ))
        })?;
        let mut out = Vec::new();
        for row in rows {
            let (scope, target, created_at) = row?;
            if let Some(scope) = ProtectScope::from_parts(&scope, &target) {
                out.push((scope, created_at));
            }
        }
        Ok(out)
    }

    /// Finnes målet (bilde, album eller person)? Mapper er fritekst og regnes alltid som funnet.
    pub fn protection_target_exists(&self, scope: &ProtectScope) -> Result<bool, StoreError> {
        let (sql, id) = match scope {
            ProtectScope::Folder(_) => return Ok(true),
            ProtectScope::Asset(a) => ("SELECT 1 FROM asset WHERE id = ?1", a.0),
            ProtectScope::Album(a) => ("SELECT 1 FROM album WHERE id = ?1", *a),
            ProtectScope::Person(p) => ("SELECT 1 FROM person WHERE id = ?1", p.0),
        };
        let mut st = self.conn.prepare_cached(sql)?;
        Ok(st.exists([id as i64])?)
    }

    /// Effektiv beskyttelse for `ids`: (bilde, omfang som beskytter), sortert på bilde og omfang.
    /// Et bilde kan stå med flere omfang. Bilder uten beskyttelse er ikke med.
    pub fn protected_by_scope(
        &self,
        ids: &[AssetId],
    ) -> Result<Vec<(AssetId, ProtectScope)>, StoreError> {
        if ids.is_empty() || self.protection_count()? == 0 {
            return Ok(Vec::new());
        }
        let mut st = self.conn.prepare_cached(EFFECTIVE_SQL)?;
        let rows = st.query_map([ids_json(ids)], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
            ))
        })?;
        let mut out = Vec::new();
        for row in rows {
            let (id, scope, target) = row?;
            if let Some(s) = ProtectScope::from_parts(&scope, &target) {
                out.push((AssetId(id as u64), s));
            }
        }
        out.sort();
        out.dedup();
        Ok(out)
    }
}
