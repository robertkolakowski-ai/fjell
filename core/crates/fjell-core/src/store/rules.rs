//! Lagring for V1-grenen `rules` (store, L1). Eier alle SQL-spørringer for `rule` og `rule_hit`.
//!
//! Avhenger av: `super` (`Store`), `crate::model`. Filen eies av grenen `rules` (docs/v1/V1-PLAN.md);
//! ingen andre grener rører den. Skjemaet er ferdig i migrering 2 (`002_v1.sql`) og endres ikke av grenen.
//! Grensesnitt:
//! - Regelrader: `rule_insert`, `rule_update`, `rule_rows`, `rule_row`, `rule_set_enabled`, `rule_set_deleted`
//!   (sletting er alltid `deleted_at`; raden fjernes aldri, triggeren `rule_no_delete` håndhever det).
//! - `rule_scan`: ett lesepass over aktive, ubeskyttede bilder som leverer én `ScanRow` per bilde; vilkårene
//!   avgjøres av modulen `rules` (ett pass for alle regler er mye raskere enn ett SQL-søk per regel).
//! - `rule_apply`: skriver `rule_hit` og synkroniserer ventende `rule`-forslag i én transaksjon.
//!   Den rører aldri `asset.state` eller `action_log`: regler foreslår, brukeren avgjør.
//!
//! `rule_scan` leverer bare aktive bilder (`state = 0`) som ikke er beskyttet (`protect_flags`, og `protection`
//! for bilde, mappe, album og person inkl. sammenslåtte personer): beskyttelse vinner alltid over regler.

use super::{Store, StoreError};
use crate::model::{AssetId, ReasonCode, RuleCondition, RuleId, RuleSpec};
use rusqlite::{params, OptionalExtension};
use std::collections::HashSet;

/// Uskarphetsterskel for `IsBlurry` uten ferdig forslag; speiler `quality::BLUR_ABS_MAX` (L1, ikke tillatt
/// som avhengighet for denne grenen). Endres den der, må den endres her.
pub const BLUR_SCORE_MAX: f64 = 0.006;
/// Konfidens for forslag fra regler. Brukeren har selv skrevet regelen, men forslaget er fortsatt et forslag.
pub const RULE_CONFIDENCE: f64 = 0.8;

/// Rad i `rule`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RuleRow {
    pub id: RuleId,
    pub name: String,
    /// Kodet `RuleSpec` (kodingen eies av modulen `rules`).
    pub spec: String,
    pub enabled: bool,
    pub created_at: i64,
    pub deleted_at: Option<i64>,
}

/// Resultat av `rule_apply`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct RuleApplyStats {
    /// Rader i `rule_hit` etter synkroniseringen.
    pub hits: u64,
    pub suggestions_added: u64,
    pub suggestions_removed: u64,
}

/// Ett aktivt, ubeskyttet bilde slik reglene ser det.
pub struct ScanRow<'a> {
    pub id: AssetId,
    pub created_at: i64,
    pub bytes: u64,
    /// Tom hvis ingen regel spør om MIME eller mappe og ingen mappe er beskyttet.
    pub mime: &'a str,
    pub source_id: &'a str,
    /// Plattformhint, `kind = screenshot_hint` eller et `screenshot`-forslag. Bare satt hvis en regel spør.
    pub screenshot: bool,
    /// Under uskarphetsterskelen eller har `blurry`-forslag. Bare satt hvis en regel spør.
    pub blurry: bool,
    /// Ikke-keeper i en eksakt gruppe. Bare satt hvis en regel spør.
    pub exact_dup: bool,
    /// De av albumene reglene spør om som bildet ligger i.
    pub albums: &'a [u64],
}

const PROTECTED_PERSONS_CTE: &str = "WITH RECURSIVE prot_person(id) AS (\
    SELECT CAST(target AS INTEGER) FROM protection WHERE scope = 'person' \
    UNION SELECT pe.id FROM person pe JOIN prot_person pp ON pe.merged_into = pp.id)";

fn row_to_rule(r: &rusqlite::Row<'_>) -> rusqlite::Result<RuleRow> {
    Ok(RuleRow {
        id: RuleId(r.get::<_, i64>(0)? as u64),
        name: r.get(1)?,
        spec: r.get(2)?,
        enabled: r.get::<_, i64>(3)? != 0,
        created_at: r.get(4)?,
        deleted_at: r.get(5)?,
    })
}

const RULE_COLS: &str = "id, name, spec, enabled, created_at, deleted_at";

/// Sortert, unik liste med id-er. Passet går i stigende id, så en markør som bare flyttes framover erstatter
/// hash-oppslag (50 000 bilder × flere mengder er ellers en merkbar del av budsjettet).
struct IdCursor {
    ids: Vec<i64>,
    at: usize,
}

impl IdCursor {
    fn new(mut ids: Vec<i64>) -> IdCursor {
        ids.sort_unstable();
        ids.dedup();
        IdCursor { ids, at: 0 }
    }

    /// `id` må ikke minke mellom kall.
    fn has(&mut self, id: i64) -> bool {
        while self.at < self.ids.len() && self.ids[self.at] < id {
            self.at += 1;
        }
        self.ids.get(self.at) == Some(&id)
    }

    fn is_empty(&self) -> bool {
        self.ids.is_empty()
    }
}

fn id_list(conn: &rusqlite::Connection, sql: &str) -> Result<Vec<i64>, StoreError> {
    let mut st = conn.prepare(sql)?;
    let v = st
        .query_map([], |r| r.get::<_, i64>(0))?
        .collect::<Result<_, _>>()?;
    Ok(v)
}

/// Differansen mellom to sorterte, unike lister: (bare i `old`, bare i `new`).
fn sorted_diff(old: &[i64], new: &[i64]) -> (Vec<i64>, Vec<i64>) {
    let (mut gone, mut added) = (Vec::new(), Vec::new());
    let (mut i, mut j) = (0, 0);
    while i < old.len() || j < new.len() {
        match (old.get(i), new.get(j)) {
            (Some(o), Some(n)) if o == n => {
                i += 1;
                j += 1;
            }
            (Some(o), Some(n)) if o < n => {
                gone.push(*o);
                i += 1;
            }
            (Some(_), Some(n)) => {
                added.push(*n);
                j += 1;
            }
            (Some(o), None) => {
                gone.push(*o);
                i += 1;
            }
            (None, Some(n)) => {
                added.push(*n);
                j += 1;
            }
            (None, None) => unreachable!(),
        }
    }
    (gone, added)
}

impl Store {
    /// Antall rader i `rule`.
    pub fn rule_count(&self) -> Result<u64, StoreError> {
        let n: i64 = self
            .conn
            .query_row("SELECT count(*) FROM rule", [], |r| r.get(0))?;
        Ok(n as u64)
    }

    /// Ny regel, aktiv. Returnerer id.
    pub fn rule_insert(&mut self, name: &str, spec: &str, now: i64) -> Result<RuleId, StoreError> {
        self.conn.execute(
            "INSERT INTO rule(name, spec, enabled, created_at) VALUES (?1, ?2, 1, ?3)",
            params![name, spec, now],
        )?;
        Ok(RuleId(self.conn.last_insert_rowid() as u64))
    }

    /// Endrer navn og spesifikasjon på en regel som ikke er slettet. `false` hvis den ikke finnes.
    pub fn rule_update(&mut self, id: RuleId, name: &str, spec: &str) -> Result<bool, StoreError> {
        let n = self.conn.execute(
            "UPDATE rule SET name = ?2, spec = ?3 WHERE id = ?1 AND deleted_at IS NULL",
            params![id.0 as i64, name, spec],
        )?;
        Ok(n == 1)
    }

    /// Alle regler (også slettede), eldste først.
    pub fn rule_rows(&self) -> Result<Vec<RuleRow>, StoreError> {
        let mut st = self
            .conn
            .prepare_cached(&format!("SELECT {RULE_COLS} FROM rule ORDER BY id"))?;
        let rows = st.query_map([], row_to_rule)?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    pub fn rule_row(&self, id: RuleId) -> Result<Option<RuleRow>, StoreError> {
        Ok(self
            .conn
            .prepare_cached(&format!("SELECT {RULE_COLS} FROM rule WHERE id = ?1"))?
            .query_row([id.0 as i64], row_to_rule)
            .optional()?)
    }

    /// Slår en regel av eller på (ikke slettede). `false` hvis den ikke finnes.
    pub fn rule_set_enabled(&mut self, id: RuleId, enabled: bool) -> Result<bool, StoreError> {
        let n = self.conn.execute(
            "UPDATE rule SET enabled = ?2 WHERE id = ?1 AND deleted_at IS NULL",
            params![id.0 as i64, enabled as i64],
        )?;
        Ok(n == 1)
    }

    /// Soft-sletting (`Some(tid)`) eller gjenoppretting (`None`). `false` hvis regelen ikke finnes.
    pub fn rule_set_deleted(&mut self, id: RuleId, at: Option<i64>) -> Result<bool, StoreError> {
        let n = self.conn.execute(
            "UPDATE rule SET deleted_at = ?2 WHERE id = ?1",
            params![id.0 as i64, at],
        )?;
        Ok(n == 1)
    }

    /// Leser alle aktive, ubeskyttede bilder i stigende id og kaller `f` for hver. Endrer ingenting.
    /// `specs` styrer hva som beregnes: flaggene og tekstkolonnene koster hver sitt og leses bare når en regel
    /// bruker dem.
    pub fn rule_scan(
        &self,
        specs: &[&RuleSpec],
        mut f: impl FnMut(ScanRow<'_>),
    ) -> Result<(), StoreError> {
        let c = &self.conn;
        let (mut albums, mut screenshot, mut blurry, mut exact_dup, mut text) =
            (Vec::<u64>::new(), false, false, false, false);
        for cond in specs.iter().flat_map(|s| s.all_of.iter()) {
            match cond {
                RuleCondition::InAlbum(a) if !albums.contains(a) => albums.push(*a),
                RuleCondition::IsScreenshot => screenshot = true,
                RuleCondition::IsBlurry => blurry = true,
                RuleCondition::IsExactDuplicate => exact_dup = true,
                RuleCondition::Mime(_) | RuleCondition::InFolder(_) => text = true,
                _ => {}
            }
        }
        // Beskyttelse: listene bygges én gang per pass og er tomme når ingenting er beskyttet.
        let mut protected = id_list(
            c,
            "SELECT CAST(target AS INTEGER) FROM protection WHERE scope = 'asset'",
        )?;
        protected.extend(id_list(
            c,
            "SELECT aa.asset_id FROM asset_album aa JOIN protection p \
             ON p.scope = 'album' AND p.target = CAST(aa.album_id AS TEXT)",
        )?);
        protected.extend(id_list(
            c,
            &format!(
                "{PROTECTED_PERSONS_CTE} SELECT asset_id FROM face_embedding \
                 WHERE person_id IN (SELECT id FROM prot_person)"
            ),
        )?);
        let mut protected = IdCursor::new(protected);
        let folders: Vec<String> = {
            let mut st = c.prepare("SELECT target FROM protection WHERE scope = 'folder'")?;
            let rows = st.query_map([], |r| r.get(0))?;
            rows.collect::<Result<_, _>>()?
        };
        text |= !folders.is_empty();
        let mut shot = IdCursor::new(if screenshot {
            let mut v = id_list(
                c,
                "SELECT asset_id FROM suggestion WHERE kind = 'screenshot'",
            )?;
            v.extend(id_list(
                c,
                "SELECT id FROM asset NOT INDEXED \
                 WHERE kind = 'screenshot_hint' OR platform_screenshot_hint = 1",
            )?);
            v
        } else {
            Vec::new()
        });
        let mut blur_set = IdCursor::new(if blurry {
            let mut v = id_list(c, "SELECT asset_id FROM suggestion WHERE kind = 'blurry'")?;
            let mut st = c.prepare("SELECT asset_id FROM fingerprint WHERE blur_score < ?1")?;
            for r in st.query_map([BLUR_SCORE_MAX], |r| r.get::<_, i64>(0))? {
                v.push(r?);
            }
            v
        } else {
            Vec::new()
        });
        let mut dup_set = IdCursor::new(if exact_dup {
            id_list(
                c,
                "SELECT m.asset_id FROM grp_member m JOIN grp g ON g.id = m.grp_id \
                 WHERE g.kind = 'exact' AND g.keeper_id IS NOT NULL AND g.keeper_id <> m.asset_id",
            )?
        } else {
            Vec::new()
        });
        // Album: parallelle lister sortert på bilde-id; `album_at` er markøren.
        let (mut album_asset, mut album_id): (Vec<i64>, Vec<u64>) = (Vec::new(), Vec::new());
        if !albums.is_empty() {
            let list = albums
                .iter()
                .map(|a| (*a).min(i64::MAX as u64).to_string())
                .collect::<Vec<_>>()
                .join(",");
            let mut st = c.prepare(&format!(
                "SELECT asset_id, album_id FROM asset_album WHERE album_id IN ({list}) ORDER BY asset_id"
            ))?;
            for r in st.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?)))? {
                let (asset, album) = r?;
                album_asset.push(asset);
                album_id.push(album as u64);
            }
        }
        let mut album_at = 0usize;
        let mut st = c.prepare(if text {
            "SELECT id, created_at, bytes, mime, source_id FROM asset NOT INDEXED \
             WHERE state = 0 AND protect_flags = 0 ORDER BY id"
        } else {
            "SELECT id, created_at, bytes, '', '' FROM asset NOT INDEXED \
             WHERE state = 0 AND protect_flags = 0 ORDER BY id"
        })?;
        let mut rows = st.query([])?;
        while let Some(r) = rows.next()? {
            let id: i64 = r.get(0)?;
            let source_id = r.get_ref(4)?.as_str().map_err(rusqlite::Error::from)?;
            if (!protected.is_empty() && protected.has(id))
                || folders.iter().any(|p| source_id.starts_with(p.as_str()))
            {
                continue;
            }
            while album_at < album_asset.len() && album_asset[album_at] < id {
                album_at += 1;
            }
            let mut album_end = album_at;
            while album_end < album_asset.len() && album_asset[album_end] == id {
                album_end += 1;
            }
            f(ScanRow {
                id: AssetId(id as u64),
                created_at: r.get(1)?,
                bytes: r.get::<_, i64>(2)?.max(0) as u64,
                mime: r.get_ref(3)?.as_str().map_err(rusqlite::Error::from)?,
                source_id,
                screenshot: !shot.is_empty() && shot.has(id),
                blurry: !blur_set.is_empty() && blur_set.has(id),
                exact_dup: !dup_set.is_empty() && dup_set.has(id),
                albums: &album_id[album_at..album_end],
            });
        }
        Ok(())
    }

    /// Regelen med lavest id som treffer bildet (begrunnelsesdetaljen `ReasonDetail::Rule`).
    pub fn rule_hit_for_asset(&self, asset: AssetId) -> Result<Option<RuleId>, StoreError> {
        Ok(self
            .conn
            .prepare_cached("SELECT min(rule_id) FROM rule_hit WHERE asset_id = ?1")?
            .query_row([asset.0 as i64], |r| r.get::<_, Option<i64>>(0))?
            .map(|n| RuleId(n as u64)))
    }

    /// Antall treff per regel (rader i `rule_hit`).
    pub fn rule_hit_counts(&self) -> Result<Vec<(RuleId, u64)>, StoreError> {
        let mut st = self
            .conn
            .prepare_cached("SELECT rule_id, count(*) FROM rule_hit GROUP BY rule_id")?;
        let rows = st.query_map([], |r| {
            Ok((
                RuleId(r.get::<_, i64>(0)? as u64),
                r.get::<_, i64>(1)? as u64,
            ))
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// Skriver treffene til `rule_hit` og synkroniserer ventende forslag av typen `rule`, atomisk.
    /// `hits` er treffene til alle aktive regler; treff for andre regler (av, slettet) fjernes.
    ///
    /// - Forslag legges til med status `pending` og røres ellers ikke: `accepted` og `rejected` består, og et
    ///   avvist forslag kommer ikke tilbake (`UNIQUE(asset_id, kind)`).
    /// - Ventende forslag uten treff lenger fjernes (avledet data, som ved gruppering).
    /// - `asset` og `action_log` røres aldri.
    pub fn rule_apply(
        &mut self,
        hits: &[(RuleId, Vec<AssetId>)],
        now: i64,
    ) -> Result<RuleApplyStats, StoreError> {
        let tx = self.conn.transaction()?;
        // Differanse mot lagrede treff: i jevn drift (få endringer) skrives nesten ingenting.
        let stored: Vec<(i64, i64)> = {
            let mut st =
                tx.prepare("SELECT rule_id, asset_id FROM rule_hit ORDER BY rule_id, asset_id")?;
            let rows = st.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?)))?;
            rows.collect::<Result<_, _>>()?
        };
        let active: HashSet<i64> = hits.iter().map(|(r, _)| r.0 as i64).collect();
        let stored_for = |rule: i64| -> Vec<i64> {
            let from = stored.partition_point(|(r, _)| *r < rule);
            let to = stored.partition_point(|(r, _)| *r <= rule);
            stored[from..to].iter().map(|(_, a)| *a).collect()
        };
        for r in stored
            .iter()
            .map(|(r, _)| *r)
            .filter(|r| !active.contains(r))
        {
            tx.prepare_cached("DELETE FROM rule_hit WHERE rule_id = ?1")?
                .execute([r])?;
        }
        let mut desired: Vec<i64> = Vec::new();
        {
            let mut ins = tx.prepare_cached(
                "INSERT INTO rule_hit(rule_id, asset_id, hit_at) VALUES (?1, ?2, ?3)",
            )?;
            let mut del =
                tx.prepare_cached("DELETE FROM rule_hit WHERE rule_id = ?1 AND asset_id = ?2")?;
            for (rule, ids) in hits {
                let rule = rule.0 as i64;
                let mut new: Vec<i64> = ids.iter().map(|a| a.0 as i64).collect();
                new.sort_unstable();
                new.dedup();
                let (gone, added) = sorted_diff(&stored_for(rule), &new);
                for a in gone {
                    del.execute(params![rule, a])?;
                }
                for a in added {
                    ins.execute(params![rule, a, now])?;
                }
                desired.extend(new);
            }
        }
        // Forslag: ett per bilde med minst ett treff. Bare ventende forslag fjernes; `accepted` og `rejected`
        // består, og et avvist forslag kommer ikke tilbake (`UNIQUE(asset_id, kind)`).
        desired.sort_unstable();
        desired.dedup();
        let existing: Vec<(i64, bool)> = {
            let mut st = tx.prepare(
                "SELECT asset_id, status = 'pending' FROM suggestion WHERE kind = 'rule' ORDER BY asset_id",
            )?;
            let rows = st.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, bool>(1)?)))?;
            rows.collect::<Result<_, _>>()?
        };
        let existing_ids: Vec<i64> = existing.iter().map(|(a, _)| *a).collect();
        let (stale, missing) = sorted_diff(&existing_ids, &desired);
        let (mut removed, mut added) = (0usize, 0usize);
        {
            let mut del = tx.prepare_cached(
                "DELETE FROM suggestion WHERE asset_id = ?1 AND kind = 'rule' AND status = 'pending'",
            )?;
            for a in stale {
                removed += del.execute([a])?;
            }
            let mut ins = tx.prepare_cached(
                "INSERT INTO suggestion(asset_id, kind, reason_code, confidence) \
                 VALUES (?1, 'rule', ?2, ?3) ON CONFLICT(asset_id, kind) DO NOTHING",
            )?;
            for a in missing {
                added += ins.execute(params![a, ReasonCode::Rule.key(), RULE_CONFIDENCE])?;
            }
        }
        let total: i64 = tx.query_row("SELECT count(*) FROM rule_hit", [], |r| r.get(0))?;
        tx.commit()?;
        Ok(RuleApplyStats {
            hits: total as u64,
            suggestions_added: added as u64,
            suggestions_removed: removed as u64,
        })
    }
}
