//! Tester av ekte kjerne (`Fjell` + `FsSource` + karantene + «Tøm») mot temp-mapper, med
//! `FakeRecycleBin` i stedet for Windows Papirkurv. Kjører på Linux uten Tauri.

use crate::dto::{DecisionDto, PermanentAckDto};
use crate::{NoRecycleBin, Shell};
use fjell_fs::{FakeRecycleBin, FsSource, RecycleBin};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

struct Env {
    root: PathBuf,
    lib: PathBuf,
    data: PathBuf,
    bin: Arc<FakeRecycleBin>,
    sh: Shell,
}

fn env(tag: &str) -> Env {
    let root = std::env::temp_dir().join(format!("fjell-shell-{tag}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    let (lib, data) = (root.join("bibliotek"), root.join("appdata"));
    fs::create_dir_all(&lib).unwrap();
    fs::create_dir_all(&data).unwrap();
    // To like filer og en unik: ett eksakt duplikatpar.
    fs::write(lib.join("a.jpg"), b"same-bytes-same-bytes").unwrap();
    fs::write(lib.join("b.jpg"), b"same-bytes-same-bytes").unwrap();
    fs::write(lib.join("c.jpg"), b"different-bytes").unwrap();
    let bin = Arc::new(FakeRecycleBin {
        bin_dir: Some(root.join("papirkurv")),
        ..Default::default()
    });
    let sh = Shell::with_data_dir(&data, bin.clone() as Arc<dyn RecycleBin>);
    Env {
        root,
        lib,
        data,
        bin,
        sh,
    }
}

impl Env {
    fn open(&self) {
        let src = FsSource::new(&self.lib).without_decoding();
        self.sh.open_library(&self.lib, Arc::new(src)).unwrap();
    }
    fn scan(&self) {
        self.sh.start_scan(true).unwrap();
        let t = Instant::now();
        while self.sh.state().scan_phase != "done" {
            assert!(
                t.elapsed() < Duration::from_secs(20),
                "skanning ble ikke ferdig"
            );
            std::thread::sleep(Duration::from_millis(10));
        }
    }
    fn dup_ids(&self) -> Vec<u64> {
        let p = self.sh.cards("exact_dup", 50, None).unwrap();
        p.cards.iter().map(|c| c.id).collect()
    }
}

impl Drop for Env {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn exists(p: &Path) -> bool {
    p.exists()
}

fn quarantined(lib: &Path) -> usize {
    fn walk(d: &Path) -> usize {
        fs::read_dir(d).map_or(0, |rd| {
            rd.filter_map(|e| e.ok())
                .map(|e| {
                    let p = e.path();
                    if p.is_dir() {
                        walk(&p)
                    } else {
                        usize::from(p.extension().is_some_and(|x| x == "jpg"))
                    }
                })
                .sum()
        })
    }
    walk(&lib.join(".fjell-karantene"))
}

#[test]
fn no_library_gives_idle_state_and_no_library_errors() {
    let e = env("nolib");
    let s = e.sh.state();
    assert_eq!((s.backend, s.scan_phase), ("core", "idle"));
    assert_eq!(e.sh.start_scan(false).unwrap_err().code, "no_library");
    assert_eq!(e.sh.undo().unwrap_err().code, "no_library");
}

#[test]
fn scan_reports_exact_groups() {
    let e = env("scan");
    e.open();
    e.scan();
    let s = e.sh.state();
    assert_eq!(s.backend, "core");
    let exact = s.per_kind.iter().find(|k| k.kind == "exact_dup").unwrap();
    assert_eq!(exact.count, 1, "ett overskuddskort i paret");
    assert_eq!(e.dup_ids().len(), 1);
}

#[test]
fn trash_moves_to_quarantine_and_undo_restores() {
    let e = env("trash");
    e.open();
    e.scan();
    let ids = e.dup_ids();
    e.sh.decide(&ids, DecisionDto::Trash).unwrap();
    assert_eq!(e.sh.state().trash_count, 1);
    assert_eq!(quarantined(&e.lib), 1);
    let present = ["a.jpg", "b.jpg"]
        .iter()
        .filter(|n| exists(&e.lib.join(n)))
        .count();
    assert_eq!(
        present, 1,
        "en av duplikatene er flyttet bort, ikke slettet"
    );
    e.sh.undo().unwrap();
    assert_eq!(e.sh.state().trash_count, 0);
    assert_eq!(quarantined(&e.lib), 0);
    assert!(exists(&e.lib.join("a.jpg")) && exists(&e.lib.join("b.jpg")));
    assert_eq!(
        fs::read(e.lib.join("a.jpg")).unwrap(),
        b"same-bytes-same-bytes"
    );
}

#[test]
fn rescan_after_trash_does_not_lose_staged_files() {
    let e = env("rescan");
    e.open();
    e.scan();
    e.sh.decide(&e.dup_ids(), DecisionDto::Trash).unwrap();
    e.scan();
    assert_eq!(e.sh.state().trash_count, 1);
    e.sh.undo().unwrap();
    assert!(exists(&e.lib.join("a.jpg")) && exists(&e.lib.join("b.jpg")));
}

#[test]
fn empty_goes_to_recycle_bin_not_permanent() {
    let e = env("empty");
    e.open();
    e.scan();
    e.sh.decide(&e.dup_ids(), DecisionDto::Trash).unwrap();
    let r = e.sh.empty(&[], None).unwrap();
    assert_eq!((r.purged, r.kept_trashed, r.unsupported), (1, 0, 0));
    assert!(r.freed_bytes > 0);
    assert_eq!(e.bin.calls.lock().unwrap().len(), 1, "gikk via Papirkurv");
    assert_eq!(fs::read_dir(e.root.join("papirkurv")).unwrap().count(), 1);
    assert_eq!(quarantined(&e.lib), 0);
    let s = e.sh.trash_summary().unwrap();
    assert_eq!((s.count, s.freed_bytes > 0), (0, true));
    assert_eq!(e.sh.undo().unwrap_err().code, "not_undoable");
}

#[test]
fn empty_on_volume_without_recycle_bin_keeps_everything() {
    let e = env("nobin");
    // Eget skall med Papirkurv som ikke støttes: ingenting skal slettes.
    let sh = Shell::with_data_dir(&e.data, Arc::new(NoRecycleBin));
    sh.open_library(&e.lib, Arc::new(FsSource::new(&e.lib).without_decoding()))
        .unwrap();
    sh.start_scan(true).unwrap();
    while sh.state().scan_phase != "done" {
        std::thread::sleep(Duration::from_millis(10));
    }
    let ids: Vec<u64> = sh
        .cards("exact_dup", 50, None)
        .unwrap()
        .cards
        .iter()
        .map(|c| c.id)
        .collect();
    sh.decide(&ids, DecisionDto::Trash).unwrap();
    let r = sh.empty(&[], None).unwrap();
    assert_eq!((r.purged, r.unsupported, r.kept_trashed), (0, 1, 1));
    assert_eq!(sh.state().trash_count, 1);
    assert_eq!(quarantined(&e.lib), 1, "filen ligger fortsatt i karantene");
}

#[test]
fn permanent_requires_ack_with_matching_count() {
    let e = env("perm");
    e.open();
    e.scan();
    e.sh.decide(&e.dup_ids(), DecisionDto::Trash).unwrap();
    let no = |checked, count| Some(PermanentAckDto { checked, count });
    assert_eq!(
        e.sh.empty(&[], no(false, 1)).unwrap_err().code,
        "ack_required"
    );
    assert_eq!(
        e.sh.empty(&[], no(true, 5)).unwrap_err().code,
        "ack_mismatch"
    );
    assert_eq!(e.sh.state().trash_count, 1);
    assert_eq!(quarantined(&e.lib), 1, "ingenting slettet uten gyldig ack");
    let r = e.sh.empty(&[], no(true, 1)).unwrap();
    assert_eq!(r.purged, 1);
    assert!(
        e.bin.calls.lock().unwrap().is_empty(),
        "permanent: ikke Papirkurv"
    );
    assert_eq!(quarantined(&e.lib), 0);
}

#[test]
fn set_keeper_maps_unknown_asset() {
    let e = env("keeper");
    e.open();
    e.scan();
    let g = e.sh.cards("exact_dup", 5, None).unwrap().cards[0]
        .group
        .unwrap();
    assert_eq!(e.sh.set_keeper(g, 9_999).unwrap_err().code, "unknown_asset");
}

#[test]
fn reopen_restores_last_library_and_reconciles_interrupted_undo() {
    let e = env("reopen");
    e.sh.set_library(e.lib.to_str().unwrap()).unwrap();
    assert_eq!(e.sh.library().as_deref(), e.lib.to_str());
    e.open();
    e.scan();
    e.sh.decide(&e.dup_ids(), DecisionDto::Trash).unwrap();
    // Simuler avbrutt angre: indeksen angret, men filen ble liggende i karantene.
    let (api, _) = e.sh.parts().unwrap();
    api.undo().unwrap();
    assert_eq!(quarantined(&e.lib), 1);
    let again = Shell::with_data_dir(&e.data, e.bin.clone() as Arc<dyn RecycleBin>);
    assert_eq!(again.library().as_deref(), e.lib.to_str());
    assert_eq!(quarantined(&e.lib), 0, "avstemming flyttet filen tilbake");
    assert!(exists(&e.lib.join("a.jpg")) && exists(&e.lib.join("b.jpg")));
}

#[test]
fn rejects_missing_folder_and_index_inside_library() {
    let e = env("bad");
    assert_eq!(
        e.sh.set_library("/finnes/ikke/fjell").unwrap_err().code,
        "not_a_folder"
    );
    let inside = Shell::with_data_dir(e.lib.join("data"), Arc::new(NoRecycleBin));
    let err = inside.set_library(e.lib.to_str().unwrap()).unwrap_err();
    assert_eq!(err.code, "bad_request");
}

#[test]
fn ack_required_carries_codes_and_confirmation_is_validated() {
    let e = env("ack");
    e.open();
    assert_eq!(
        e.sh.decide_confirmed(&[1], DecisionDto::Trash, &["nope".into()])
            .unwrap_err()
            .code,
        "bad_request"
    );
}
