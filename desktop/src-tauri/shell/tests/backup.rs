//! Sikkerhetskopi av indeksen og integritet gjennom skallets kommandolag (docs/v1/BACKUP.md).
//! Kontrakten mot mocken (standardsvar, JSON-form, ingen gjenoppretting uten avkrysning) og ekte kjerne mot
//! temp-mapper: kopi, gjenoppretting, korrupt indeks ved oppstart (trygg modus), gjenoppretting uten åpen
//! indeks og «bygg på nytt». Kjørt på Linux; ikke verifisert på Windows.

use fjell_fs::{FakeRecycleBin, RecycleBin};
use fjell_shell::dto::DecisionDto;
use fjell_shell::Shell;
use serde_json::json;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

#[test]
fn mock_defaults_are_empty_and_integrity_is_ok() {
    let shell = Shell::mock();
    assert!(shell.backups().unwrap().is_empty());
    assert_eq!(shell.integrity().unwrap(), "ok");
    let e = shell.backup_create().unwrap_err();
    assert_eq!(
        (e.code, e.detail.as_str()),
        ("core_error", "not_implemented")
    );
}

#[test]
fn restore_without_acknowledgement_is_refused_before_anything_happens() {
    let shell = Shell::mock();
    let e = shell.backup_restore(1, false).unwrap_err();
    assert_eq!(
        (e.code, e.detail.as_str()),
        ("core_error", "backup_ack_required")
    );
}

#[test]
fn integrity_without_library_is_ok() {
    // Uten bibliotek finnes ingen indeks å sjekke; frontend skal ikke gå i trygg modus av den grunn.
    assert_eq!(Shell::mock().integrity().unwrap(), "ok");
}

#[test]
fn json_shape_is_camel_case_codes_only() {
    let b = fjell_shell::dto_robust::BackupDto {
        id: 2,
        kind: "pre_restore",
        created_at: 1_700_000_000,
        bytes: 4096,
        asset_count: 10,
        integrity: "ok",
    };
    assert_eq!(
        serde_json::to_value(&b).unwrap(),
        json!({ "id": 2, "kind": "pre_restore", "createdAt": 1_700_000_000, "bytes": 4096, "assetCount": 10, "integrity": "ok" })
    );
}

// ---- ekte kjerne ----

struct Env {
    root: PathBuf,
    lib: PathBuf,
    data: PathBuf,
    sh: Shell,
}

fn bin(root: &Path) -> Arc<dyn RecycleBin> {
    Arc::new(FakeRecycleBin {
        bin_dir: Some(root.join("papirkurv")),
        ..Default::default()
    })
}

fn env(tag: &str) -> Env {
    let root =
        std::env::temp_dir().join(format!("fjell-shell-backup-{tag}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    let (lib, data) = (root.join("bibliotek"), root.join("appdata"));
    fs::create_dir_all(&lib).unwrap();
    fs::create_dir_all(&data).unwrap();
    fs::write(lib.join("a.jpg"), b"same-bytes-same-bytes").unwrap();
    fs::write(lib.join("b.jpg"), b"same-bytes-same-bytes").unwrap();
    fs::write(lib.join("c.jpg"), b"different-bytes").unwrap();
    let sh = Shell::with_data_dir(&data, bin(&root));
    Env {
        root,
        lib,
        data,
        sh,
    }
}

impl Env {
    fn open(&self) {
        self.sh.set_library(self.lib.to_str().unwrap()).unwrap();
    }
    fn scan(&self) {
        self.sh.start_scan(true).unwrap();
        self.wait_scan();
    }
    fn wait_scan(&self) {
        let t = Instant::now();
        while self.sh.state().scan_phase != "done" {
            assert!(
                t.elapsed() < Duration::from_secs(30),
                "skanning ble ikke ferdig"
            );
            std::thread::sleep(Duration::from_millis(10));
        }
    }
    fn index(&self) -> PathBuf {
        fs::read_dir(&self.data)
            .unwrap()
            .map(|e| e.unwrap().path())
            .find(|p| {
                p.file_name()
                    .unwrap()
                    .to_string_lossy()
                    .ends_with(".sqlite")
            })
            .expect("indeksfil")
    }
    /// Ny skall-instans mot samme app-data-mappe (omstart av appen).
    fn restart(&mut self) {
        let sh = std::mem::replace(&mut self.sh, Shell::mock());
        drop(sh);
        self.sh = Shell::with_data_dir(&self.data, bin(&self.root));
    }
}

impl Drop for Env {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

/// `<indeks>.corrupt-<tid>` (ikke sidefilene `-wal`/`-shm`).
fn is_corrupt_aside(name: &str) -> bool {
    name.contains(".corrupt-") && !name.ends_with("-wal") && !name.ends_with("-shm")
}

fn damage_middle(p: &Path) {
    let mut b = fs::read(p).unwrap();
    let from = b.len() / 3;
    let to = (b.len() * 2 / 3).max(from + 4096).min(b.len());
    for (i, x) in b[from..to].iter_mut().enumerate() {
        *x = (i as u8).wrapping_mul(31).wrapping_add(7);
    }
    fs::write(p, b).unwrap();
}

#[test]
fn real_core_create_list_restore_and_undo() {
    let e = env("flow");
    e.open();
    e.scan();
    assert!(e.sh.backups().unwrap().is_empty());
    assert_eq!(e.sh.integrity().unwrap(), "ok");
    let b = e.sh.backup_create().unwrap();
    assert_eq!((b.kind, b.asset_count, b.integrity), ("manual", 3, "ok"));
    assert_eq!(e.sh.backups().unwrap(), vec![b]);

    // Slett et duplikat, så gjenopprett kopien: indeksen er tilbake før slettingen, filen er lagt tilbake.
    let ids: Vec<u64> =
        e.sh.cards("exact_dup", 50, None)
            .unwrap()
            .cards
            .iter()
            .map(|c| c.id)
            .collect();
    e.sh.decide(&ids, DecisionDto::Trash).unwrap();
    assert_eq!(e.sh.trash_summary().unwrap().count, 1);
    assert!(
        e.sh.backup_restore(1, false).is_err(),
        "uten avkrysning skjer ingenting"
    );
    assert_eq!(e.sh.trash_summary().unwrap().count, 1);
    let r = e.sh.backup_restore(1, true).unwrap();
    assert_eq!((r.restored, r.asset_count), (1, 3));
    assert_eq!(e.sh.trash_summary().unwrap().count, 0);
    assert!(
        e.lib.join("a.jpg").exists() && e.lib.join("b.jpg").exists(),
        "ingen fil mistet"
    );
    // Angre gjenopprettingen: «Nylig slettet» er tilbake som før.
    e.sh.backup_restore_undo(r.safety).unwrap();
    assert_eq!(e.sh.trash_summary().unwrap().count, 1);
    assert_eq!(e.sh.integrity().unwrap(), "ok");
}

#[test]
fn corrupt_index_at_startup_gives_safe_mode_and_restore_from_latest_good_backup() {
    let mut e = env("safe");
    e.open();
    e.scan();
    e.sh.backup_create().unwrap();
    e.restart();
    assert_eq!(e.sh.integrity().unwrap(), "ok");
    assert!(e.sh.library().is_some());
    let idx = e.index();
    e.restart();
    // Ødelegg indeksen mens appen er stengt.
    damage_middle(&idx);
    e.restart();
    assert!(e.sh.library().is_none(), "kjernen kunne ikke åpne indeksen");
    assert_eq!(e.sh.integrity().unwrap(), "corrupt");
    // Kopiene er lesbare uten åpen indeks, og den siste gode kan gjenopprettes.
    let list = e.sh.backups().unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].integrity, "ok");
    let r = e.sh.backup_restore(list[0].id, true).unwrap();
    assert_eq!((r.restored, r.safety, r.asset_count), (list[0].id, 0, 3));
    assert!(e.sh.library().is_some(), "biblioteket er åpnet igjen");
    assert_eq!(e.sh.integrity().unwrap(), "ok");
    // Den ødelagte filen er ikke slettet, og originalene er urørt.
    let kept = fs::read_dir(&e.data)
        .unwrap()
        .filter(|f| is_corrupt_aside(&f.as_ref().unwrap().file_name().to_string_lossy()))
        .count();
    assert_eq!(kept, 1);
    assert_eq!(fs::read(e.lib.join("c.jpg")).unwrap(), b"different-bytes");
}

#[test]
fn rebuild_index_keeps_corrupt_file_originals_and_quarantined_files_are_put_back() {
    let mut e = env("rebuild");
    e.open();
    e.scan();
    // Slett et duplikat: filen ligger nå i karantenen, og indeksen vet det.
    let ids: Vec<u64> =
        e.sh.cards("exact_dup", 50, None)
            .unwrap()
            .cards
            .iter()
            .map(|c| c.id)
            .collect();
    e.sh.decide(&ids, DecisionDto::Trash).unwrap();
    assert!(!(e.lib.join("a.jpg").exists() && e.lib.join("b.jpg").exists()));
    let idx = e.index();
    e.restart();
    // Hel indeks: «bygg på nytt» avvises og ingenting røres.
    assert_eq!(e.sh.rebuild_index().unwrap_err().code, "bad_request");
    assert!(idx.exists() && e.sh.library().is_some());
    e.restart();
    // Avkortet indeks uten noen kopi.
    let bytes = fs::read(&idx).unwrap();
    fs::remove_file(format!("{}-wal", idx.display())).ok();
    fs::remove_file(format!("{}-shm", idx.display())).ok();
    fs::write(&idx, &bytes[..bytes.len() / 2]).unwrap();
    e.restart();
    assert_eq!(e.sh.integrity().unwrap(), "corrupt");
    assert!(
        e.sh.backups().unwrap().is_empty(),
        "ingen kopi å gjenopprette"
    );

    let r = e.sh.rebuild_index().unwrap();
    assert!(r.corrupt_kept && r.scanning && r.trash_state_reset);
    e.wait_scan();
    assert!(e.sh.library().is_some());
    assert_eq!(e.sh.integrity().unwrap(), "ok");
    // Den ødelagte indeksen ligger igjen; ingen originaler er slettet: begge duplikatene finnes igjen på sin plass.
    let kept = fs::read_dir(&e.data)
        .unwrap()
        .filter(|f| is_corrupt_aside(&f.as_ref().unwrap().file_name().to_string_lossy()))
        .count();
    assert_eq!(kept, 1);
    assert!(
        e.lib.join("a.jpg").exists()
            && e.lib.join("b.jpg").exists()
            && e.lib.join("c.jpg").exists()
    );
    assert_eq!(
        e.sh.state()
            .per_kind
            .iter()
            .find(|k| k.kind == "exact_dup")
            .map(|k| k.count),
        Some(1)
    );
}

#[test]
fn prepare_index_rolls_back_an_interrupted_restore() {
    let mut e = env("prepare");
    e.open();
    e.scan();
    let idx = e.index();
    drop(std::mem::replace(&mut e.sh, Shell::mock()));
    // Krasj midt i byttet: indeksen mangler, den gamle ligger som `.before-restore`.
    let before = PathBuf::from(format!("{}.before-restore", idx.display()));
    fs::rename(&idx, &before).unwrap();
    // `prepare_index` kalles før indeksen åpnes, og trenger ikke noe åpent bibliotek.
    let sh = Shell::mock();
    let p = sh.prepare_index(&idx);
    assert_eq!((p.recovered, p.migration_backup), ("rolled_back", false));
    assert!(idx.exists() && !before.exists());
    assert_eq!(sh.prepare_index(&idx).recovered, "none");
}
