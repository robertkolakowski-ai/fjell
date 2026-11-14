//! Lagring for V1-grenen `people` (store, L1). Eier alle SQL-spørringer for `person` og `face_embedding`.
//!
//! Avhenger av: `super` (`Store`), `crate::model`. Filen eies av grenen `people` (docs/v1/V1-PLAN.md);
//! ingen andre grener rører den. Skjemaet er ferdig i migrering 2 (`002_v1.sql`) og endres ikke av grenen.
//! Grensesnitt:
//! - `face_index_enabled`/`set_face_index_enabled`: valgfri ansiktsindeks (standard av, `setting.face_index_enabled`).
//! - `put_faces`, `faces_all`, `face_assign`, `asset_has_faces`: ansikter per bilde (embedding kun lokal).
//! - `person_new`, `person_root`, `person_name_set`, `people_rows`, `person_assets`: personer.
//! - `person_merge`/`person_unmerge`/`person_split`: alt angrebart uten at rader fjernes (`merged_into`;
//!   en splitt angres ved å slå den nye personen tilbake i den gamle).
//! - `face_index_wipe`: eksplisitt personvernvalg (avledet data), den eneste stien som fjerner rader.
//!
//! Beskyttelse av en person (`ProtectScope::Person`) eies av `protect`; her leses bare `protection` for visning.
//! Bare aktive bilder (`asset.state = 0`) telles i visningene.

use super::{Store, StoreError};
use crate::model::{AssetId, Embedding, FaceObservation, PersonId};
use rusqlite::{params, OptionalExtension};
use std::collections::HashMap;

/// Ett lagret ansikt.
#[derive(Clone, Debug, PartialEq)]
pub struct FaceRow {
    pub id: u64,
    pub asset: AssetId,
    pub person: Option<PersonId>,
    pub quality: f32,
    pub embedding: Embedding,
}

/// En «rot»-person (ikke slått sammen i en annen) slik visningen trenger den.
#[derive(Clone, Debug, PartialEq)]
pub struct PersonRow {
    pub id: PersonId,
    pub name: Option<String>,
    pub face_count: u64,
    pub asset_count: u64,
    /// Bilde med det beste ansiktet (høyest kvalitet, laveste id ved likhet).
    pub cover: Option<AssetId>,
    pub protected: bool,
}

/// Rot-oppslag: hver person til sin rot (følger `merged_into`).
const ROOTS_CTE: &str = "WITH RECURSIVE r(id, root) AS (\
    SELECT id, id FROM person WHERE merged_into IS NULL \
    UNION ALL SELECT p.id, r.root FROM person p JOIN r ON p.merged_into = r.id)";

impl Store {
    /// Antall rader i `face_embedding`.
    pub fn face_count(&self) -> Result<u64, StoreError> {
        let n: i64 = self
            .conn
            .query_row("SELECT count(*) FROM face_embedding", [], |r| r.get(0))?;
        Ok(n as u64)
    }

    /// Er ansiktsindeksen slått på? Manglende eller ukjent verdi betyr av.
    pub fn face_index_enabled(&self) -> Result<bool, StoreError> {
        Ok(self.setting("face_index_enabled")?.as_deref() == Some("1"))
    }

    pub fn set_face_index_enabled(&mut self, on: bool) -> Result<(), StoreError> {
        self.set_setting("face_index_enabled", if on { "1" } else { "0" })
    }

    /// Finnes det ansikter for bildet analysert med `algo` eller nyere?
    pub fn asset_has_faces(&self, asset: AssetId, algo: u32) -> Result<bool, StoreError> {
        let n: i64 = self.conn.query_row(
            "SELECT count(*) FROM face_embedding WHERE asset_id = ?1 AND algo_ver >= ?2",
            params![asset.0 as i64, algo],
            |r| r.get(0),
        )?;
        Ok(n > 0)
    }

    /// Lagrer ansikter for ett bilde (uten person ennå) i én transaksjon. Returnerer ansikts-id-ene.
    pub fn put_faces(
        &mut self,
        asset: AssetId,
        faces: &[FaceObservation],
        algo: u32,
    ) -> Result<Vec<u64>, StoreError> {
        let tx = self.conn.transaction()?;
        let mut ids = Vec::with_capacity(faces.len());
        for f in faces {
            tx.prepare_cached(
                "INSERT INTO face_embedding(asset_id, box_x, box_y, box_w, box_h, quality, embedding, algo_ver) \
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8)",
            )?
            .execute(params![
                asset.0 as i64,
                f.bbox.x,
                f.bbox.y,
                f.bbox.w,
                f.bbox.h,
                f.quality as f64,
                f.embedding.to_le_bytes(),
                algo
            ])?;
            ids.push(tx.last_insert_rowid() as u64);
        }
        tx.commit()?;
        Ok(ids)
    }

    /// Alle ansikter på aktive bilder, i id-rekkefølge. Rader med ugyldig embedding hoppes over.
    pub fn faces_all(&self) -> Result<Vec<FaceRow>, StoreError> {
        let mut st = self.conn.prepare_cached(
            "SELECT f.id, f.asset_id, f.person_id, f.quality, f.embedding FROM face_embedding f \
             JOIN asset a ON a.id = f.asset_id WHERE a.state = 0 ORDER BY f.id",
        )?;
        let rows = st.query_map([], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, i64>(1)?,
                r.get::<_, Option<i64>>(2)?,
                r.get::<_, f64>(3)?,
                r.get::<_, Vec<u8>>(4)?,
            ))
        })?;
        let mut out = Vec::new();
        for row in rows {
            let (id, asset, person, quality, blob) = row?;
            if let Some(embedding) = Embedding::from_le_bytes(&blob) {
                out.push(FaceRow {
                    id: id as u64,
                    asset: AssetId(asset as u64),
                    person: person.map(|p| PersonId(p as u64)),
                    quality: quality as f32,
                    embedding,
                });
            }
        }
        Ok(out)
    }

    /// Ny selvstendig person uten navn.
    pub fn person_new(&mut self, now: i64) -> Result<PersonId, StoreError> {
        self.conn
            .execute("INSERT INTO person(created_at) VALUES (?1)", [now])?;
        Ok(PersonId(self.conn.last_insert_rowid() as u64))
    }

    /// Tildeler ansiktet en person. Returnerer `false` hvis ansiktet eller personen ikke finnes.
    pub fn face_assign(&mut self, face: u64, person: PersonId) -> Result<bool, StoreError> {
        if !self.person_exists(person)? {
            return Ok(false);
        }
        let n = self.conn.execute(
            "UPDATE face_embedding SET person_id = ?2 WHERE id = ?1",
            params![face as i64, person.0 as i64],
        )?;
        Ok(n > 0)
    }

    fn person_exists(&self, id: PersonId) -> Result<bool, StoreError> {
        Ok(self
            .conn
            .query_row("SELECT 1 FROM person WHERE id = ?1", [id.0 as i64], |_| {
                Ok(())
            })
            .optional()?
            .is_some())
    }

    /// Roten til en person (følger `merged_into`). `None` hvis personen ikke finnes.
    pub fn person_root(&self, id: PersonId) -> Result<Option<PersonId>, StoreError> {
        let mut cur = id.0 as i64;
        // Kjeden er aldri syklisk (sammenslåing går bare mellom ulike røtter); grensen er et sikkerhetsnett.
        for _ in 0..10_000 {
            let next: Option<Option<i64>> = self
                .conn
                .query_row("SELECT merged_into FROM person WHERE id = ?1", [cur], |r| {
                    r.get(0)
                })
                .optional()?;
            match next {
                None => return Ok(None),
                Some(None) => return Ok(Some(PersonId(cur as u64))),
                Some(Some(n)) => cur = n,
            }
        }
        Ok(None)
    }

    /// Setter eller fjerner navnet (tomt navn etter trim fjerner det). Returnerer `false` hvis personen ikke finnes.
    pub fn person_name_set(
        &mut self,
        id: PersonId,
        name: Option<&str>,
    ) -> Result<bool, StoreError> {
        let name = name.map(str::trim).filter(|n| !n.is_empty());
        let n = self.conn.execute(
            "UPDATE person SET name = ?2 WHERE id = ?1",
            params![id.0 as i64, name],
        )?;
        Ok(n > 0)
    }

    /// Slår `absorbed` inn i `target` (begge løses til sine røtter). Ingen rader fjernes; angres med `person_unmerge`.
    /// Returnerer rotpersonen som ble stående, eller `None` (ingenting skrevet) hvis en av dem ikke finnes
    /// eller begge allerede er samme person.
    pub fn person_merge(
        &mut self,
        target: PersonId,
        absorbed: PersonId,
    ) -> Result<Option<PersonId>, StoreError> {
        let (Some(t), Some(a)) = (self.person_root(target)?, self.person_root(absorbed)?) else {
            return Ok(None);
        };
        if t == a {
            return Ok(None);
        }
        self.conn.execute(
            "UPDATE person SET merged_into = ?2 WHERE id = ?1",
            params![a.0 as i64, t.0 as i64],
        )?;
        Ok(Some(t))
    }

    /// Opphever sammenslåingen av `absorbed` (personen blir selvstendig igjen med sine egne ansikter).
    /// Returnerer `false` hvis personen ikke var slått sammen.
    pub fn person_unmerge(&mut self, absorbed: PersonId) -> Result<bool, StoreError> {
        let n = self.conn.execute(
            "UPDATE person SET merged_into = NULL WHERE id = ?1 AND merged_into IS NOT NULL",
            [absorbed.0 as i64],
        )?;
        Ok(n > 0)
    }

    /// Flytter de oppgitte ansiktene ut av `person` (og personer slått inn i den) til en ny person.
    /// `None` hvis listen er tom, personen ukjent eller et ansikt ikke tilhører personen; da skrives ingenting.
    /// Angres ved å slå den nye personen inn i den gamle (`person_merge`); ingen rader fjernes.
    pub fn person_split(
        &mut self,
        person: PersonId,
        faces: &[u64],
        now: i64,
    ) -> Result<Option<PersonId>, StoreError> {
        let Some(root) = self.person_root(person)? else {
            return Ok(None);
        };
        if faces.is_empty() {
            return Ok(None);
        }
        let tx = self.conn.transaction()?;
        for f in faces {
            let ok: Option<i64> = tx
                .query_row(
                    &format!(
                        "{ROOTS_CTE} SELECT 1 FROM face_embedding f JOIN r ON r.id = f.person_id \
                         WHERE f.id = ?1 AND r.root = ?2"
                    ),
                    params![*f as i64, root.0 as i64],
                    |r| r.get(0),
                )
                .optional()?;
            if ok.is_none() {
                return Ok(None);
            }
        }
        tx.execute("INSERT INTO person(created_at) VALUES (?1)", [now])?;
        let new = tx.last_insert_rowid();
        for f in faces {
            tx.execute(
                "UPDATE face_embedding SET person_id = ?2 WHERE id = ?1",
                params![*f as i64, new],
            )?;
        }
        tx.commit()?;
        Ok(Some(PersonId(new as u64)))
    }

    /// Rot-personer med antall ansikter og bilder, største først (deretter id).
    pub fn people_rows(&self) -> Result<Vec<PersonRow>, StoreError> {
        let mut persons: Vec<PersonRow> = Vec::new();
        {
            let mut st = self.conn.prepare(
                "SELECT p.id, p.name, \
                   EXISTS(SELECT 1 FROM protection WHERE scope = 'person' AND target = CAST(p.id AS TEXT)) \
                 FROM person p WHERE p.merged_into IS NULL ORDER BY p.id",
            )?;
            let rows = st.query_map([], |r| {
                Ok(PersonRow {
                    id: PersonId(r.get::<_, i64>(0)? as u64),
                    name: r.get(1)?,
                    face_count: 0,
                    asset_count: 0,
                    cover: None,
                    protected: r.get::<_, i64>(2)? != 0,
                })
            })?;
            for r in rows {
                persons.push(r?);
            }
        }
        let index: HashMap<u64, usize> = persons
            .iter()
            .enumerate()
            .map(|(i, p)| (p.id.0, i))
            .collect();
        let mut best: HashMap<u64, (f64, i64)> = HashMap::new();
        let mut assets: HashMap<u64, std::collections::HashSet<i64>> = HashMap::new();
        {
            let mut st = self.conn.prepare(&format!(
                "{ROOTS_CTE} SELECT r.root, f.asset_id, f.quality FROM face_embedding f \
                 JOIN r ON r.id = f.person_id JOIN asset a ON a.id = f.asset_id WHERE a.state = 0 \
                 ORDER BY f.id"
            ))?;
            let rows = st.query_map([], |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, i64>(1)?,
                    r.get::<_, f64>(2)?,
                ))
            })?;
            for r in rows {
                let (root, asset, q) = r?;
                let root = root as u64;
                let Some(&i) = index.get(&root) else { continue };
                persons[i].face_count += 1;
                assets.entry(root).or_default().insert(asset);
                let e = best.entry(root).or_insert((q, asset));
                if q > e.0 || (q == e.0 && asset < e.1) {
                    *e = (q, asset);
                }
            }
        }
        for p in &mut persons {
            p.asset_count = assets.get(&p.id.0).map_or(0, |s| s.len() as u64);
            p.cover = best.get(&p.id.0).map(|b| AssetId(b.1 as u64));
        }
        persons.sort_by(|a, b| b.face_count.cmp(&a.face_count).then(a.id.0.cmp(&b.id.0)));
        Ok(persons)
    }

    /// Aktive bilder med personen (inkludert personer slått inn i den), sortert. Til beskyttelse og visning.
    pub fn person_assets(&self, person: PersonId) -> Result<Vec<AssetId>, StoreError> {
        let Some(root) = self.person_root(person)? else {
            return Ok(Vec::new());
        };
        let mut st = self.conn.prepare(&format!(
            "{ROOTS_CTE} SELECT DISTINCT f.asset_id FROM face_embedding f JOIN r ON r.id = f.person_id \
             JOIN asset a ON a.id = f.asset_id WHERE r.root = ?1 AND a.state = 0 ORDER BY f.asset_id"
        ))?;
        let rows = st.query_map([root.0 as i64], |r| r.get::<_, i64>(0))?;
        let mut out = Vec::new();
        for r in rows {
            out.push(AssetId(r? as u64));
        }
        Ok(out)
    }

    /// «Slett ansiktsindeks»: fjerner alle embeddings og personer i én transaksjon. Dette er avledet data som
    /// kan gjenskapes, og stien er ikke angrebar (eksplisitt personvernvalg, med varsel i UI). Bilder, beskyttelser
    /// og alt annet røres ikke; beskyttelser av typen `person` blir hengende. Returnerer antall ansikter slettet.
    pub fn face_index_wipe(&mut self) -> Result<u64, StoreError> {
        let tx = self.conn.transaction()?;
        let n = tx.execute("DELETE FROM face_embedding", [])?;
        tx.execute("UPDATE person SET merged_into = NULL", [])?;
        tx.execute("DELETE FROM person", [])?;
        tx.commit()?;
        Ok(n as u64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::FaceBox;

    fn store_with_assets(n: u64) -> Store {
        let s = Store::open_in_memory().unwrap();
        for i in 1..=n {
            s.conn
                .execute(
                    "INSERT INTO asset(id, source_id, kind, created_at, mod_token, width, height, bytes, mime) \
                     VALUES (?1, ?2, 'photo', 1, 'm', 4, 4, 3, 'image/jpeg')",
                    params![i as i64, format!("s{i}")],
                )
                .unwrap();
        }
        s
    }

    fn face(q: f32) -> FaceObservation {
        FaceObservation {
            bbox: FaceBox {
                x: 1,
                y: 2,
                w: 3,
                h: 4,
            },
            quality: q,
            embedding: Embedding(vec![1.0, 0.0]),
        }
    }

    #[test]
    fn index_is_off_by_default_and_toggles() {
        let mut s = Store::open_in_memory().unwrap();
        assert!(!s.face_index_enabled().unwrap());
        s.set_face_index_enabled(true).unwrap();
        assert!(s.face_index_enabled().unwrap());
        s.set_face_index_enabled(false).unwrap();
        assert!(!s.face_index_enabled().unwrap());
    }

    #[test]
    fn put_and_read_faces_roundtrip() {
        let mut s = store_with_assets(2);
        let ids = s.put_faces(AssetId(1), &[face(0.5), face(0.9)], 1).unwrap();
        assert_eq!(ids.len(), 2);
        assert!(s.asset_has_faces(AssetId(1), 1).unwrap());
        assert!(!s.asset_has_faces(AssetId(1), 2).unwrap());
        assert!(!s.asset_has_faces(AssetId(2), 0).unwrap());
        let all = s.faces_all().unwrap();
        assert_eq!(all.len(), 2);
        assert_eq!(all[1].embedding, Embedding(vec![1.0, 0.0]));
        assert_eq!(all[0].person, None);
    }

    #[test]
    fn merge_unmerge_is_reversible_and_resolves_roots() {
        let mut s = store_with_assets(3);
        let a = s.person_new(1).unwrap();
        let b = s.person_new(1).unwrap();
        let c = s.person_new(1).unwrap();
        for (asset, p) in [(1, a), (2, b), (3, c)] {
            let f = s.put_faces(AssetId(asset), &[face(0.5)], 1).unwrap()[0];
            assert!(s.face_assign(f, p).unwrap());
        }
        assert_eq!(s.person_merge(a, b).unwrap(), Some(a));
        assert_eq!(s.person_merge(c, b).unwrap(), Some(c));
        // b er løst til a, så a slås inn i c
        assert_eq!(s.person_root(b).unwrap(), Some(c));
        assert_eq!(s.person_merge(a, c).unwrap(), None);
        assert_eq!(s.person_merge(a, PersonId(99)).unwrap(), None);
        let rows = s.people_rows().unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(
            (rows[0].id, rows[0].face_count, rows[0].asset_count),
            (c, 3, 3)
        );
        // angre: a og b blir selvstendige igjen
        assert!(s.person_unmerge(a).unwrap());
        assert!(!s.person_unmerge(a).unwrap());
        let rows = s.people_rows().unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(s.person_root(a).unwrap(), Some(a));
        assert_eq!(s.person_count_raw(), 3, "ingen person fjernes");
    }

    #[test]
    fn split_moves_faces_and_undo_by_merge() {
        let mut s = store_with_assets(2);
        let p = s.person_new(1).unwrap();
        let f1 = s.put_faces(AssetId(1), &[face(0.5)], 1).unwrap()[0];
        let f2 = s.put_faces(AssetId(2), &[face(0.6)], 1).unwrap()[0];
        s.face_assign(f1, p).unwrap();
        s.face_assign(f2, p).unwrap();
        let q = s.person_split(p, &[f2], 5).unwrap().unwrap();
        assert_ne!(p, q);
        assert_eq!(s.person_assets(p).unwrap(), vec![AssetId(1)]);
        assert_eq!(s.person_assets(q).unwrap(), vec![AssetId(2)]);
        // angre
        assert_eq!(s.person_merge(p, q).unwrap(), Some(p));
        assert_eq!(s.person_assets(p).unwrap(), vec![AssetId(1), AssetId(2)]);
        // fremmed eller tomt ansikt: ingenting skrives
        let r = s.put_faces(AssetId(1), &[face(0.1)], 1).unwrap()[0];
        assert_eq!(s.person_split(p, &[r], 6).unwrap(), None);
        assert_eq!(s.person_split(p, &[], 6).unwrap(), None);
        assert_eq!(s.person_count_raw(), 2);
    }

    #[test]
    fn name_trims_and_clears() {
        let mut s = Store::open_in_memory().unwrap();
        let p = s.person_new(1).unwrap();
        assert!(s.person_name_set(p, Some("  Kari ")).unwrap());
        assert_eq!(s.people_rows().unwrap()[0].name.as_deref(), Some("Kari"));
        s.person_name_set(p, Some("   ")).unwrap();
        assert_eq!(s.people_rows().unwrap()[0].name, None);
        assert!(!s.person_name_set(PersonId(77), Some("x")).unwrap());
    }

    #[test]
    fn view_counts_only_active_assets_and_picks_best_cover() {
        let mut s = store_with_assets(3);
        let p = s.person_new(1).unwrap();
        for (a, q) in [(1u64, 0.4f32), (2, 0.9), (3, 0.99)] {
            let f = s.put_faces(AssetId(a), &[face(q)], 1).unwrap()[0];
            s.face_assign(f, p).unwrap();
        }
        s.conn
            .execute("UPDATE asset SET state = 1 WHERE id = 3", [])
            .unwrap();
        let r = &s.people_rows().unwrap()[0];
        assert_eq!(
            (r.face_count, r.asset_count, r.cover),
            (2, 2, Some(AssetId(2)))
        );
        assert!(!r.protected);
        s.conn
            .execute(
                "INSERT INTO protection(scope, target, created_at) VALUES ('person', ?1, 1)",
                [p.0.to_string()],
            )
            .unwrap();
        assert!(s.people_rows().unwrap()[0].protected);
    }

    #[test]
    fn wipe_removes_only_derived_face_data() {
        let mut s = store_with_assets(2);
        let a = s.person_new(1).unwrap();
        let b = s.person_new(1).unwrap();
        let f = s.put_faces(AssetId(1), &[face(0.5)], 1).unwrap()[0];
        s.face_assign(f, a).unwrap();
        s.person_merge(a, b).unwrap();
        s.conn
            .execute(
                "INSERT INTO protection(scope, target, created_at) VALUES ('person', ?1, 1)",
                [a.0.to_string()],
            )
            .unwrap();
        assert_eq!(s.face_index_wipe().unwrap(), 1);
        assert_eq!(s.face_count().unwrap(), 0);
        assert_eq!(s.person_count_raw(), 0);
        assert!(s.people_rows().unwrap().is_empty());
        assert_eq!(s.protection_row_count(), 1, "beskyttelsen blir hengende");
        let assets: i64 = s
            .conn
            .query_row("SELECT count(*) FROM asset WHERE state = 0", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(assets, 2, "bilder røres ikke");
        assert_eq!(s.face_index_wipe().unwrap(), 0);
    }

    impl Store {
        fn person_count_raw(&self) -> i64 {
            self.conn
                .query_row("SELECT count(*) FROM person", [], |r| r.get(0))
                .unwrap()
        }
        fn protection_row_count(&self) -> i64 {
            self.conn
                .query_row("SELECT count(*) FROM protection", [], |r| r.get(0))
                .unwrap()
        }
    }
}
