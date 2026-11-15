//! Tester for sikkerhetskopi, integritet og gjenoppretting (docs/v1/BACKUP.md). Kjørt på Linux.
//! Dekker: konsistent kopi av åpen WAL-database, rulling, korrupt DB (avkortet, ødelagt side, tom, bit-flipp),
//! angrebar gjenoppretting, feil og simulert krasj midt i gjenopprettingen, og feilkoder uten delvis skriving.

use super::*;
use crate::model::{Album, AssetMeta, Kind, ProtectFlags, SourceId};
use std::sync::atomic::{AtomicU32, Ordering};

struct Dir(PathBuf);
impl Dir {
    fn new(tag: &str) -> Dir {
        static N: AtomicU32 = AtomicU32::new(0);
        let p = std::env::temp_dir().join(format!(
            "fjell-backup-{tag}-{}-{}",
            std::process::id(),
            N.fetch_add(1, Ordering::SeqCst)
        ));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(&p).unwrap();
        Dir(p)
    }
    fn db(&self) -> PathBuf {
        self.0.join("index.sqlite")
    }
}
impl Drop for Dir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

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
        protect_flags: ProtectFlags::default(),
        albums: vec![Album {
            id: 7,
            name: "Ferie med personnavn".into(),
            shared: false,
        }],
        backup: None,
    }
}

fn seed(s: &mut Store, from: usize, n: usize) {
    let ms: Vec<_> = (from..from + n)
        .map(|i| meta(&format!("f{i}.jpg")))
        .collect();
    s.upsert_assets(&ms).unwrap();
}

fn count(s: &Store) -> u64 {
    count_assets(s.read_conn())
}

fn open_with(d: &Dir, n: usize) -> Store {
    let mut s = Store::open(&d.db()).unwrap();
    seed(&mut s, 0, n);
    s
}

fn manual(db: &Path, now: i64) -> BackupMeta {
    Store::backup_create_at(db, BackupKind::Manual, "manual", now, 5)
        .unwrap()
        .meta
}

/// Ødelegger sidene midt i filen (samme lengde): en ødelagt side, ikke en avkortet fil.
fn damage_middle(p: &Path) {
    let mut b = fs::read(p).unwrap();
    let from = b.len() / 3;
    let to = (b.len() * 2 / 3).max(from + 4096).min(b.len());
    for (i, x) in b[from..to].iter_mut().enumerate() {
        *x = (i as u8).wrapping_mul(31).wrapping_add(7);
    }
    fs::write(p, b).unwrap();
}

fn big_db(d: &Dir) {
    let mut s = Store::open(&d.db()).unwrap();
    seed(&mut s, 0, 3000);
    s.conn()
        .query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |_| Ok(()))
        .unwrap();
}

fn kinds(db: &Path) -> Vec<BackupKind> {
    Store::backup_list_at(db).iter().map(|m| m.kind).collect()
}

fn leftovers(d: &Dir) -> Vec<String> {
    let mut v: Vec<String> = fs::read_dir(&d.0)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|n| n.contains("restoring") || n.contains("before-restore"))
        .collect();
    v.sort();
    v
}

// ---- opprettelse ----

#[test]
fn copy_of_open_wal_database_is_consistent_and_labelled_index_only() {
    let d = Dir::new("create");
    let mut s = open_with(&d, 300);
    // Skriveren er åpen og har ukheckpointede sider i WAL.
    assert!(d.0.join("index.sqlite-wal").exists());
    let m = manual(&d.db(), 1_700_000_000);
    assert_eq!(
        (m.kind, m.asset_count, m.schema),
        (BackupKind::Manual, 300, 5)
    );
    assert_eq!(m.integrity, IntegrityStatus::Ok);
    let dir = backup_dir(&d.db());
    let copy = db_file(&dir, m.id);
    assert_eq!(Store::check_file(&copy, true), IndexCheck::Ok);
    let c = Connection::open(&copy).unwrap();
    assert_eq!(count_assets(&c), 300);
    drop(c);
    // Ingen sidefiler, riktig sjekksum og merking; alt ligger under datamappen, ikke i en bildemappe.
    let names: Vec<String> = fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(names.len(), 2, "{names:?}");
    assert_eq!(hash_file(&copy).unwrap(), m.checksum);
    let text = fs::read_to_string(meta_file(&dir, m.id)).unwrap();
    assert!(text.contains("content=index_only") && text.contains("schema=5"));
    assert!(dir.starts_with(&d.0));
    // Skriveren blokkeres ikke av kopien.
    seed(&mut s, 300, 10);
    assert_eq!(count(&s), 310);
}

#[test]
fn rolling_keeps_last_n_auto_and_never_touches_manual() {
    let d = Dir::new("roll");
    let _s = open_with(&d, 3);
    let man = manual(&d.db(), 1);
    let mut pruned_total = Vec::new();
    for i in 0..5 {
        let c = Store::backup_create_at(&d.db(), BackupKind::Auto, "weekly", 10 + i, 2).unwrap();
        pruned_total.extend(c.pruned);
    }
    let list = Store::backup_list_at(&d.db());
    let autos = list.iter().filter(|m| m.kind == BackupKind::Auto).count();
    assert_eq!(autos, 2);
    assert_eq!(
        pruned_total.len(),
        3,
        "rulleringen rapporterer hva som ble fjernet"
    );
    assert!(
        list.iter().any(|m| m.id == man.id),
        "manuell kopi røres aldri"
    );
    assert!(list.windows(2).all(|w| w[0].id > w[1].id), "nyeste først");
    let dir = backup_dir(&d.db());
    for id in pruned_total {
        assert!(!db_file(&dir, id).exists() && !meta_file(&dir, id).exists());
    }
}

#[test]
fn keep_setting_is_clamped_and_defaults() {
    let mut s = Store::open_in_memory().unwrap();
    assert_eq!(s.backup_keep_auto(), DEFAULT_KEEP_AUTO);
    s.set_setting(KEEP_AUTO_KEY, "9").unwrap();
    assert_eq!(s.backup_keep_auto(), 9);
    s.set_setting(KEEP_AUTO_KEY, "0").unwrap();
    assert_eq!(s.backup_keep_auto(), 1);
    s.set_setting(KEEP_AUTO_KEY, "100000").unwrap();
    assert_eq!(s.backup_keep_auto(), 50);
    s.set_setting(KEEP_AUTO_KEY, "abc").unwrap();
    assert_eq!(s.backup_keep_auto(), DEFAULT_KEEP_AUTO);
}

#[test]
fn ids_are_never_reused_and_stale_tmp_files_are_cleaned() {
    let d = Dir::new("ids");
    let _s = open_with(&d, 2);
    let a = manual(&d.db(), 1);
    let dir = backup_dir(&d.db());
    fs::write(dir.join("backup-000009.db.tmp"), b"halv").unwrap();
    fs::write(db_file(&dir, BackupId(7)), b"foreldreloes fil uten meta").unwrap();
    let b = manual(&d.db(), 2);
    assert!(
        b.id > a.id && b.id.0 > 7,
        "foreldreløs fil gjenbrukes ikke: {:?}",
        b.id
    );
    assert!(!dir.join("backup-000009.db.tmp").exists());
    // En fil uten `.meta` er ikke en kopi.
    assert_eq!(Store::backup_list_at(&d.db()).len(), 2);
}

#[test]
fn backup_of_corrupt_source_fails_and_keeps_older_copies() {
    let d = Dir::new("src-corrupt");
    let good;
    {
        let _s = open_with(&d, 50);
        good = manual(&d.db(), 1);
    }
    damage_middle(&d.db());
    let e = Store::backup_create_at(&d.db(), BackupKind::Auto, "weekly", 2, 1).unwrap_err();
    assert_eq!(e.code(), "backup_failed");
    assert!(
        matches!(e.cause(), "source_corrupt" | "write_failed" | "io"),
        "{}",
        e.cause()
    );
    let list = Store::backup_list_at(&d.db());
    assert_eq!(list.len(), 1, "ingen ny kopi og ingen rulling");
    assert_eq!(list[0].id, good.id);
    let dir = backup_dir(&d.db());
    assert!(!fs::read_dir(&dir).unwrap().any(|e| e
        .unwrap()
        .file_name()
        .to_string_lossy()
        .ends_with(".tmp")));
}

#[test]
fn write_failure_gives_code_and_leaves_nothing_behind() {
    let d = Dir::new("write-fail");
    let _s = open_with(&d, 2);
    // `backups` er en fil, så mappen kan ikke lages.
    fs::write(d.0.join("backups"), b"i veien").unwrap();
    let e = Store::backup_create_at(&d.db(), BackupKind::Manual, "manual", 1, 5).unwrap_err();
    assert_eq!((e.code(), e.cause()), ("backup_failed", "write_failed"));
    let e = Store::backup_create_at(
        &d.0.join("finnes-ikke.sqlite"),
        BackupKind::Manual,
        "m",
        1,
        5,
    )
    .unwrap_err();
    assert_eq!(e.cause(), "no_file");
}

#[test]
fn io_and_sqlite_errors_map_to_stable_causes() {
    let kind = |k| io::Error::from(k);
    assert_eq!(
        cause_of_io(&kind(io::ErrorKind::StorageFull)),
        BackupCause::DiskFull
    );
    for raw in [28, 39, 112] {
        assert_eq!(
            cause_of_io(&io::Error::from_raw_os_error(raw)),
            BackupCause::DiskFull
        );
    }
    for raw in [32, 33] {
        assert_eq!(
            cause_of_io(&io::Error::from_raw_os_error(raw)),
            BackupCause::Locked
        );
    }
    assert_eq!(
        cause_of_io(&kind(io::ErrorKind::ResourceBusy)),
        BackupCause::Locked
    );
    assert_eq!(
        cause_of_io(&kind(io::ErrorKind::PermissionDenied)),
        BackupCause::WriteFailed
    );
    let sq = |c: ErrorCode| {
        rusqlite::Error::SqliteFailure(
            rusqlite::ffi::Error {
                code: c,
                extended_code: 0,
            },
            None,
        )
    };
    assert_eq!(
        cause_of_sqlite(&sq(ErrorCode::DiskFull)),
        BackupCause::DiskFull
    );
    assert_eq!(
        cause_of_sqlite(&sq(ErrorCode::DatabaseBusy)),
        BackupCause::Locked
    );
    assert_eq!(
        cause_of_sqlite(&sq(ErrorCode::DatabaseLocked)),
        BackupCause::Locked
    );
    assert_eq!(
        cause_of_sqlite(&sq(ErrorCode::DatabaseCorrupt)),
        BackupCause::SourceCorrupt
    );
    for e in [
        BackupError::NotFound,
        BackupError::Corrupt,
        BackupError::SchemaTooNew,
        BackupError::Failed(BackupCause::DiskFull),
        BackupError::Failed(BackupCause::Locked),
    ] {
        assert!(crate::model::ROBUST_ERROR_CODES.contains(&e.code()));
    }
}

// ---- integritet ----

#[test]
fn integrity_codes_for_healthy_truncated_damaged_empty_and_missing() {
    let d = Dir::new("check");
    big_db(&d);
    assert_eq!(Store::check_file(&d.db(), false), IndexCheck::Ok);
    assert_eq!(Store::check_file(&d.db(), true), IndexCheck::Ok);
    assert_eq!(
        Store::check_file(&d.db(), true).status(),
        IntegrityStatus::Ok
    );

    let orig = fs::read(d.db()).unwrap();
    // Avkortet midt i en side.
    fs::write(d.db(), &orig[..orig.len() / 2 + 123]).unwrap();
    let c = Store::check_file(&d.db(), false);
    assert_eq!(c.status(), IntegrityStatus::Corrupt, "{c:?}");
    // Ødelagt side midt i filen.
    fs::write(d.db(), &orig).unwrap();
    damage_middle(&d.db());
    assert_eq!(
        Store::check_file(&d.db(), true).status(),
        IntegrityStatus::Corrupt
    );
    // Ikke en SQLite-fil i det hele tatt.
    fs::write(d.db(), vec![0xAB; 8192]).unwrap();
    assert_eq!(Store::check_file(&d.db(), false), IndexCheck::Corrupt);
    // Tom fil er ikke «en ny indeks».
    fs::write(d.db(), b"").unwrap();
    assert_eq!(Store::check_file(&d.db(), false), IndexCheck::Corrupt);
    // Mangler.
    fs::remove_file(d.db()).unwrap();
    assert_eq!(
        Store::check_file(&d.db(), false).status(),
        IntegrityStatus::Unreadable
    );
}

#[test]
fn schema_too_new_is_unreadable_and_old_is_usable() {
    let d = Dir::new("schema");
    {
        let _s = open_with(&d, 2);
    }
    let c = Connection::open(d.db()).unwrap();
    c.pragma_update(None, "user_version", 99).unwrap();
    drop(c);
    let r = Store::check_file(&d.db(), true);
    assert_eq!(r, IndexCheck::SchemaTooNew(99));
    assert_eq!(
        (r.code(), r.status()),
        ("schema_too_new", IntegrityStatus::Unreadable)
    );
    assert!(!r.usable());
    // Eldre skjema er brukbart (migreres ved åpning) og gir `ok` mot brukeren.
    let c = Connection::open(d.db()).unwrap();
    c.pragma_update(None, "user_version", 3).unwrap();
    drop(c);
    let r = Store::check_file(&d.db(), true);
    assert_eq!(r, IndexCheck::SchemaOld(3));
    assert!(r.usable() && r.status().is_ok());
}

// ---- liste og verifisering ----

#[test]
fn list_flags_truncated_copy_and_verify_catches_bit_flip() {
    let d = Dir::new("verify");
    let _s = open_with(&d, 200);
    let a = manual(&d.db(), 1);
    let b = manual(&d.db(), 2);
    let dir = backup_dir(&d.db());
    // Bit-flipp med samme lengde: listen (størrelse) ser ok, full verifisering fanger den.
    let mut bytes = fs::read(db_file(&dir, a.id)).unwrap();
    let mid = bytes.len() / 2;
    bytes[mid] ^= 0xFF;
    fs::write(db_file(&dir, a.id), bytes).unwrap();
    assert_eq!(
        Store::backup_verify_at(&d.db(), a.id).unwrap_err(),
        BackupError::Corrupt
    );
    // Avkortet: listen flagger den.
    let full = fs::read(db_file(&dir, b.id)).unwrap();
    fs::write(db_file(&dir, b.id), &full[..full.len() / 2]).unwrap();
    let list = Store::backup_list_at(&d.db());
    assert_eq!(
        list.iter().find(|m| m.id == b.id).unwrap().integrity,
        IntegrityStatus::Corrupt
    );
    assert_eq!(
        Store::backup_verify_at(&d.db(), b.id).unwrap_err(),
        BackupError::Corrupt
    );
    assert_eq!(
        Store::backup_verify_at(&d.db(), BackupId(77)).unwrap_err(),
        BackupError::NotFound
    );
    assert!(Store::latest_good_backup_at(&d.db()).is_none());
    // Ødelagt `.meta` listes som ødelagt og kan ikke gjenopprettes.
    fs::write(meta_file(&dir, a.id), b"soeppel").unwrap();
    let list = Store::backup_list_at(&d.db());
    assert_eq!(
        list.iter().find(|m| m.id == a.id).unwrap().integrity,
        IntegrityStatus::Corrupt
    );
}

#[test]
fn latest_good_skips_a_corrupt_newest_and_pre_restore() {
    let d = Dir::new("latest");
    let _s = open_with(&d, 5);
    let a = manual(&d.db(), 1);
    let b = manual(&d.db(), 2);
    let _p = Store::backup_create_at(&d.db(), BackupKind::PreRestore, "pre_restore", 3, 5).unwrap();
    let dir = backup_dir(&d.db());
    fs::write(db_file(&dir, b.id), b"x").unwrap();
    assert_eq!(Store::latest_good_backup_at(&d.db()).unwrap().id, a.id);
}

#[test]
fn before_migration_copies_only_old_schemas() {
    let d = Dir::new("premig");
    {
        let _s = open_with(&d, 4);
    }
    assert!(
        Store::backup_before_migration_at(&d.db(), 1)
            .unwrap()
            .is_none(),
        "nyeste skjema"
    );
    assert!(
        Store::backup_before_migration_at(&d.0.join("ingen.sqlite"), 1)
            .unwrap()
            .is_none()
    );
    let c = Connection::open(d.db()).unwrap();
    c.pragma_update(None, "user_version", 4).unwrap();
    drop(c);
    let made = Store::backup_before_migration_at(&d.db(), 5)
        .unwrap()
        .unwrap();
    assert_eq!(
        (made.meta.kind, made.meta.reason.as_str(), made.meta.schema),
        (BackupKind::Auto, "before_migration", 4)
    );
    // Kopien er en brukbar indeks med det gamle skjemaet (migreres først når den åpnes).
    let copy = db_file(&backup_dir(&d.db()), made.meta.id);
    assert_eq!(Store::check_file(&copy, true), IndexCheck::SchemaOld(4));
}

// ---- gjenoppretting ----

#[test]
fn restore_takes_safety_copy_replaces_index_and_can_be_undone() {
    let d = Dir::new("restore");
    let mut s = open_with(&d, 3);
    let b = manual(&d.db(), 100);
    seed(&mut s, 3, 4);
    assert_eq!(count(&s), 7);
    let gen_before = s.change_counter();

    let r = s.restore_backup(b.id, 200).unwrap();
    assert_eq!(r.restored.id, b.id);
    assert_eq!(r.safety.kind, BackupKind::PreRestore);
    assert_eq!(
        r.safety.asset_count, 7,
        "sikringskopien har det som var der før"
    );
    assert_eq!(count(&s), 3, "indeksen er byttet");
    assert!(s.change_counter() > gen_before);
    assert!(leftovers(&d).is_empty(), "{:?}", leftovers(&d));
    // Den åpne indeksen virker: skrive og lese.
    seed(&mut s, 100, 1);
    assert_eq!(count(&s), 4);
    let pre = kinds(&d.db())
        .iter()
        .filter(|k| **k == BackupKind::PreRestore)
        .count();
    assert_eq!(pre, 1);

    // Angre: sett sikringskopien tilbake (som selv sikrer dagens tilstand).
    let u = s.restore_backup(r.safety.id, 300).unwrap();
    assert_eq!(count(&s), 7);
    assert_eq!(u.safety.asset_count, 4);
    // Ingenting er tapt: alle kopier finnes fortsatt.
    assert_eq!(Store::backup_list_at(&d.db()).len(), 3);
}

#[test]
fn restore_of_corrupt_or_missing_backup_leaves_live_index_untouched() {
    let d = Dir::new("restore-bad");
    let mut s = open_with(&d, 3);
    let b = manual(&d.db(), 1);
    let dir = backup_dir(&d.db());
    let mut bytes = fs::read(db_file(&dir, b.id)).unwrap();
    let n = bytes.len() / 2;
    bytes[n] ^= 0x55;
    fs::write(db_file(&dir, b.id), bytes).unwrap();
    seed(&mut s, 10, 2);
    let e = s.restore_backup(b.id, 2).unwrap_err();
    assert_eq!(e.code(), "backup_corrupt");
    assert_eq!(
        s.restore_backup(BackupId(99), 2).unwrap_err().code(),
        "backup_not_found"
    );
    assert_eq!(count(&s), 5);
    assert!(leftovers(&d).is_empty());
    // Ingen sikringskopi tas når kopien allerede er avvist.
    assert_eq!(kinds(&d.db()), vec![BackupKind::Manual]);
    // Fortsatt skrivbar.
    seed(&mut s, 50, 1);
    assert_eq!(count(&s), 6);
}

#[test]
fn restore_of_schema_too_new_backup_is_refused() {
    let d = Dir::new("restore-new");
    let mut s = open_with(&d, 3);
    // Gi kopien et nyere skjema med riktig sjekksum, som en nyere app ville laget den.
    let b = manual(&d.db(), 1);
    let dir = backup_dir(&d.db());
    let f = db_file(&dir, b.id);
    Connection::open(&f)
        .unwrap()
        .pragma_update(None, "user_version", 99)
        .unwrap();
    let mut m = read_meta(&meta_file(&dir, b.id)).unwrap();
    m.checksum = hash_file(&f).unwrap();
    m.bytes = fs::metadata(&f).unwrap().len();
    write_meta(&meta_file(&dir, b.id), &m).unwrap();
    let e = s.restore_backup(b.id, 2).unwrap_err();
    assert_eq!((e, e.code()), (BackupError::SchemaTooNew, "backup_corrupt"));
    assert_eq!(count(&s), 3);
}

#[test]
fn restore_on_memory_store_is_refused() {
    let mut s = Store::open_in_memory().unwrap();
    assert_eq!(
        s.restore_backup(BackupId(1), 1).unwrap_err().cause(),
        "no_file"
    );
}

const STEPS: [RestoreStep; 5] = [
    RestoreStep::Copied,
    RestoreStep::LiveClosed,
    RestoreStep::OldMoved,
    RestoreStep::NewMoved,
    RestoreStep::Reopened,
];

#[test]
fn error_in_every_step_rolls_back_to_the_old_index() {
    for step in STEPS {
        let d = Dir::new("fault");
        let mut s = open_with(&d, 3);
        let b = manual(&d.db(), 1);
        seed(&mut s, 3, 4); // 7 rader nå
        let fault = Some(Fault { step, crash: false });
        let e = s.restore_backup_with(b.id, 2, fault).unwrap_err();
        assert_eq!(e, BackupError::Failed(BackupCause::Interrupted), "{step:?}");
        assert_eq!(count(&s), 7, "{step:?}: den gamle indeksen er urørt");
        assert!(leftovers(&d).is_empty(), "{step:?}: {:?}", leftovers(&d));
        // Den åpne indeksen er fortsatt brukbar og varig.
        seed(&mut s, 20, 1);
        drop(s);
        let s = Store::open(&d.db()).unwrap();
        assert_eq!(count(&s), 8, "{step:?}");
        assert_eq!(Store::check_file(&d.db(), true), IndexCheck::Ok);
    }
}

#[test]
fn crash_in_every_step_recovers_to_old_or_new_never_to_nothing() {
    for step in STEPS {
        let d = Dir::new("crash");
        let mut s = open_with(&d, 3);
        let b = manual(&d.db(), 1);
        seed(&mut s, 3, 4); // 7 rader (gammel)
        let fault = Some(Fault { step, crash: true });
        let _ = s.restore_backup_with(b.id, 2, fault);
        drop(s); // prosessen døde
        let rec = Store::recover_interrupted_restore(&d.db(), 10);
        let s = Store::open(&d.db()).unwrap();
        let new_in_place = matches!(step, RestoreStep::NewMoved | RestoreStep::Reopened);
        assert_eq!(
            count(&s),
            if new_in_place { 3 } else { 7 },
            "{step:?} {rec:?}"
        );
        assert_eq!(Store::check_file(&d.db(), true), IndexCheck::Ok, "{step:?}");
        assert!(leftovers(&d).is_empty(), "{step:?}: {:?}", leftovers(&d));
        // Det som var der før er alltid med i sikringskopien.
        let safety = Store::backup_list_at(&d.db())
            .into_iter()
            .find(|m| m.kind == BackupKind::PreRestore)
            .unwrap();
        assert_eq!(safety.asset_count, 7);
        if step == RestoreStep::OldMoved {
            assert_eq!(rec, Recovered::RolledBack);
        }
        if new_in_place {
            assert_eq!(rec, Recovered::CleanedUp);
        }
    }
}

#[test]
fn crash_recovery_when_new_index_is_broken_rolls_back_and_keeps_the_broken_one_aside() {
    let d = Dir::new("crash-broken");
    {
        let _s = open_with(&d, 3);
    }
    // Simulert tilstand: ny indeks på plass er avkortet, den gamle ligger som `.before-restore`.
    fs::copy(d.db(), with_suffix(&d.db(), ".before-restore")).unwrap();
    fs::write(d.db(), b"avkortet").unwrap();
    assert_eq!(
        Store::recover_interrupted_restore(&d.db(), 77),
        Recovered::RolledBack
    );
    let s = Store::open(&d.db()).unwrap();
    assert_eq!(count(&s), 3);
    assert_eq!(
        fs::read(with_suffix(&d.db(), ".corrupt-77")).unwrap(),
        b"avkortet"
    );
    drop(s);
    assert_eq!(
        Store::recover_interrupted_restore(&d.db(), 78),
        Recovered::Nothing
    );
}

// ---- trygg modus uten åpen indeks ----

#[test]
fn restore_closed_replaces_corrupt_index_and_keeps_the_corrupt_file() {
    let d = Dir::new("closed");
    {
        let _s = open_with(&d, 40);
        manual(&d.db(), 1);
    }
    // Flush WAL slik at skaden ligger i hovedfilen.
    damage_middle(&d.db());
    let broken = fs::read(d.db()).unwrap();
    let good = Store::latest_good_backup_at(&d.db()).unwrap();
    let m = Store::restore_closed_at(&d.db(), good.id, 55).unwrap();
    assert_eq!(m.asset_count, 40);
    let s = Store::open(&d.db()).unwrap();
    assert_eq!(count(&s), 40);
    // Den ødelagte er ikke slettet.
    assert_eq!(
        fs::read(with_suffix(&d.db(), ".corrupt-55")).unwrap(),
        broken
    );
}

#[test]
fn restore_closed_with_bad_backup_leaves_everything_as_it_was() {
    let d = Dir::new("closed-bad");
    let id;
    {
        let _s = open_with(&d, 5);
        let m = manual(&d.db(), 1);
        id = m.id;
        let dir = backup_dir(&d.db());
        let f = db_file(&dir, m.id);
        let b = fs::read(&f).unwrap();
        fs::write(&f, &b[..b.len() - 10]).unwrap();
    }
    let before = fs::read(d.db()).unwrap();
    let e = Store::restore_closed_at(&d.db(), id, 9).unwrap_err();
    assert_eq!(e, BackupError::Corrupt);
    assert_eq!(fs::read(d.db()).unwrap(), before);
    assert!(leftovers(&d).is_empty());
}

#[test]
fn quarantine_corrupt_index_moves_and_never_deletes() {
    let d = Dir::new("quarantine");
    fs::write(d.db(), "oedelagt").unwrap();
    fs::write(with_suffix(&d.db(), "-wal"), b"wal").unwrap();
    let a = Store::quarantine_corrupt_index(&d.db(), 5).unwrap();
    assert!(!d.db().exists());
    assert_eq!(fs::read(&a).unwrap(), b"oedelagt");
    assert_eq!(fs::read(with_suffix(&a, "-wal")).unwrap(), b"wal");
    // Ny, tom indeks kan åpnes etterpå («bygg på nytt»).
    let s = Store::open(&d.db()).unwrap();
    assert_eq!(count(&s), 0);
    drop(s);
    // Samme tidsstempel to ganger overskriver ikke.
    fs::remove_file(d.db()).unwrap();
    fs::write(d.db(), b"nummer to").unwrap();
    let b = Store::quarantine_corrupt_index(&d.db(), 5).unwrap();
    assert_ne!(a, b);
    assert!(a.exists() && b.exists());
}

#[test]
fn backup_files_contain_only_the_index_never_image_bytes() {
    let d = Dir::new("noimages");
    let _s = open_with(&d, 20);
    let img = d.0.join("foto.jpg");
    fs::write(&img, b"\xFF\xD8\xFF hemmelig bildeinnhold").unwrap();
    let m = manual(&d.db(), 1);
    let dir = backup_dir(&d.db());
    for e in fs::read_dir(&dir).unwrap() {
        let bytes = fs::read(e.unwrap().path()).unwrap();
        assert!(!bytes.windows(8).any(|w| w == b"hemmelig"));
    }
    assert_eq!(
        &fs::read(db_file(&dir, m.id)).unwrap()[..15],
        b"SQLite format 3"
    );
    assert_eq!(
        fs::read(&img).unwrap(),
        b"\xFF\xD8\xFF hemmelig bildeinnhold",
        "originalen er urørt"
    );
}

#[test]
fn concurrent_backups_get_distinct_ids_and_both_verify() {
    for round in 0..5 {
        let d = Dir::new("conc");
        let _s = open_with(&d, 2000);
        let db = d.db();
        let handles: Vec<_> = (0..2)
            .map(|i| {
                let db = db.clone();
                std::thread::spawn(move || {
                    Store::backup_create_at(&db, BackupKind::Manual, "manual", 10 + i, 5)
                })
            })
            .collect();
        let ids: Vec<_> = handles
            .into_iter()
            .map(|h| h.join().unwrap().unwrap().meta.id)
            .collect();
        assert_ne!(ids[0], ids[1], "runde {round}: samme id");
        for id in ids {
            Store::backup_verify_at(&db, id).unwrap_or_else(|e| panic!("runde {round}: {e:?}"));
        }
    }
}
