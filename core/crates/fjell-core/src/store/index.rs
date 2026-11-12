//! Indeksering i lagringslaget (PLAN-KJERNE K6, brukt av K7). Del av `store` (L1).
//!
//! Avhenger av: `model` og `super::Store`. Ingen algoritmer; bare SQL og transaksjonsgrenser.
//! Grensesnitt:
//! - `upsert_assets(&[AssetMeta]) -> UpsertStats`: transaksjoner på 500 rader. Uendret rad skrives
//!   ikke. Endret `mod_token` nullstiller fingeravtrykket (raden beholdes, `algo_ver = 0`). `state`,
//!   `trashed_at` og `expires_at` røres aldri (I13). Purgede rader røres ikke (terminal).
//! - `hash_rows()` (aktive, lokale, ikke video) og `put_content_hashes(..)` for rask fase.
//! - `begin_scan(algo)`: nullstiller `fail_count` når algoritmeversjonen har økt.
//! - `work_set(algo)`: aktive lokale bilder uten fingeravtrykk eller med `algo_ver < algo`, og med
//!   `fail_count < 3`. `put_fingerprints(..)` (én transaksjon per kall), `record_failures(..)`.
//! - `source_ids(ids)`: plattform-ID per id (UI trenger den til plattformens `delete`).
//! - `index_rows()`: alt grupperingen trenger for aktive bilder med fingeravtrykk.
//!
//! Hashene lagres som `i64` (bitmønsteret er bevart); miniatyrer lagres aldri (I16).

use super::{Store, StoreError};
use crate::model::{Album, AlgoVer, AssetId, AssetMeta, BackupState, Kind, ProtectFlags, SourceId};
use rusqlite::{params, OptionalExtension, Transaction};

/// Rader per transaksjon i `upsert_assets`.
const UPSERT_BATCH: usize = 500;
/// Etter så mange feil hoppes bildet over til algoritmeversjonen øker.
pub const MAX_FAILS: i64 = 3;

/// Telling fra `upsert_assets`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct UpsertStats {
    pub new: u64,
    /// `mod_token` endret (fingeravtrykket er nullstilt).
    pub changed: u64,
    /// Bare metadata (flagg, album, ...) oppdatert; fingeravtrykket beholdt.
    pub touched: u64,
    pub unchanged: u64,
}

/// Aktiv, lokal fil for størrelsesforfilteret (rask fase).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HashRow {
    pub id: AssetId,
    pub source: SourceId,
    pub bytes: u64,
    pub hashed: bool,
}

/// Element i arbeidssettet (full fase). `meta.albums` er tom (brukes ikke av analysen).
#[derive(Clone, Debug, PartialEq)]
pub struct WorkItem {
    pub id: AssetId,
    pub meta: AssetMeta,
}

/// Resultat for ett bilde i full fase.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FingerprintRow {
    pub id: AssetId,
    pub dhash: u64,
    pub phash: u64,
    pub blur: f32,
    /// Skjermbildesignaler pluss lavinformasjonsbit (se `pipeline`).
    pub signals: u8,
    pub algo: AlgoVer,
}

/// Rad for gruppering og forslag: aktivt bilde med fingeravtrykk.
#[derive(Clone, Debug, PartialEq)]
pub struct IndexRow {
    pub id: AssetId,
    pub kind: Kind,
    pub width: u32,
    pub height: u32,
    pub bytes: u64,
    pub created_at: i64,
    pub protect: ProtectFlags,
    pub content_hash: Option<[u8; 32]>,
    pub dhash: Option<u64>,
    pub phash: Option<u64>,
    pub blur: Option<f32>,
    pub signals: Option<u8>,
    pub algo: u32,
}

fn kind_of(s: &str) -> Kind {
    match s {
        "screenshot_hint" => Kind::ScreenshotHint,
        "video" => Kind::Video,
        _ => Kind::Photo,
    }
}

fn backup_i(b: Option<BackupState>) -> Option<i64> {
    b.map(|b| (b == BackupState::BackedUp) as i64)
}

/// (kind, created_at, width, height, bytes, mime, hint, exif, local, backup, protect).
type MetaRow = (
    String,
    i64,
    i64,
    i64,
    i64,
    String,
    i64,
    Option<i64>,
    i64,
    Option<i64>,
    i64,
);

/// Eksisterende rad slik `upsert` sammenligner den.
struct Existing {
    id: i64,
    state: i64,
    token: String,
    meta: MetaRow,
}

fn albums_of(tx: &Transaction, id: i64) -> Result<Vec<(i64, String, i64)>, StoreError> {
    let mut st = tx.prepare_cached(
        "SELECT al.id, al.name, al.shared FROM asset_album aa JOIN album al ON al.id = aa.album_id \
         WHERE aa.asset_id = ?1 ORDER BY al.id",
    )?;
    let rows = st.query_map([id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?;
    Ok(rows.collect::<Result<_, _>>()?)
}

fn want_albums(m: &AssetMeta) -> Vec<(i64, String, i64)> {
    let mut v: Vec<_> = m
        .albums
        .iter()
        .map(|a: &Album| (a.id as i64, a.name.clone(), a.shared as i64))
        .collect();
    v.sort();
    v.dedup_by_key(|a| a.0);
    v
}

fn write_albums(tx: &Transaction, id: i64, want: &[(i64, String, i64)]) -> Result<(), StoreError> {
    tx.prepare_cached("DELETE FROM asset_album WHERE asset_id = ?1")?
        .execute([id])?;
    for (aid, name, shared) in want {
        tx.prepare_cached(
            "INSERT INTO album(id, name, shared) VALUES (?1, ?2, ?3) \
             ON CONFLICT(id) DO UPDATE SET name = excluded.name, shared = excluded.shared",
        )?
        .execute(params![aid, name, shared])?;
        tx.prepare_cached("INSERT OR IGNORE INTO asset_album(asset_id, album_id) VALUES (?1, ?2)")?
            .execute(params![id, aid])?;
    }
    Ok(())
}

fn upsert_one(tx: &Transaction, m: &AssetMeta, stats: &mut UpsertStats) -> Result<(), StoreError> {
    let existing: Option<Existing> = tx
        .prepare_cached(
            "SELECT id, state, mod_token, kind, created_at, width, height, bytes, mime, \
             platform_screenshot_hint, has_camera_exif, is_local, backup, protect_flags \
             FROM asset WHERE source_id = ?1",
        )?
        .query_row([&m.source_id.0], |r| {
            Ok(Existing {
                id: r.get(0)?,
                state: r.get(1)?,
                token: r.get(2)?,
                meta: (
                    r.get(3)?,
                    r.get(4)?,
                    r.get(5)?,
                    r.get(6)?,
                    r.get(7)?,
                    r.get(8)?,
                    r.get(9)?,
                    r.get(10)?,
                    r.get(11)?,
                    r.get(12)?,
                    r.get(13)?,
                ),
            })
        })
        .optional()?;
    let now_meta = (
        m.kind.as_str().to_string(),
        m.created_at,
        m.width as i64,
        m.height as i64,
        m.bytes as i64,
        m.mime.clone(),
        m.platform_screenshot_hint as i64,
        m.has_camera_exif.map(|b| b as i64),
        m.is_local as i64,
        backup_i(m.backup),
        m.protect_flags.0 as i64,
    );
    let want = want_albums(m);
    let Some(ex) = existing else {
        tx.prepare_cached(
            "INSERT INTO asset(source_id, kind, created_at, mod_token, width, height, bytes, mime, \
             platform_screenshot_hint, has_camera_exif, is_local, backup, protect_flags) \
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13)",
        )?
        .execute(params![
            m.source_id.0,
            now_meta.0,
            now_meta.1,
            m.mod_token,
            now_meta.2,
            now_meta.3,
            now_meta.4,
            now_meta.5,
            now_meta.6,
            now_meta.7,
            now_meta.8,
            now_meta.9,
            now_meta.10
        ])?;
        let id = tx.last_insert_rowid();
        write_albums(tx, id, &want)?;
        stats.new += 1;
        return Ok(());
    };
    if ex.state == 2 {
        stats.unchanged += 1; // purged er terminal
        return Ok(());
    }
    let token_changed = ex.token != m.mod_token;
    let albums_same = albums_of(tx, ex.id)? == want;
    if !token_changed && ex.meta == now_meta && albums_same {
        stats.unchanged += 1;
        return Ok(());
    }
    // `state`, `trashed_at`, `expires_at` og `expired_notified_at` er bevisst ikke med (I13).
    tx.prepare_cached(
        "UPDATE asset SET kind=?2, created_at=?3, mod_token=?4, width=?5, height=?6, bytes=?7, \
         mime=?8, platform_screenshot_hint=?9, has_camera_exif=?10, is_local=?11, backup=?12, \
         protect_flags=?13 WHERE id=?1",
    )?
    .execute(params![
        ex.id,
        now_meta.0,
        now_meta.1,
        m.mod_token,
        now_meta.2,
        now_meta.3,
        now_meta.4,
        now_meta.5,
        now_meta.6,
        now_meta.7,
        now_meta.8,
        now_meta.9,
        now_meta.10
    ])?;
    if !albums_same {
        write_albums(tx, ex.id, &want)?;
    }
    if token_changed {
        tx.prepare_cached(
            "UPDATE fingerprint SET content_hash=NULL, dhash=NULL, phash=NULL, blur_score=NULL, \
             shot_signals=NULL, algo_ver=0, fail_count=0 WHERE asset_id=?1",
        )?
        .execute([ex.id])?;
        stats.changed += 1;
    } else {
        stats.touched += 1;
    }
    Ok(())
}

fn u64_of(v: Option<i64>) -> Option<u64> {
    v.map(|v| v as u64)
}

impl Store {
    pub fn upsert_assets(&mut self, metas: &[AssetMeta]) -> Result<UpsertStats, StoreError> {
        let mut stats = UpsertStats::default();
        for chunk in metas.chunks(UPSERT_BATCH) {
            let tx = self.conn.transaction()?;
            for m in chunk {
                upsert_one(&tx, m, &mut stats)?;
            }
            tx.commit()?;
        }
        Ok(stats)
    }

    /// Nullstiller `fail_count` hvis algoritmeversjonen er en annen enn sist (setting `algo_ver`).
    pub fn begin_scan(&mut self, algo: AlgoVer) -> Result<(), StoreError> {
        let cur = algo.0.to_string();
        if self.setting("algo_ver")?.as_deref() == Some(cur.as_str()) {
            return Ok(());
        }
        let tx = self.conn.transaction()?;
        tx.execute(
            "UPDATE fingerprint SET fail_count = 0 WHERE fail_count > 0",
            [],
        )?;
        tx.execute(
            "INSERT INTO setting(key, value) VALUES ('algo_ver', ?1) \
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            [&cur],
        )?;
        tx.commit()?;
        Ok(())
    }

    /// Aktive, lokale bilder (ikke video) for størrelsesforfilteret.
    pub fn hash_rows(&self) -> Result<Vec<HashRow>, StoreError> {
        let mut st = self.conn.prepare_cached(
            "SELECT a.id, a.source_id, a.bytes, f.content_hash IS NOT NULL FROM asset a \
             LEFT JOIN fingerprint f ON f.asset_id = a.id \
             WHERE a.state = 0 AND a.is_local = 1 AND a.kind != 'video' ORDER BY a.id",
        )?;
        let rows = st.query_map([], |r| {
            Ok(HashRow {
                id: AssetId(r.get::<_, i64>(0)? as u64),
                source: SourceId(r.get(1)?),
                bytes: r.get::<_, i64>(2)? as u64,
                hashed: r.get(3)?,
            })
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// Lagrer innholdshasher (rask fase) i én transaksjon. Oppretter fingeravtrykksraden ved behov.
    pub fn put_content_hashes(&mut self, hashes: &[(AssetId, [u8; 32])]) -> Result<(), StoreError> {
        let tx = self.conn.transaction()?;
        for (id, h) in hashes {
            tx.prepare_cached(
                "INSERT INTO fingerprint(asset_id, content_hash) VALUES (?1, ?2) \
                 ON CONFLICT(asset_id) DO UPDATE SET content_hash = excluded.content_hash",
            )?
            .execute(params![id.0 as i64, &h[..]])?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn work_set(&self, algo: AlgoVer) -> Result<Vec<WorkItem>, StoreError> {
        let mut st = self.conn.prepare_cached(
            "SELECT a.id, a.source_id, a.kind, a.created_at, a.mod_token, a.width, a.height, a.bytes, \
             a.mime, a.platform_screenshot_hint, a.has_camera_exif, a.is_local, a.backup, a.protect_flags \
             FROM asset a LEFT JOIN fingerprint f ON f.asset_id = a.id \
             WHERE a.state = 0 AND a.is_local = 1 AND a.kind != 'video' \
             AND (f.asset_id IS NULL OR f.algo_ver < ?1) AND COALESCE(f.fail_count, 0) < ?2 \
             ORDER BY a.id",
        )?;
        let rows = st.query_map(params![algo.0 as i64, MAX_FAILS], |r| {
            let backup: Option<i64> = r.get(12)?;
            Ok(WorkItem {
                id: AssetId(r.get::<_, i64>(0)? as u64),
                meta: AssetMeta {
                    source_id: SourceId(r.get(1)?),
                    kind: kind_of(&r.get::<_, String>(2)?),
                    created_at: r.get(3)?,
                    mod_token: r.get(4)?,
                    width: r.get::<_, i64>(5)? as u32,
                    height: r.get::<_, i64>(6)? as u32,
                    bytes: r.get::<_, i64>(7)? as u64,
                    mime: r.get(8)?,
                    platform_screenshot_hint: r.get(9)?,
                    has_camera_exif: r.get::<_, Option<i64>>(10)?.map(|v| v != 0),
                    is_local: r.get(11)?,
                    protect_flags: ProtectFlags(r.get::<_, i64>(13)? as u8),
                    albums: Vec::new(),
                    backup: backup.map(|v| {
                        if v != 0 {
                            BackupState::BackedUp
                        } else {
                            BackupState::NotBackedUp
                        }
                    }),
                },
            })
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// Lagrer resultater (kall med høyst 100 rader om gangen). Alt eller ingenting.
    pub fn put_fingerprints(&mut self, rows: &[FingerprintRow]) -> Result<(), StoreError> {
        let tx = self.conn.transaction()?;
        for r in rows {
            tx.prepare_cached(
                "INSERT INTO fingerprint(asset_id, dhash, phash, blur_score, shot_signals, algo_ver) \
                 VALUES (?1,?2,?3,?4,?5,?6) ON CONFLICT(asset_id) DO UPDATE SET dhash=excluded.dhash, \
                 phash=excluded.phash, blur_score=excluded.blur_score, shot_signals=excluded.shot_signals, \
                 algo_ver=excluded.algo_ver, fail_count=0",
            )?
            .execute(params![
                r.id.0 as i64,
                r.dhash as i64,
                r.phash as i64,
                r.blur as f64,
                r.signals as i64,
                r.algo.0 as i64
            ])?;
        }
        tx.commit()?;
        Ok(())
    }

    /// Øker `fail_count` (oppretter raden ved behov). Ved `MAX_FAILS` hoppes bildet over.
    pub fn record_failures(&mut self, ids: &[AssetId]) -> Result<(), StoreError> {
        let tx = self.conn.transaction()?;
        for id in ids {
            tx.prepare_cached(
                "INSERT INTO fingerprint(asset_id, fail_count) VALUES (?1, 1) \
                 ON CONFLICT(asset_id) DO UPDATE SET fail_count = fail_count + 1",
            )?
            .execute([id.0 as i64])?;
        }
        tx.commit()?;
        Ok(())
    }

    /// Plattform-ID for hver id (til `PlatformDelete::delete`). Ukjente id-er utelates.
    pub fn source_ids(&self, ids: &[AssetId]) -> Result<Vec<(AssetId, SourceId)>, StoreError> {
        let mut st = self
            .conn
            .prepare_cached("SELECT source_id FROM asset WHERE id = ?1")?;
        let mut out = Vec::new();
        for id in ids {
            if let Some(s) = st
                .query_row([id.0 as i64], |r| r.get::<_, String>(0))
                .optional()?
            {
                out.push((*id, SourceId(s)));
            }
        }
        Ok(out)
    }

    /// Plattform-ID for alle AKTIVE bilder (ikke papirkurv, ikke purgede). Kilder bruker den til å
    /// oppdage filer som er fjernet utenfor Fjell (T11); papirkurvbilder ligger i karantenen med vilje.
    pub fn active_source_ids(&self) -> Result<Vec<SourceId>, StoreError> {
        active_source_ids_on(&self.conn)
    }

    /// Aktive bilder med fingeravtrykksrad, sortert på id.
    pub fn index_rows(&self) -> Result<Vec<IndexRow>, StoreError> {
        index_rows_on(&self.conn)
    }
}

pub(super) fn active_source_ids_on(
    conn: &rusqlite::Connection,
) -> Result<Vec<SourceId>, StoreError> {
    let mut st = conn.prepare_cached("SELECT source_id FROM asset WHERE state = 0")?;
    let rows = st.query_map([], |r| r.get::<_, String>(0))?;
    Ok(rows
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .map(SourceId)
        .collect())
}

/// Som `Store::index_rows`, men på en gitt tilkobling (f.eks. `Store::reader`, utenfor skrivelåsen).
pub fn index_rows_on(conn: &rusqlite::Connection) -> Result<Vec<IndexRow>, StoreError> {
    {
        let mut st = conn.prepare_cached(
            "SELECT a.id, a.kind, a.width, a.height, a.bytes, a.created_at, a.protect_flags, \
             f.content_hash, f.dhash, f.phash, f.blur_score, f.shot_signals, f.algo_ver \
             FROM asset a JOIN fingerprint f ON f.asset_id = a.id WHERE a.state = 0 ORDER BY a.id",
        )?;
        let rows = st.query_map([], |r| {
            let hash: Option<Vec<u8>> = r.get(7)?;
            Ok(IndexRow {
                id: AssetId(r.get::<_, i64>(0)? as u64),
                kind: kind_of(&r.get::<_, String>(1)?),
                width: r.get::<_, i64>(2)? as u32,
                height: r.get::<_, i64>(3)? as u32,
                bytes: r.get::<_, i64>(4)? as u64,
                created_at: r.get(5)?,
                protect: ProtectFlags(r.get::<_, i64>(6)? as u8),
                content_hash: hash.and_then(|h| <[u8; 32]>::try_from(h.as_slice()).ok()),
                dhash: u64_of(r.get(8)?),
                phash: u64_of(r.get(9)?),
                blur: r.get::<_, Option<f64>>(10)?.map(|v| v as f32),
                signals: r.get::<_, Option<i64>>(11)?.map(|v| v as u8),
                algo: r.get::<_, i64>(12)? as u32,
            })
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    }
}
