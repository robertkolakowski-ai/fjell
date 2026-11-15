//! Tester for backup og gjenoppretting gjennom `Fjell` (docs/v1/BACKUP.md). Kjørt på Linux.

use crate::api::Decision;
use crate::api::{ApiError, Fjell, FjellApi, ScanConfig, ScanMode, SuggestionKind};
use crate::model::{
    AssetMeta, BackupId, BackupKind, FakeClock, IntegrityStatus, Kind, MemSource, ProtectFlags,
    RestoreAck, SourceId,
};
use crate::store::Store;
use std::sync::Arc;

struct Dir(std::path::PathBuf);
impl Drop for Dir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn fjell_in(tag: &str, pairs: usize) -> (Fjell, Arc<FakeClock>, Dir) {
    let dir = std::env::temp_dir().join(format!("fjell-api-backup-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let src = MemSource::new();
    for p in 0..pairs {
        let bytes = format!("pair-{p}").into_bytes();
        for (k, name) in ["a", "b"].into_iter().enumerate() {
            src.put(
                AssetMeta {
                    source_id: SourceId(format!("p{p:04}{name}.jpg")),
                    kind: Kind::Photo,
                    created_at: 1_000 + (p * 2 + k) as i64,
                    mod_token: "m".into(),
                    width: 100,
                    height: 100,
                    bytes: bytes.len() as u64,
                    mime: "image/jpeg".into(),
                    platform_screenshot_hint: false,
                    has_camera_exif: Some(true),
                    is_local: true,
                    protect_flags: ProtectFlags::default(),
                    albums: vec![],
                    backup: None,
                },
                None,
                bytes.clone(),
            );
        }
    }
    let clock = Arc::new(FakeClock::new(5_000_000));
    let f = Fjell::new(
        Store::open(&dir.join("index.sqlite")).unwrap(),
        Arc::new(src),
        clock.clone(),
    );
    f.start_scan(ScanConfig {
        mode: ScanMode::Quick,
        priority: 1,
        deadline: None,
    })
    .unwrap();
    f.wait_scan().unwrap().unwrap();
    (f, clock, Dir(dir))
}

fn trash_one(f: &Fjell) {
    let cards = f.cards(SuggestionKind::ExactDup, 100, None).unwrap().cards;
    f.decide_many(vec![(cards[0].id, Decision::Trash)]).unwrap();
}

#[test]
fn create_list_restore_and_undo_through_the_api() {
    let (f, _c, _d) = fjell_in("flow", 5);
    assert_eq!(f.backup_list().unwrap(), vec![]);
    let b = f.backup_create().unwrap();
    assert_eq!(
        (b.kind, b.asset_count, b.integrity),
        (BackupKind::Manual, 10, IntegrityStatus::Ok)
    );
    assert_eq!(f.backup_list().unwrap(), vec![b]);

    trash_one(&f);
    assert_eq!(f.trash_summary().count, 1);
    // Uten avkrysning finnes ingen `RestoreAck`: porten er typen.
    assert!(RestoreAck::new(b.id, false).is_none());
    let r = f
        .backup_restore(RestoreAck::new(b.id, true).unwrap())
        .unwrap();
    assert_eq!((r.restored, r.asset_count), (b.id, 10));
    assert_eq!(f.trash_summary().count, 0, "indeksen er tilbake til kopien");
    let list = f.backup_list().unwrap();
    assert_eq!(list.len(), 2);
    assert!(list
        .iter()
        .any(|e| e.id == r.safety && e.kind == BackupKind::PreRestore));

    // Angre setter «papirkurv»-tilstanden tilbake.
    f.backup_restore_undo(r.safety).unwrap();
    assert_eq!(f.trash_summary().count, 1);
    // Angre av noe som ikke er en sikringskopi avvises.
    assert_eq!(
        f.backup_restore_undo(b.id),
        Err(ApiError::Internal("backup_not_found".into()))
    );
    assert_eq!(f.integrity_check().unwrap(), IntegrityStatus::Ok);
    // Fortsatt brukbar: kort, handlinger og angre virker etter bytte av indeks.
    trash_one(&f);
    f.undo().unwrap();
}

#[test]
fn unknown_and_corrupt_backups_give_codes_and_change_nothing() {
    let (f, _c, d) = fjell_in("codes", 3);
    assert_eq!(
        f.backup_restore(RestoreAck::new(BackupId(42), true).unwrap()),
        Err(ApiError::Internal("backup_not_found".into()))
    );
    let b = f.backup_create().unwrap();
    // Ødelegg kopien.
    let dir = d.0.join("backups").join("index.sqlite");
    let file = dir.join(format!("backup-{:06}.db", b.id.0));
    let bytes = std::fs::read(&file).unwrap();
    std::fs::write(&file, &bytes[..bytes.len() / 2]).unwrap();
    assert_eq!(
        f.backup_list().unwrap()[0].integrity,
        IntegrityStatus::Corrupt
    );
    trash_one(&f);
    assert_eq!(
        f.backup_restore(RestoreAck::new(b.id, true).unwrap()),
        Err(ApiError::Internal("backup_corrupt".into()))
    );
    assert_eq!(f.trash_summary().count, 1, "urørt");
}

#[test]
fn memory_index_has_no_backups_and_integrity_is_ok() {
    // Minnedatabase: ingen kopier, men `integrity_check` er ok og `backup_create` gir kode.
    let f = Fjell::new(
        Store::open_in_memory().unwrap(),
        Arc::new(MemSource::new()),
        Arc::new(FakeClock::new(1)),
    );
    assert_eq!(f.backup_list().unwrap(), vec![]);
    assert_eq!(f.integrity_check().unwrap(), IntegrityStatus::Ok);
    assert_eq!(
        f.backup_create(),
        Err(ApiError::Internal("backup_failed".into()))
    );
}

#[test]
fn auto_backup_runs_only_when_due_and_rolls() {
    let (f, clock, _d) = fjell_in("auto", 2);
    let first = f.backup_auto_if_due("weekly").unwrap().unwrap();
    assert_eq!(first.kind, BackupKind::Auto);
    assert!(
        f.backup_auto_if_due("weekly").unwrap().is_none(),
        "ikke igjen samme uke"
    );
    assert!(
        f.backup_auto_if_due("before_purge").unwrap().is_none(),
        "kopi finnes siste time"
    );
    clock.advance(2 * 3600);
    assert!(f.backup_auto_if_due("before_purge").unwrap().is_some());
    clock.advance(8 * 24 * 3600);
    assert!(f.backup_auto_if_due("weekly").unwrap().is_some());
    assert!(f.backup_auto_if_due("ukjent").is_err());
    // Standard: de 5 siste automatiske beholdes.
    for _ in 0..8 {
        clock.advance(8 * 24 * 3600);
        f.backup_auto_if_due("weekly").unwrap().unwrap();
    }
    let autos = f
        .backup_list()
        .unwrap()
        .iter()
        .filter(|e| e.kind == BackupKind::Auto)
        .count();
    assert_eq!(autos, 5);
}

#[test]
fn backup_does_not_block_actions() {
    // Kopien holder ikke skrivelåsen: en handling midt i en kopi venter ikke på den.
    let (f, _c, _d) = fjell_in("nonblock", 200);
    let f = Arc::new(f);
    let g = f.clone();
    let t = std::thread::spawn(move || {
        for _ in 0..5 {
            g.backup_create().unwrap();
        }
    });
    let mut worst = std::time::Duration::ZERO;
    for _ in 0..20 {
        let t0 = std::time::Instant::now();
        let _ = f.trash_summary();
        worst = worst.max(t0.elapsed());
    }
    t.join().unwrap();
    assert!(worst < std::time::Duration::from_millis(100), "{worst:?}");
}
