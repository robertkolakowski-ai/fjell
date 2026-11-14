//! Lagring for V1-grenen `series` (store, L1). Eier alle SQL-spørringer for serier (`grp` med kind 'series') og `eye_result`.
//!
//! Avhenger av: `super` (`Store`), `crate::model`. Filen eies av grenen `series` (docs/v1/V1-PLAN.md);
//! ingen andre grener rører den. Skjemaet er ferdig i migrering 5 (`005_v1.sql`) og endres ikke av grenen.
//! Grensesnitt:
//! - `eye_result_count`, `put_eye_results`, `eye_results`, `eye_missing`: øyeanalyse per bilde. Ingen rad =
//!   ikke analysert (aldri «ingen lukkede øyne»). `algo_ver` gjør at nye modeller analyserer på nytt.
//! - `apply_series(&[SeriesSpec], ts) -> SeriesStats`: alt i én transaksjon. En serie med samme aktive
//!   medlemmer som en lagret gjenbrukes (id og keeper beholdes, så et manuelt keeper-valg overlever ny
//!   gruppering). Andre serier byttes ut. Bare ventende forslag av typen `series` endres; `accepted`/`rejected`
//!   røres aldri, og keeper får aldri forslag. Andre grupper og forslagstyper røres ikke.
//! - `set_series_keeper(series, keeper)`: bytter keeper (angres ved å velge forrige keeper), flytter det ventende forslaget.
//! - `series_members(series)`: medlemmer med rang, score, størrelse og beskyttelsesflagg til side-ved-side-visning.
//! - `series_count`: antall serier med minst to aktive medlemmer.

use super::{Store, StoreError};
use crate::model::{AssetId, EyeReport, ProtectFlags, ReasonCode, SeriesId};
use rusqlite::{params, OptionalExtension};
use std::collections::{BTreeMap, HashSet};

/// Ett medlem i en serie som skal lagres. `reason`/`confidence` gjelder forslaget når medlemmet ikke er keeper.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SeriesMemberSpec {
    pub asset: AssetId,
    pub score: f32,
    pub reason: ReasonCode,
    pub confidence: f32,
}

/// En serie som skal lagres. `members` er beste først (rangen er posisjonen); `keeper` er et av dem.
#[derive(Clone, Debug, PartialEq)]
pub struct SeriesSpec {
    pub members: Vec<SeriesMemberSpec>,
    pub keeper: AssetId,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SeriesStats {
    pub groups: u64,
    pub kept_groups: u64,
    pub suggestions: u64,
    pub removed_suggestions: u64,
}

/// Medlem slik side-ved-side-visningen trenger det.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SeriesMemberRow {
    pub asset: AssetId,
    /// 1-basert.
    pub rank: u32,
    pub score: f32,
    pub is_keeper: bool,
    pub bytes: u64,
    pub protect: ProtectFlags,
}

impl Store {
    /// Antall rader i `eye_result`.
    pub fn eye_result_count(&self) -> Result<u64, StoreError> {
        let n: i64 = self
            .conn
            .query_row("SELECT count(*) FROM eye_result", [], |r| r.get(0))?;
        Ok(n as u64)
    }

    /// Lagrer øyeanalyser (erstatter eksisterende rad per bilde) i én transaksjon.
    pub fn put_eye_results(
        &mut self,
        rows: &[(AssetId, EyeReport)],
        algo: u32,
    ) -> Result<(), StoreError> {
        let tx = self.conn.transaction()?;
        for (id, r) in rows {
            tx.prepare_cached(
                "INSERT INTO eye_result(asset_id, faces, closed, confidence, algo_ver) VALUES (?1,?2,?3,?4,?5) \
                 ON CONFLICT(asset_id) DO UPDATE SET faces=excluded.faces, closed=excluded.closed, \
                 confidence=excluded.confidence, algo_ver=excluded.algo_ver",
            )?
            .execute(params![
                id.0 as i64,
                r.faces as i64,
                r.closed as i64,
                r.confidence as f64,
                algo as i64
            ])?;
        }
        tx.commit()?;
        Ok(())
    }

    /// Lagrede øyeanalyser for `ids` (bilder uten rad er ikke med).
    pub fn eye_results(&self, ids: &[AssetId]) -> Result<BTreeMap<AssetId, EyeReport>, StoreError> {
        let mut st = self.conn.prepare_cached(
            "SELECT faces, closed, confidence FROM eye_result WHERE asset_id = ?1",
        )?;
        let mut out = BTreeMap::new();
        for id in ids {
            let row = st
                .query_row([id.0 as i64], |r| {
                    Ok(EyeReport {
                        faces: r.get::<_, i64>(0)?.clamp(0, 255) as u8,
                        closed: r.get::<_, i64>(1)?.clamp(0, 255) as u8,
                        confidence: r.get::<_, f64>(2)? as f32,
                    })
                })
                .optional()?;
            if let Some(r) = row {
                out.insert(*id, r);
            }
        }
        Ok(out)
    }

    /// Aktive bilder av typen foto med fingeravtrykk som mangler øyeanalyse for `algo` (ikke analysert, eller
    /// analysert med en annen versjon), sortert på id.
    pub fn eye_missing(&self, algo: u32) -> Result<Vec<AssetId>, StoreError> {
        let mut st = self.conn.prepare_cached(
            "SELECT a.id FROM asset a JOIN fingerprint f ON f.asset_id = a.id \
             LEFT JOIN eye_result e ON e.asset_id = a.id \
             WHERE a.state = 0 AND a.kind = 'photo' AND (e.asset_id IS NULL OR e.algo_ver != ?1) ORDER BY a.id",
        )?;
        let rows = st.query_map([algo as i64], |r| Ok(AssetId(r.get::<_, i64>(0)? as u64)))?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// Lagrer seriene og synkroniserer ventende `series`-forslag. Se modulhodet.
    pub fn apply_series(
        &mut self,
        specs: &[SeriesSpec],
        ts: i64,
    ) -> Result<SeriesStats, StoreError> {
        let tx = self.conn.transaction()?;
        let mut stats = SeriesStats::default();
        // Lagrede serier: (aktive medlemmer sortert) -> (id, keeper).
        let mut existing: BTreeMap<Vec<i64>, (i64, Option<i64>)> = BTreeMap::new();
        let all_ids: Vec<i64> = {
            let mut st = tx.prepare_cached("SELECT id FROM grp WHERE kind = 'series'")?;
            let rows = st.query_map([], |r| r.get(0))?;
            rows.collect::<Result<_, _>>()?
        };
        for gid in &all_ids {
            let keeper: Option<i64> =
                tx.query_row("SELECT keeper_id FROM grp WHERE id = ?1", [gid], |r| {
                    r.get(0)
                })?;
            let mut st = tx.prepare_cached(
                "SELECT m.asset_id FROM grp_member m JOIN asset a ON a.id = m.asset_id \
                 WHERE m.grp_id = ?1 AND a.state = 0 ORDER BY m.asset_id",
            )?;
            let members: Vec<i64> = st
                .query_map([gid], |r| r.get(0))?
                .collect::<Result<_, _>>()?;
            existing.insert(members, (*gid, keeper));
        }
        let mut kept: HashSet<i64> = HashSet::new();
        // (asset, gruppe, reason, confidence) som skal finnes som ventende; første forekomst vinner.
        let mut desired: Vec<(i64, i64, ReasonCode, f32)> = Vec::new();
        let mut seen: HashSet<i64> = HashSet::new();
        for s in specs {
            if s.members.len() < 2 || !s.members.iter().any(|m| m.asset == s.keeper) {
                continue;
            }
            if s.members.iter().any(|m| seen.contains(&(m.asset.0 as i64))) {
                continue;
            }
            seen.extend(s.members.iter().map(|m| m.asset.0 as i64));
            let mut ids: Vec<i64> = s.members.iter().map(|m| m.asset.0 as i64).collect();
            ids.sort();
            let reuse = existing
                .get(&ids)
                .filter(|(_, k)| k.is_some_and(|k| ids.contains(&k)))
                .copied();
            let (gid, keeper) = match reuse {
                Some((id, Some(keeper))) => {
                    kept.insert(id);
                    stats.kept_groups += 1;
                    for (rank, m) in s.members.iter().enumerate() {
                        tx.prepare_cached(
                            "UPDATE grp_member SET rank = ?3, score = ?4 WHERE grp_id = ?1 AND asset_id = ?2",
                        )?
                        .execute(params![id, m.asset.0 as i64, rank as i64, m.score as f64])?;
                    }
                    (id, keeper)
                }
                _ => {
                    tx.prepare_cached(
                        "INSERT INTO grp(kind, keeper_id, created_at) VALUES ('series', ?1, ?2)",
                    )?
                    .execute(params![s.keeper.0 as i64, ts])?;
                    let id = tx.last_insert_rowid();
                    for (rank, m) in s.members.iter().enumerate() {
                        tx.prepare_cached(
                            "INSERT INTO grp_member(grp_id, asset_id, rank, score) VALUES (?1,?2,?3,?4)",
                        )?
                        .execute(params![id, m.asset.0 as i64, rank as i64, m.score as f64])?;
                    }
                    kept.insert(id);
                    (id, s.keeper.0 as i64)
                }
            };
            stats.groups += 1;
            for m in &s.members {
                let a = m.asset.0 as i64;
                if a == keeper {
                    continue;
                }
                // Den automatiske keeperen kan være degradert av et manuelt valg: da gjelder ikke keeper-merket.
                let reason = if m.asset == s.keeper {
                    ReasonCode::SeriesMember
                } else {
                    m.reason
                };
                desired.push((a, gid, reason, m.confidence));
            }
        }
        for id in all_ids.iter().filter(|id| !kept.contains(id)) {
            tx.prepare_cached("DELETE FROM grp WHERE id = ?1")?
                .execute([id])?;
        }
        // Synkroniser ventende series-forslag; andre statuser og forslagstyper røres ikke.
        let mut have: BTreeMap<i64, (i64, String)> = BTreeMap::new();
        {
            let mut st = tx.prepare_cached(
                "SELECT id, asset_id, status FROM suggestion WHERE kind = 'series'",
            )?;
            let rows = st.query_map([], |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, i64>(1)?,
                    r.get::<_, String>(2)?,
                ))
            })?;
            for row in rows {
                let (id, asset, status) = row?;
                have.insert(asset, (id, status));
            }
        }
        let want: HashSet<i64> = desired.iter().map(|d| d.0).collect();
        for (asset, (id, status)) in &have {
            if status == "pending" && !want.contains(asset) {
                tx.prepare_cached("DELETE FROM suggestion WHERE id = ?1")?
                    .execute([id])?;
                stats.removed_suggestions += 1;
            }
        }
        for (asset, gid, reason, conf) in &desired {
            match have.get(asset) {
                Some((_, status)) if status != "pending" => {}
                Some((id, _)) => {
                    tx.prepare_cached(
                        "UPDATE suggestion SET grp_id=?2, reason_code=?3, confidence=?4 WHERE id=?1",
                    )?
                    .execute(params![id, gid, reason.key(), *conf as f64])?;
                    stats.suggestions += 1;
                }
                None => {
                    tx.prepare_cached(
                        "INSERT INTO suggestion(asset_id, kind, grp_id, reason_code, confidence) \
                         VALUES (?1,'series',?2,?3,?4)",
                    )?
                    .execute(params![
                        asset,
                        gid,
                        reason.key(),
                        *conf as f64
                    ])?;
                    stats.suggestions += 1;
                }
            }
        }
        tx.commit()?;
        Ok(stats)
    }

    /// Bytter keeper i en serie. `false` hvis serien ikke finnes, ikke er en serie, eller `keeper` ikke er et
    /// aktivt medlem. Det ventende forslaget for ny keeper fjernes; forrige keeper får et ventende
    /// `series_member`-forslag (med samme konfidens) hvis den ikke allerede har et avgjort forslag.
    pub fn set_series_keeper(
        &mut self,
        series: SeriesId,
        keeper: AssetId,
    ) -> Result<bool, StoreError> {
        let tx = self.conn.transaction()?;
        let cur: Option<Option<i64>> = tx
            .prepare_cached("SELECT keeper_id FROM grp WHERE id = ?1 AND kind = 'series'")?
            .query_row([series.0 as i64], |r| r.get(0))
            .optional()?;
        let member: bool = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM grp_member m JOIN asset a ON a.id = m.asset_id \
             WHERE m.grp_id = ?1 AND m.asset_id = ?2 AND a.state = 0)",
            params![series.0 as i64, keeper.0 as i64],
            |r| r.get(0),
        )?;
        let Some(old) = cur.flatten() else {
            return Ok(false);
        };
        if !member {
            return Ok(false);
        }
        if old != keeper.0 as i64 {
            let conf: Option<f64> = tx
                .prepare_cached(
                    "SELECT confidence FROM suggestion WHERE asset_id = ?1 AND kind = 'series' \
                     AND grp_id = ?2 AND status = 'pending'",
                )?
                .query_row(params![keeper.0 as i64, series.0 as i64], |r| r.get(0))
                .optional()?;
            tx.execute(
                "UPDATE grp SET keeper_id = ?2 WHERE id = ?1",
                params![series.0 as i64, keeper.0 as i64],
            )?;
            tx.execute(
                "DELETE FROM suggestion WHERE asset_id = ?1 AND kind = 'series' AND grp_id = ?2 \
                 AND status = 'pending'",
                params![keeper.0 as i64, series.0 as i64],
            )?;
            let old_active: bool = tx.query_row(
                "SELECT EXISTS(SELECT 1 FROM asset WHERE id = ?1 AND state = 0)",
                [old],
                |r| r.get(0),
            )?;
            if old_active {
                tx.execute(
                    "INSERT INTO suggestion(asset_id, kind, grp_id, reason_code, confidence) \
                     VALUES (?1,'series',?2,?3,?4) ON CONFLICT(asset_id, kind) DO NOTHING",
                    params![
                        old,
                        series.0 as i64,
                        ReasonCode::SeriesMember.key(),
                        conf.unwrap_or(0.5)
                    ],
                )?;
            }
        }
        tx.commit()?;
        Ok(true)
    }

    /// Aktive medlemmer av serien, beste først. Tom hvis serien ikke finnes.
    pub fn series_members(&self, series: SeriesId) -> Result<Vec<SeriesMemberRow>, StoreError> {
        let mut st = self.conn.prepare_cached(
            "SELECT m.asset_id, m.score, a.bytes, a.protect_flags, g.keeper_id FROM grp g \
             JOIN grp_member m ON m.grp_id = g.id JOIN asset a ON a.id = m.asset_id \
             WHERE g.id = ?1 AND g.kind = 'series' AND a.state = 0 ORDER BY m.rank, m.asset_id",
        )?;
        let rows = st.query_map([series.0 as i64], |r| {
            let id: i64 = r.get(0)?;
            let keeper: Option<i64> = r.get(4)?;
            Ok((
                id,
                r.get::<_, Option<f64>>(1)?,
                r.get::<_, i64>(2)?,
                r.get::<_, i64>(3)?,
                keeper == Some(id),
            ))
        })?;
        let mut out = Vec::new();
        for (i, row) in rows.enumerate() {
            let (id, score, bytes, flags, is_keeper) = row?;
            out.push(SeriesMemberRow {
                asset: AssetId(id as u64),
                rank: i as u32 + 1,
                score: score.unwrap_or(0.0) as f32,
                is_keeper,
                bytes: bytes as u64,
                protect: ProtectFlags(flags as u8),
            });
        }
        Ok(out)
    }

    /// Serien (om noen) et aktivt bilde er med i.
    pub fn series_of(&self, asset: AssetId) -> Result<Option<SeriesId>, StoreError> {
        let id: Option<i64> = self
            .conn
            .prepare_cached(
                "SELECT g.id FROM grp g JOIN grp_member m ON m.grp_id = g.id \
                 WHERE g.kind = 'series' AND m.asset_id = ?1 LIMIT 1",
            )?
            .query_row([asset.0 as i64], |r| r.get(0))
            .optional()?;
        Ok(id.map(|i| SeriesId(i as u64)))
    }

    /// Antall serier med minst to aktive medlemmer.
    pub fn series_count(&self) -> Result<u64, StoreError> {
        let n: i64 = self.conn.query_row(
            "SELECT count(*) FROM (SELECT g.id FROM grp g JOIN grp_member m ON m.grp_id = g.id \
             JOIN asset a ON a.id = m.asset_id WHERE g.kind = 'series' AND a.state = 0 \
             GROUP BY g.id HAVING count(*) >= 2)",
            [],
            |r| r.get(0),
        )?;
        Ok(n as u64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Album, AlgoVer, AssetMeta, Kind, SourceId};
    use crate::store::FingerprintRow;

    fn meta(name: &str) -> AssetMeta {
        AssetMeta {
            source_id: SourceId(name.into()),
            kind: Kind::Photo,
            created_at: 1,
            mod_token: "t".into(),
            width: 10,
            height: 10,
            bytes: 100,
            mime: "image/jpeg".into(),
            platform_screenshot_hint: false,
            has_camera_exif: None,
            is_local: true,
            protect_flags: ProtectFlags(ProtectFlags::FAVORITE),
            albums: Vec::<Album>::new(),
            backup: None,
        }
    }

    /// Lager `n` aktive bilder med fingeravtrykk; id 1..=n.
    fn store_with(n: u64) -> Store {
        let mut s = Store::open_in_memory().unwrap();
        let metas: Vec<AssetMeta> = (1..=n).map(|i| meta(&format!("a{i}"))).collect();
        s.upsert_assets(&metas).unwrap();
        let rows: Vec<FingerprintRow> = (1..=n)
            .map(|i| FingerprintRow {
                id: AssetId(i),
                dhash: 0,
                phash: 0,
                blur: 0.3,
                signals: 0,
                algo: AlgoVer(1),
            })
            .collect();
        s.put_fingerprints(&rows).unwrap();
        s
    }

    fn spec(keeper: u64, others: &[u64]) -> SeriesSpec {
        let mk = |a: u64, reason| SeriesMemberSpec {
            asset: AssetId(a),
            score: 0.9,
            reason,
            confidence: 0.7,
        };
        let mut members = vec![mk(keeper, ReasonCode::KeeperBestOverall)];
        members.extend(others.iter().map(|&o| mk(o, ReasonCode::SeriesMember)));
        SeriesSpec {
            members,
            keeper: AssetId(keeper),
        }
    }

    fn pending(s: &Store) -> Vec<(i64, String, String)> {
        let mut st = s
            .conn
            .prepare(
                "SELECT asset_id, reason_code, status FROM suggestion WHERE kind = 'series' ORDER BY asset_id",
            )
            .unwrap();
        st.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap()
    }

    #[test]
    fn eye_results_roundtrip_and_missing_follow_algo_version() {
        let mut s = store_with(3);
        assert_eq!(s.eye_result_count().unwrap(), 0);
        assert_eq!(s.eye_missing(1).unwrap().len(), 3);
        let r = EyeReport {
            faces: 2,
            closed: 1,
            confidence: 0.9,
        };
        s.put_eye_results(&[(AssetId(2), r)], 1).unwrap();
        assert_eq!(s.eye_result_count().unwrap(), 1);
        assert_eq!(
            s.eye_results(&[AssetId(1), AssetId(2)])
                .unwrap()
                .get(&AssetId(2)),
            Some(&r)
        );
        assert!(!s
            .eye_results(&[AssetId(1)])
            .unwrap()
            .contains_key(&AssetId(1)));
        assert_eq!(s.eye_missing(1).unwrap(), vec![AssetId(1), AssetId(3)]);
        // Ny modellversjon: alt må analyseres på nytt.
        assert_eq!(s.eye_missing(2).unwrap().len(), 3);
        s.put_eye_results(&[(AssetId(2), EyeReport::default())], 2)
            .unwrap();
        assert_eq!(s.eye_results(&[AssetId(2)]).unwrap()[&AssetId(2)].closed, 0);
    }

    #[test]
    fn apply_series_stores_group_members_and_suggestions_without_keeper_suggestion() {
        let mut s = store_with(4);
        let st = s.apply_series(&[spec(2, &[1, 3])], 10).unwrap();
        assert_eq!((st.groups, st.kept_groups, st.suggestions), (1, 0, 2));
        assert_eq!(s.series_count().unwrap(), 1);
        let p = pending(&s);
        assert_eq!(p.len(), 2);
        assert!(p.iter().all(|x| x.0 != 2 && x.2 == "pending"));
        let sid = s.series_of(AssetId(1)).unwrap().unwrap();
        assert_eq!(s.series_of(AssetId(4)).unwrap(), None);
        let m = s.series_members(sid).unwrap();
        assert_eq!(m.len(), 3);
        assert!(m[0].is_keeper && m[0].asset == AssetId(2) && m[0].rank == 1);
        assert!(m[0].protect.has(ProtectFlags::FAVORITE));
    }

    #[test]
    fn reapply_keeps_group_and_manual_keeper_and_leaves_other_kinds_alone() {
        let mut s = store_with(4);
        // Et ventende visual_dup-forslag skal aldri røres av serier.
        s.conn
            .execute(
                "INSERT INTO suggestion(asset_id, kind, reason_code, confidence) VALUES (4,'blurry','blurry',0.5)",
                [],
            )
            .unwrap();
        s.apply_series(&[spec(2, &[1, 3])], 10).unwrap();
        let sid = s.series_of(AssetId(1)).unwrap().unwrap();
        assert!(s.set_series_keeper(sid, AssetId(1)).unwrap());
        let st = s.apply_series(&[spec(2, &[1, 3])], 20).unwrap();
        assert_eq!(st.kept_groups, 1);
        assert_eq!(s.series_of(AssetId(2)).unwrap(), Some(sid));
        let m = s.series_members(sid).unwrap();
        assert!(m.iter().find(|x| x.asset == AssetId(1)).unwrap().is_keeper);
        let p = pending(&s);
        assert!(
            p.iter().all(|x| x.0 != 1),
            "manuell keeper får ikke forslag"
        );
        // Den automatiske keeperen er nå en vanlig kandidat uten keeper-begrunnelse.
        assert_eq!(p.iter().find(|x| x.0 == 2).unwrap().1, "series_member");
        let blurry: i64 = s
            .conn
            .query_row(
                "SELECT count(*) FROM suggestion WHERE kind = 'blurry'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(blurry, 1);
    }

    #[test]
    fn rejected_suggestion_survives_and_dissolved_series_removes_only_pending() {
        let mut s = store_with(4);
        s.apply_series(&[spec(2, &[1, 3])], 10).unwrap();
        s.conn
            .execute(
                "UPDATE suggestion SET status='rejected' WHERE asset_id = 1 AND kind='series'",
                [],
            )
            .unwrap();
        // Ny gruppering: serien er borte.
        let st = s.apply_series(&[], 20).unwrap();
        assert_eq!(st.removed_suggestions, 1);
        assert_eq!(s.series_count().unwrap(), 0);
        let p = pending(&s);
        assert_eq!(p.len(), 1);
        assert_eq!(p[0].2, "rejected");
        // Serien dukker opp igjen i annen sammensetning: avvisningen overstyres ikke.
        s.apply_series(&[spec(2, &[1, 4])], 30).unwrap();
        let p = pending(&s);
        assert_eq!(p.iter().find(|x| x.0 == 1).unwrap().2, "rejected");
        assert_eq!(p.iter().find(|x| x.0 == 4).unwrap().2, "pending");
    }

    #[test]
    fn set_series_keeper_moves_pending_and_is_undoable() {
        let mut s = store_with(3);
        s.apply_series(&[spec(1, &[2, 3])], 10).unwrap();
        let sid = s.series_of(AssetId(1)).unwrap().unwrap();
        assert!(s.set_series_keeper(sid, AssetId(3)).unwrap());
        let ids: Vec<i64> = pending(&s).iter().map(|x| x.0).collect();
        assert_eq!(ids, vec![1, 2]);
        // Angre: velg forrige keeper.
        assert!(s.set_series_keeper(sid, AssetId(1)).unwrap());
        let ids: Vec<i64> = pending(&s).iter().map(|x| x.0).collect();
        assert_eq!(ids, vec![2, 3]);
        // Ikke-medlem, ukjent serie og trashed medlem avvises.
        assert!(!s.set_series_keeper(sid, AssetId(99)).unwrap());
        assert!(!s.set_series_keeper(SeriesId(999), AssetId(1)).unwrap());
    }

    #[test]
    fn non_series_group_is_not_touched_by_series_calls() {
        let mut s = store_with(3);
        s.conn
            .execute(
                "INSERT INTO grp(kind, keeper_id, created_at) VALUES ('visual', 1, 1)",
                [],
            )
            .unwrap();
        let gid = s.conn.last_insert_rowid() as u64;
        s.conn
            .execute(
                "INSERT INTO grp_member(grp_id, asset_id, rank) VALUES (?1,1,0),(?1,2,1)",
                [gid as i64],
            )
            .unwrap();
        assert!(!s.set_series_keeper(SeriesId(gid), AssetId(2)).unwrap());
        s.apply_series(&[spec(1, &[3])], 5).unwrap();
        let n: i64 = s
            .conn
            .query_row("SELECT count(*) FROM grp WHERE kind = 'visual'", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(n, 1);
        assert!(s.series_members(SeriesId(gid)).unwrap().is_empty());
    }

    #[test]
    fn invalid_specs_are_ignored_and_overlap_first_wins() {
        let mut s = store_with(4);
        let bad_keeper = SeriesSpec {
            keeper: AssetId(9),
            ..spec(1, &[2])
        };
        let single = SeriesSpec {
            members: spec(1, &[]).members,
            keeper: AssetId(1),
        };
        let st = s
            .apply_series(&[bad_keeper, single, spec(1, &[2]), spec(2, &[3, 4])], 1)
            .unwrap();
        assert_eq!(st.groups, 1);
        assert_eq!(s.series_count().unwrap(), 1);
    }

    #[test]
    fn trashed_members_do_not_count() {
        let mut s = store_with(3);
        s.apply_series(&[spec(1, &[2, 3])], 1).unwrap();
        s.conn
            .execute(
                "UPDATE asset SET state = 1, trashed_at = 1, expires_at = 2 WHERE id = 3",
                [],
            )
            .unwrap();
        assert_eq!(s.series_count().unwrap(), 1);
        s.conn
            .execute(
                "UPDATE asset SET state = 1, trashed_at = 1, expires_at = 2 WHERE id = 2",
                [],
            )
            .unwrap();
        assert_eq!(s.series_count().unwrap(), 0);
    }
}
