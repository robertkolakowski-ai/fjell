//! Sikkerhetskopi, integritetssjekk og gjenoppretting av indeksen (`store`, L1). Se docs/v1/BACKUP.md.
//!
//! Avhenger av: model (`BackupId`, `BackupKind`, `IntegrityStatus`), `migrate` (antall skjemaversjoner), blake3.
//! Modulen er privat i `store/mod.rs` (delt fil), så alt er `impl Store` med modelltyper og primitiver
//! i signaturene; returtypene (`BackupMeta`, `IndexCheck`, ...) er `pub`, men kan ikke navngis utenfor `store`.
//!
//! Regler (CLAUDE.md «Ufravikelig», docs/v1/ROBUSTHET-KONTRAKT.md):
//! - Kopien er bare indeksen (`VACUUM INTO` via en egen skrivebeskyttet tilkobling, så en åpen WAL-database
//!   kopieres konsistent og skriveren aldri blokkeres). Aldri bildefiler; meta merker kopien `content=index_only`.
//! - En kopi finnes først når `.meta` ligger der (meta skrives sist, atomisk). Halvferdige `.tmp`-filer ryddes.
//! - Gjenoppretting er angrebar og aldri destruktiv: en `PreRestore`-kopi tas først, den nye indeksen kopieres
//!   til siden og verifiseres (sjekksum, integritet, skjema) før noe byttes, og den gamle flyttes bare til siden
//!   (`.before-restore`), aldri slettet før den nye er åpnet. Feil midtveis ruller tilbake; krasj midtveis
//!   rettes av `recover_interrupted_restore`.
//! - Feil er koder (`BackupError::code`/`cause`), ingen delvis skriving (skriv til `.tmp`, så `rename`).
//! - En ødelagt indeks slettes aldri: `quarantine_corrupt_index` flytter den til siden (`.corrupt-<tid>`).

use super::migrate::MIGRATIONS;
use super::{Store, StoreError};
use crate::model::{BackupId, BackupKind, IntegrityStatus};
use rusqlite::{Connection, ErrorCode, OpenFlags};
use std::fs::{self, File};
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Format-versjon for `.meta`.
pub const BACKUP_FORMAT: u32 = 1;
/// Antall automatiske kopier som beholdes når ingenting annet er satt (`setting` `backup_keep_auto`).
pub const DEFAULT_KEEP_AUTO: usize = 5;
/// Nøkkel i `setting` for antall automatiske kopier som beholdes.
pub const KEEP_AUTO_KEY: &str = "backup_keep_auto";
const MAX_KEEP_AUTO: usize = 50;
const BUSY_MS: u64 = 1000;

/// Hvorfor en kopi eller gjenoppretting feilet (stabile koder til diagnose og tester).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BackupCause {
    DiskFull,
    WriteFailed,
    /// Filen er låst av noen andre (annen prosess, antivirus, åpen tilkobling).
    Locked,
    /// Kilden (live-indeksen) er ikke hel; ingen kopi tas av en ødelagt indeks.
    SourceCorrupt,
    /// Ingen indeksfil (minnedatabase).
    NoFile,
    Io,
    /// Avbrutt av feilinjeksjon i tester (simulert krasj).
    Interrupted,
}

impl BackupCause {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::DiskFull => "disk_full",
            Self::WriteFailed => "write_failed",
            Self::Locked => "locked",
            Self::SourceCorrupt => "source_corrupt",
            Self::NoFile => "no_file",
            Self::Io => "io",
            Self::Interrupted => "interrupted",
        }
    }
}

/// Feil fra backup og gjenoppretting.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BackupError {
    NotFound,
    /// Kopien er ødelagt (sjekksum, størrelse eller integritet).
    Corrupt,
    /// Kopien er laget av en nyere versjon enn denne kjernen kjenner.
    SchemaTooNew,
    Failed(BackupCause),
}

impl BackupError {
    /// Stabil feilkode i `ROBUST_ERROR_CODES`.
    pub fn code(&self) -> &'static str {
        match self {
            Self::NotFound => "backup_not_found",
            Self::Corrupt | Self::SchemaTooNew => "backup_corrupt",
            Self::Failed(_) => "backup_failed",
        }
    }

    /// Finere årsak (`disk_full`, `write_failed`, `locked`, ...), til diagnose.
    pub fn cause(&self) -> &'static str {
        match self {
            Self::NotFound => "not_found",
            Self::Corrupt => "corrupt",
            Self::SchemaTooNew => "schema_too_new",
            Self::Failed(c) => c.as_str(),
        }
    }

    fn interrupted() -> BackupError {
        BackupError::Failed(BackupCause::Interrupted)
    }
}

/// Årsak ut fra en I/O-feil (full disk, skrivefeil, låst fil).
pub fn cause_of_io(e: &io::Error) -> BackupCause {
    use io::ErrorKind as K;
    match e.kind() {
        K::StorageFull | K::QuotaExceeded => return BackupCause::DiskFull,
        K::ResourceBusy => return BackupCause::Locked,
        _ => {}
    }
    match e.raw_os_error() {
        // ENOSPC, ERROR_HANDLE_DISK_FULL, ERROR_DISK_FULL
        Some(28) | Some(39) | Some(112) => BackupCause::DiskFull,
        // ERROR_SHARING_VIOLATION, ERROR_LOCK_VIOLATION
        Some(32) | Some(33) => BackupCause::Locked,
        _ => BackupCause::WriteFailed,
    }
}

/// Årsak ut fra en SQLite-feil.
pub fn cause_of_sqlite(e: &rusqlite::Error) -> BackupCause {
    match e {
        rusqlite::Error::SqliteFailure(f, _) => match f.code {
            ErrorCode::DiskFull => BackupCause::DiskFull,
            ErrorCode::DatabaseBusy | ErrorCode::DatabaseLocked => BackupCause::Locked,
            ErrorCode::DatabaseCorrupt | ErrorCode::NotADatabase => BackupCause::SourceCorrupt,
            ErrorCode::ReadOnly | ErrorCode::CannotOpen | ErrorCode::SystemIoFailure => {
                BackupCause::WriteFailed
            }
            _ => BackupCause::Io,
        },
        _ => BackupCause::Io,
    }
}

fn io_err(e: io::Error) -> BackupError {
    BackupError::Failed(cause_of_io(&e))
}

fn sql_err(e: rusqlite::Error) -> BackupError {
    BackupError::Failed(cause_of_sqlite(&e))
}

fn store_err(e: StoreError) -> BackupError {
    match e {
        StoreError::TooNew { .. } => BackupError::SchemaTooNew,
        StoreError::Sqlite(e) => sql_err(e),
        _ => BackupError::Failed(BackupCause::Io),
    }
}

/// Resultat av en integritetssjekk av en indeksfil, med detaljen `IntegrityStatus` ikke bærer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IndexCheck {
    Ok,
    /// Eldre skjema enn denne kjernen: kan brukes, og migreres ved åpning.
    SchemaOld(u32),
    /// Skjemaet er nyere enn denne kjernen kjenner (laget av en nyere versjon).
    SchemaTooNew(u32),
    /// Leses, men har avvik (fremmednøkler).
    Degraded,
    Corrupt,
    /// Kan ikke åpnes (mangler, låst, ingen tilgang).
    Unreadable,
}

impl IndexCheck {
    pub fn status(self) -> IntegrityStatus {
        match self {
            Self::Ok | Self::SchemaOld(_) => IntegrityStatus::Ok,
            Self::Degraded => IntegrityStatus::Degraded,
            Self::Corrupt => IntegrityStatus::Corrupt,
            Self::SchemaTooNew(_) | Self::Unreadable => IntegrityStatus::Unreadable,
        }
    }

    /// Detaljkode: `ok`, `schema_old`, `schema_too_new`, `degraded`, `corrupt`, `unreadable`.
    pub fn code(self) -> &'static str {
        match self {
            Self::Ok => "ok",
            Self::SchemaOld(_) => "schema_old",
            Self::SchemaTooNew(_) => "schema_too_new",
            Self::Degraded => "degraded",
            Self::Corrupt => "corrupt",
            Self::Unreadable => "unreadable",
        }
    }

    /// Kan indeksen åpnes og brukes (evt. etter migrering) uten tap?
    pub fn usable(self) -> bool {
        matches!(self, Self::Ok | Self::SchemaOld(_))
    }
}

/// Beskrivelse av én kopi (innholdet i `.meta`, pluss kopiens egen status når den listes).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BackupMeta {
    pub id: BackupId,
    pub kind: BackupKind,
    pub created_at: i64,
    pub bytes: u64,
    pub asset_count: u64,
    pub schema: u32,
    pub app_version: String,
    /// blake3 (hex) av kopifilen.
    pub checksum: String,
    /// Hvorfor kopien ble tatt: `manual`, `weekly`, `before_migration`, `before_purge`, `pre_restore`.
    pub reason: String,
    /// Fra størrelsessjekk ved listing; full sjekk (sjekksum og integritet) skjer ved gjenoppretting.
    pub integrity: IntegrityStatus,
}

/// En opprettet kopi og hvilke eldre automatiske kopier som ble rullet bort.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BackupCreated {
    pub meta: BackupMeta,
    /// Ids til automatiske kopier som ble fjernet av rulleringen (synlig for kalleren, ingen stille tømming).
    pub pruned: Vec<BackupId>,
}

/// Resultat av en gjenoppretting.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Restored {
    pub restored: BackupMeta,
    pub safety: BackupMeta,
}

/// Trinn i gjenopprettingen (feilinjeksjon i tester).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RestoreStep {
    Copied,
    LiveClosed,
    OldMoved,
    NewMoved,
    Reopened,
}

/// Feilinjeksjon: stopp etter `step`. `crash` etterlater filene som de er (prosessen døde), ellers ryddes og
/// rulles det tilbake som ved en vanlig feil.
#[derive(Clone, Copy, Debug)]
pub struct Fault {
    pub step: RestoreStep,
    pub crash: bool,
}

/// Hva `recover_interrupted_restore` gjorde.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Recovered {
    Nothing,
    /// Den gamle indeksen ble satt tilbake (byttet var ikke fullført).
    RolledBack,
    /// Byttet var fullført; rester ryddet.
    CleanedUp,
}

impl Recovered {
    /// Stabil kode: `none`, `rolled_back`, `cleaned_up`.
    pub fn code(self) -> &'static str {
        match self {
            Self::Nothing => "none",
            Self::RolledBack => "rolled_back",
            Self::CleanedUp => "cleaned_up",
        }
    }
}

// ---- filnavn ----

fn with_suffix(p: &Path, suffix: &str) -> PathBuf {
    let mut s = p.as_os_str().to_os_string();
    s.push(suffix);
    PathBuf::from(s)
}

/// Mappen kopiene til indeksfilen `db` ligger i: `<datamappe>/backups/<indeksfilnavn>/`. Aldri en bildemappe.
fn backup_dir(db: &Path) -> PathBuf {
    let name = db.file_name().map(|n| n.to_owned()).unwrap_or_default();
    db.parent()
        .unwrap_or_else(|| Path::new("."))
        .join("backups")
        .join(name)
}

fn db_file(dir: &Path, id: BackupId) -> PathBuf {
    dir.join(format!("backup-{:06}.db", id.0))
}

fn meta_file(dir: &Path, id: BackupId) -> PathBuf {
    dir.join(format!("backup-{:06}.meta", id.0))
}

fn parse_id(name: &str) -> Option<u64> {
    name.strip_prefix("backup-")?
        .split('.')
        .next()?
        .parse()
        .ok()
}

/// Høyeste id i mappen, både `.db`, `.meta` og `.tmp` (en halvferdig kopi gjenbruker aldri en id).
fn max_id(dir: &Path) -> u64 {
    fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| parse_id(&e.file_name().to_string_lossy()))
        .max()
        .unwrap_or(0)
}

fn hash_file(p: &Path) -> io::Result<String> {
    let mut f = File::open(p)?;
    let mut h = blake3::Hasher::new();
    let mut buf = vec![0u8; 64 * 1024];
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    Ok(h.finalize().to_hex().to_string())
}

fn write_meta(path: &Path, m: &BackupMeta) -> io::Result<()> {
    let text = format!(
        "format={BACKUP_FORMAT}\nid={}\nkind={}\ncreated_at={}\nbytes={}\nassets={}\nschema={}\napp_version={}\nchecksum={}\nreason={}\ncontent=index_only\n",
        m.id.0,
        m.kind.as_str(),
        m.created_at,
        m.bytes,
        m.asset_count,
        m.schema,
        m.app_version,
        m.checksum,
        m.reason
    );
    let tmp = with_suffix(path, ".tmp");
    fs::write(&tmp, text)?;
    File::open(&tmp)?.sync_all()?;
    fs::rename(&tmp, path)
}

fn read_meta(path: &Path) -> Option<BackupMeta> {
    let text = fs::read_to_string(path).ok()?;
    let get = |k: &str| {
        text.lines()
            .find_map(|l| l.strip_prefix(k)?.strip_prefix('='))
    };
    if get("format")?.parse::<u32>().ok()? > BACKUP_FORMAT || get("content")? != "index_only" {
        return None;
    }
    Some(BackupMeta {
        id: BackupId(get("id")?.parse().ok()?),
        kind: BackupKind::parse(get("kind")?)?,
        created_at: get("created_at")?.parse().ok()?,
        bytes: get("bytes")?.parse().ok()?,
        asset_count: get("assets")?.parse().ok()?,
        schema: get("schema")?.parse().ok()?,
        app_version: get("app_version")?.to_string(),
        checksum: get("checksum")?.to_string(),
        reason: get("reason")?.to_string(),
        integrity: IntegrityStatus::Ok,
    })
}

// ---- integritet ----

fn open_ro(path: &Path) -> Result<Connection, rusqlite::Error> {
    let flags = OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX;
    let c = Connection::open_with_flags(path, flags)?;
    c.busy_timeout(Duration::from_millis(BUSY_MS))?;
    Ok(c)
}

fn check_err(e: &rusqlite::Error) -> IndexCheck {
    match cause_of_sqlite(e) {
        BackupCause::SourceCorrupt => IndexCheck::Corrupt,
        _ => IndexCheck::Unreadable,
    }
}

fn check_conn(c: &Connection, full: bool) -> IndexCheck {
    let version: u32 = match c.pragma_query_value(None, "user_version", |r| r.get(0)) {
        Ok(v) => v,
        Err(e) => return check_err(&e),
    };
    let supported = MIGRATIONS.len() as u32;
    if version > supported {
        return IndexCheck::SchemaTooNew(version);
    }
    let pragma = if full {
        "PRAGMA integrity_check"
    } else {
        "PRAGMA quick_check"
    };
    let rows: Result<Vec<String>, rusqlite::Error> = c.prepare(pragma).and_then(|mut st| {
        let it = st.query_map([], |r| r.get::<_, String>(0))?;
        it.collect()
    });
    match rows {
        Ok(r) if r == ["ok"] => {}
        Ok(_) => return IndexCheck::Corrupt,
        Err(e) => return check_err(&e),
    }
    if full {
        match c.query_row("SELECT COUNT(*) FROM pragma_foreign_key_check", [], |r| {
            r.get::<_, i64>(0)
        }) {
            Ok(0) => {}
            Ok(_) => return IndexCheck::Degraded,
            Err(e) => return check_err(&e),
        }
    }
    if version < supported {
        IndexCheck::SchemaOld(version)
    } else {
        IndexCheck::Ok
    }
}

/// Antall bilder i indeksen (aktive og i «Nylig slettet»).
fn count_assets(c: &Connection) -> u64 {
    c.query_row("SELECT COUNT(*) FROM asset WHERE state < 2", [], |r| {
        r.get::<_, i64>(0)
    })
    .unwrap_or(0) as u64
}

impl Store {
    /// Indeksfilen (`None` for minnedatabase).
    pub fn db_path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    /// Sjekker en indeksfil uten å åpne den som `Store` (egen skrivebeskyttet tilkobling, blokkerer ikke
    /// og blokkeres ikke av skriveren). `full = false` er `quick_check` (oppstart), `true` er full
    /// `integrity_check` og `foreign_key_check`. En tom fil regnes som ødelagt: en avkortet indeks skal aldri
    /// stilltiende bli en ny, tom en.
    pub fn check_file(path: &Path, full: bool) -> IndexCheck {
        match fs::metadata(path) {
            Err(_) => return IndexCheck::Unreadable,
            Ok(m) if m.len() == 0 => return IndexCheck::Corrupt,
            Ok(_) => {}
        }
        match open_ro(path) {
            Ok(c) => check_conn(&c, full),
            Err(e) => check_err(&e),
        }
    }

    /// Kopiene til indeksen `db`, nyeste først. Bare metadata og en størrelsessjekk (rask); sjekksum og
    /// integritet kontrolleres når en kopi skal brukes. En kopi med ødelagt `.meta` listes som `Corrupt`.
    pub fn backup_list_at(db: &Path) -> Vec<BackupMeta> {
        let dir = backup_dir(db);
        let mut out = Vec::new();
        let Ok(rd) = fs::read_dir(&dir) else {
            return out;
        };
        for e in rd.flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            let Some(stem) = name.strip_suffix(".meta") else {
                continue;
            };
            let Some(id) = parse_id(stem) else { continue };
            let id = BackupId(id);
            let mut m = read_meta(&e.path()).unwrap_or(BackupMeta {
                id,
                kind: BackupKind::Manual,
                created_at: 0,
                bytes: 0,
                asset_count: 0,
                schema: 0,
                app_version: String::new(),
                checksum: String::new(),
                reason: String::new(),
                integrity: IntegrityStatus::Corrupt,
            });
            if m.integrity.is_ok() {
                m.integrity = match fs::metadata(db_file(&dir, id)) {
                    Ok(f) if f.len() == m.bytes => {
                        if m.schema > MIGRATIONS.len() as u32 {
                            IntegrityStatus::Unreadable
                        } else {
                            IntegrityStatus::Ok
                        }
                    }
                    _ => IntegrityStatus::Corrupt,
                };
            }
            out.push(m);
        }
        out.sort_by_key(|m| std::cmp::Reverse(m.id));
        out
    }

    /// Full kontroll av én kopi: `.meta`, størrelse, sjekksum, integritet og skjema.
    pub fn backup_verify_at(db: &Path, id: BackupId) -> Result<BackupMeta, BackupError> {
        let dir = backup_dir(db);
        let mp = meta_file(&dir, id);
        if !mp.exists() {
            return Err(BackupError::NotFound);
        }
        let mut m = read_meta(&mp).ok_or(BackupError::Corrupt)?;
        let file = db_file(&dir, id);
        let len = fs::metadata(&file).map_err(|_| BackupError::Corrupt)?.len();
        if len != m.bytes || hash_file(&file).map_err(|_| BackupError::Corrupt)? != m.checksum {
            return Err(BackupError::Corrupt);
        }
        match Store::check_file(&file, true) {
            c if c.usable() => {}
            IndexCheck::SchemaTooNew(_) => return Err(BackupError::SchemaTooNew),
            _ => return Err(BackupError::Corrupt),
        }
        m.integrity = IntegrityStatus::Ok;
        Ok(m)
    }

    /// Nyeste kopi (unntatt `PreRestore`) som består full kontroll. Brukes av trygg modus.
    pub fn latest_good_backup_at(db: &Path) -> Option<BackupMeta> {
        Store::backup_list_at(db)
            .into_iter()
            .filter(|m| m.kind != BackupKind::PreRestore)
            .find_map(|m| Store::backup_verify_at(db, m.id).ok())
    }

    /// Antall automatiske kopier som beholdes (setting `backup_keep_auto`, 1..=50, standard 5).
    pub fn backup_keep_auto(&self) -> usize {
        self.setting(KEEP_AUTO_KEY)
            .ok()
            .flatten()
            .and_then(|v| v.parse::<usize>().ok())
            .map_or(DEFAULT_KEEP_AUTO, |n| n.clamp(1, MAX_KEEP_AUTO))
    }

    /// Tar en kopi av indeksen `db` til appens egen mappe (`backups/` ved siden av indeksen). Går via en egen
    /// skrivebeskyttet tilkobling og tar ingen lås på skriveren, så den kan kjøres fra en bakgrunnstråd mens
    /// appen er i bruk. Kopien kontrolleres før den får `.meta`; en ødelagt kilde gir `Failed(SourceCorrupt)`
    /// og ingenting rulleres bort. Bare `Auto`-kopier ruller (de `keep_auto` nyeste beholdes).
    pub fn backup_create_at(
        db: &Path,
        kind: BackupKind,
        reason: &str,
        now: i64,
        keep_auto: usize,
    ) -> Result<BackupCreated, BackupError> {
        if !db.exists() {
            return Err(BackupError::Failed(BackupCause::NoFile));
        }
        let dir = backup_dir(db);
        fs::create_dir_all(&dir).map_err(io_err)?;
        // Rydd rester av avbrutte kopier (aldri ferdige: de har ikke `.meta`).
        for e in fs::read_dir(&dir).map_err(io_err)?.flatten() {
            if e.file_name().to_string_lossy().ends_with(".tmp") {
                let _ = fs::remove_file(e.path());
            }
        }
        let id = BackupId(max_id(&dir) + 1);
        let fin = db_file(&dir, id);
        let tmp = with_suffix(&fin, ".tmp");
        let (bytes, checksum, schema, assets) = match Self::write_copy(db, &tmp) {
            Ok(v) => v,
            Err(e) => {
                let _ = fs::remove_file(&tmp);
                return Err(e);
            }
        };
        let meta = BackupMeta {
            id,
            kind,
            created_at: now,
            bytes,
            asset_count: assets,
            schema,
            app_version: env!("CARGO_PKG_VERSION").to_string(),
            checksum,
            reason: reason.to_string(),
            integrity: IntegrityStatus::Ok,
        };
        let commit = fs::rename(&tmp, &fin)
            .and_then(|_| write_meta(&meta_file(&dir, id), &meta))
            .map_err(io_err);
        if let Err(e) = commit {
            let _ = fs::remove_file(&tmp);
            let _ = fs::remove_file(&fin);
            let _ = fs::remove_file(with_suffix(&meta_file(&dir, id), ".tmp"));
            return Err(e);
        }
        let pruned = if kind == BackupKind::Auto {
            Self::prune_auto_at(db, keep_auto.clamp(1, MAX_KEEP_AUTO))
        } else {
            Vec::new()
        };
        Ok(BackupCreated { meta, pruned })
    }

    /// Skriver og kontrollerer kopien i `tmp`. Gir (bytes, sjekksum, skjemaversjon, antall bilder).
    fn write_copy(db: &Path, tmp: &Path) -> Result<(u64, String, u32, u64), BackupError> {
        {
            let src = open_ro(db).map_err(sql_err)?;
            let t = tmp.to_str().ok_or(BackupError::Failed(BackupCause::Io))?;
            src.execute("VACUUM INTO ?1", [t]).map_err(sql_err)?;
        }
        // Kopien skal være en vanlig fil uten sidefiler: ut av WAL-modus (header), så sjekksummen er stabil.
        let copy = Connection::open(tmp).map_err(sql_err)?;
        copy.query_row("PRAGMA journal_mode = DELETE", [], |_| Ok(()))
            .map_err(sql_err)?;
        let check = check_conn(&copy, true);
        let schema = copy
            .pragma_query_value(None, "user_version", |r| r.get::<_, u32>(0))
            .unwrap_or(0);
        let assets = count_assets(&copy);
        drop(copy);
        match check {
            c if c.usable() => {}
            IndexCheck::SchemaTooNew(_) => return Err(BackupError::SchemaTooNew),
            _ => return Err(BackupError::Failed(BackupCause::SourceCorrupt)),
        }
        File::open(tmp).and_then(|f| f.sync_all()).map_err(io_err)?;
        let bytes = fs::metadata(tmp).map_err(io_err)?.len();
        let checksum = hash_file(tmp).map_err(io_err)?;
        Ok((bytes, checksum, schema, assets))
    }

    /// Fjerner automatiske kopier utover de `keep` nyeste. Gir ids som ble fjernet (meta først, så fila).
    fn prune_auto_at(db: &Path, keep: usize) -> Vec<BackupId> {
        let dir = backup_dir(db);
        let autos: Vec<BackupId> = Store::backup_list_at(db)
            .into_iter()
            .filter(|m| m.kind == BackupKind::Auto)
            .map(|m| m.id)
            .collect();
        let mut pruned = Vec::new();
        for id in autos.into_iter().skip(keep) {
            if fs::remove_file(meta_file(&dir, id)).is_ok() {
                let _ = fs::remove_file(db_file(&dir, id));
                pruned.push(id);
            }
        }
        pruned
    }

    /// Før migrering: tar en `Auto`-kopi (`before_migration`) hvis indeksen finnes og har eldre skjema enn
    /// kjernen. Åpner aldri indeksen som `Store` (ingen migrering skjer her). `None` når ingenting trengs.
    pub fn backup_before_migration_at(
        db: &Path,
        now: i64,
    ) -> Result<Option<BackupCreated>, BackupError> {
        match Store::check_file(db, false) {
            IndexCheck::SchemaOld(v) if v > 0 => {
                let kind = BackupKind::Auto;
                Store::backup_create_at(db, kind, "before_migration", now, DEFAULT_KEEP_AUTO)
                    .map(Some)
            }
            _ => Ok(None),
        }
    }

    // ---- gjenoppretting ----

    /// Gjenoppretter kopi `id` i den åpne indeksen. Se modulomtalen: `PreRestore`-kopi først, ny indeks
    /// kopieres til siden og verifiseres, den gamle flyttes til siden, den nye åpnes (migreres), og først da
    /// ryddes den gamle bort (kopien fra før finnes i kopilisten). Feil gir tilbakerulling til den gamle.
    pub fn restore_backup(&mut self, id: BackupId, now: i64) -> Result<Restored, BackupError> {
        self.restore_backup_with(id, now, None)
    }

    pub(crate) fn restore_backup_with(
        &mut self,
        id: BackupId,
        now: i64,
        fault: Option<Fault>,
    ) -> Result<Restored, BackupError> {
        let db = self
            .path
            .clone()
            .ok_or(BackupError::Failed(BackupCause::NoFile))?;
        let meta = Store::backup_verify_at(&db, id)?;
        // Den gamle indeksen sikres først. Går dette galt, er ingenting endret.
        let kind = BackupKind::PreRestore;
        let safety = Store::backup_create_at(&db, kind, "pre_restore", now, 0)?.meta;
        let tmp = with_suffix(&db, ".restoring");
        let before = with_suffix(&db, ".before-restore");
        let hit = |step: RestoreStep| fault.is_some_and(|f| f.step == step);
        let crash = fault.is_some_and(|f| f.crash);
        let _ = fs::remove_file(&tmp);

        let staged = stage_copy(&db_file(&backup_dir(&db), id), &tmp, &meta).and_then(|_| {
            if hit(RestoreStep::Copied) {
                Err(BackupError::interrupted())
            } else {
                Ok(())
            }
        });
        if let Err(e) = staged {
            if !(hit(RestoreStep::Copied) && crash) {
                let _ = fs::remove_file(&tmp);
            }
            return Err(e);
        }
        // Lukk skrivetilkoblingen (checkpoint først, så WAL er tom) før filene byttes.
        if let Err(e) = self
            .conn
            .query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |_| Ok(()))
        {
            let _ = fs::remove_file(&tmp);
            return Err(sql_err(e));
        }
        let placeholder = Connection::open_in_memory().map_err(sql_err)?;
        drop(std::mem::replace(&mut self.conn, placeholder));

        // Hvert trinn gir feil hvis feilinjeksjon treffer det; `crashed` betyr at filene skal bli liggende.
        let mut crashed = false;
        let mut step = |s: RestoreStep| -> Result<(), BackupError> {
            if hit(s) {
                crashed = crash;
                Err(BackupError::interrupted())
            } else {
                Ok(())
            }
        };
        let swapped = step(RestoreStep::LiveClosed)
            .and_then(|_| fs::rename(&db, &before).map_err(io_err))
            .and_then(|_| step(RestoreStep::OldMoved))
            .and_then(|_| {
                remove_sidecars(&db);
                fs::rename(&tmp, &db).map_err(io_err)
            })
            .and_then(|_| step(RestoreStep::NewMoved))
            .and_then(|_| Store::open(&db).map_err(store_err))
            .and_then(|s| step(RestoreStep::Reopened).map(|_| s));
        match swapped {
            Ok(new) => {
                let generation = self.generation + 1;
                *self = new;
                self.generation = generation;
                let _ = fs::remove_file(&before);
                Ok(Restored {
                    restored: meta,
                    safety,
                })
            }
            Err(e) => {
                if !crashed {
                    // Rull tilbake: den gamle indeksen settes på plass og åpnes igjen.
                    let _ = fs::remove_file(&tmp);
                    if before.exists() {
                        remove_sidecars(&db);
                        let _ = fs::remove_file(&db);
                        let _ = fs::rename(&before, &db);
                    }
                    if let Ok(old) = Store::open(&db) {
                        let generation = self.generation;
                        *self = old;
                        self.generation = generation;
                    }
                }
                Err(e)
            }
        }
    }

    /// Gjenoppretter kopi `id` når indeksen `db` ikke er åpen (trygg modus ved oppstart, eller angre). Den
    /// gjeldende filen (ødelagt eller ei) flyttes til `<db>.corrupt-<tid>` og slettes aldri. Den nye
    /// verifiseres og åpnes (migreres) før den regnes som på plass; feil setter den gamle tilbake.
    pub fn restore_closed_at(db: &Path, id: BackupId, now: i64) -> Result<BackupMeta, BackupError> {
        let meta = Store::backup_verify_at(db, id)?;
        let tmp = with_suffix(db, ".restoring");
        let _ = fs::remove_file(&tmp);
        if let Err(e) = stage_copy(&db_file(&backup_dir(db), id), &tmp, &meta) {
            let _ = fs::remove_file(&tmp);
            return Err(e);
        }
        let had = db.exists();
        let aside = if had {
            match Store::quarantine_corrupt_index(db, now) {
                Ok(a) => Some(a),
                Err(e) => {
                    let _ = fs::remove_file(&tmp);
                    return Err(e);
                }
            }
        } else {
            None
        };
        let placed = fs::rename(&tmp, db)
            .map_err(io_err)
            .and_then(|_| Store::open(db).map(drop).map_err(store_err));
        if let Err(e) = placed {
            let _ = fs::remove_file(&tmp);
            remove_sidecars(db);
            let _ = fs::remove_file(db);
            if let Some(aside) = aside {
                let _ = fs::rename(&aside, db);
                for s in ["-wal", "-shm"] {
                    let from = with_suffix(&aside, s);
                    if from.exists() {
                        let _ = fs::rename(&from, with_suffix(db, s));
                    }
                }
            }
            return Err(e);
        }
        Ok(meta)
    }

    /// Flytter en ødelagt indeks til siden (`<db>.corrupt-<tid>`, med sidefiler) slik at neste åpning lager en
    /// ny, tom en («bygg på nytt»). Filen slettes aldri. Gir stien den ble flyttet til.
    pub fn quarantine_corrupt_index(db: &Path, now: i64) -> Result<PathBuf, BackupError> {
        let aside = unique_aside(db, now);
        fs::rename(db, &aside).map_err(io_err)?;
        for suffix in ["-wal", "-shm"] {
            let from = with_suffix(db, suffix);
            if from.exists() {
                let _ = fs::rename(&from, with_suffix(&aside, suffix));
            }
        }
        Ok(aside)
    }

    /// Rydder etter en gjenoppretting som ble avbrutt av krasj. Kalles før indeksen åpnes ved oppstart.
    /// - indeksen mangler, `.before-restore` finnes: den gamle settes tilbake;
    /// - indeksen finnes, `.before-restore` finnes: er den nye hel, ryddes den gamle bort (kopien fra før ligger
    ///   i kopilisten); er den ikke hel, flyttes den ødelagte til siden og den gamle settes tilbake;
    /// - `.restoring` uten fullført bytte slettes (det er en ufullstendig kopi).
    pub fn recover_interrupted_restore(db: &Path, now: i64) -> Recovered {
        let tmp = with_suffix(db, ".restoring");
        let before = with_suffix(db, ".before-restore");
        let mut out = Recovered::Nothing;
        if !db.exists() && before.exists() {
            if fs::rename(&before, db).is_ok() {
                out = Recovered::RolledBack;
            }
        } else if db.exists() && before.exists() {
            if Store::check_file(db, false).usable() {
                let _ = fs::remove_file(&before);
                out = Recovered::CleanedUp;
            } else if Store::quarantine_corrupt_index(db, now).is_ok()
                && fs::rename(&before, db).is_ok()
            {
                out = Recovered::RolledBack;
            }
        }
        if tmp.exists() {
            let _ = fs::remove_file(&tmp);
            if out == Recovered::Nothing {
                out = Recovered::CleanedUp;
            }
        }
        out
    }
}

fn remove_sidecars(db: &Path) {
    for s in ["-wal", "-shm"] {
        let _ = fs::remove_file(with_suffix(db, s));
    }
}

fn unique_aside(db: &Path, now: i64) -> PathBuf {
    let base = with_suffix(db, &format!(".corrupt-{now}"));
    if !base.exists() {
        return base;
    }
    (1..)
        .map(|n| with_suffix(&base, &format!("-{n}")))
        .find(|p| !p.exists())
        .unwrap_or(base)
}

/// Kopierer en kopi til `tmp` (ved siden av indeksen, samme volum) og kontrollerer at kopien er lik og hel.
fn stage_copy(src: &Path, tmp: &Path, meta: &BackupMeta) -> Result<(), BackupError> {
    fs::copy(src, tmp).map_err(io_err)?;
    File::open(tmp).and_then(|f| f.sync_all()).map_err(io_err)?;
    if hash_file(tmp).map_err(io_err)? != meta.checksum {
        return Err(BackupError::Corrupt);
    }
    if !Store::check_file(tmp, false).usable() {
        return Err(BackupError::Corrupt);
    }
    Ok(())
}

#[cfg(test)]
mod tests;
