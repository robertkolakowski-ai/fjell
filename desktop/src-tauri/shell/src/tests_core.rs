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

// ---- M7-hull: liste, gjenoppretting, innstillinger, pause og første gang ----

/// To duplikatpar i tillegg til c.jpg, slik at ett kan gjenopprettes og ett bli liggende.
fn trash_one(e: &Env) -> u64 {
    e.open();
    e.scan();
    let ids = e.dup_ids();
    e.sh.decide(&ids, DecisionDto::Trash).unwrap();
    e.sh.trashed().unwrap()[0].asset
}

#[test]
fn trashed_lists_each_image_with_name_but_no_path() {
    let e = env("list");
    assert_eq!(e.sh.trashed().unwrap_err().code, "no_library");
    trash_one(&e);
    let t = e.sh.trashed().unwrap();
    assert_eq!(t.len(), 1);
    assert!(t[0].name == "a.jpg" || t[0].name == "b.jpg", "{:?}", t[0]);
    assert!(!t[0].name.contains(['/', '\\']), "bare filnavn");
    assert_eq!(t[0].bytes, 21);
    assert!(t[0].expires_at > t[0].trashed_at && !t[0].expired);
    // 30 dager (standard) mellom flyttet og utløp.
    assert_eq!(t[0].expires_at - t[0].trashed_at, 30 * 86_400);
}

#[test]
fn restore_moves_file_back_and_empties_the_list() {
    let e = env("restore");
    let id = trash_one(&e);
    assert_eq!(quarantined(&e.lib), 1);
    let r = e.sh.restore(&[id]).unwrap();
    assert_eq!(r.restored, 1);
    assert!(e.sh.trashed().unwrap().is_empty());
    assert_eq!(quarantined(&e.lib), 0);
    assert!(exists(&e.lib.join("a.jpg")) && exists(&e.lib.join("b.jpg")));
    assert_eq!(
        fs::read(e.lib.join("a.jpg")).unwrap(),
        b"same-bytes-same-bytes",
        "originalfilen er uendret"
    );
    assert_eq!(e.sh.state().trash_count, 0);
    // Gjentatt gjenoppretting er et tomt no-op, ikke en feil.
    assert_eq!(e.sh.restore(&[id]).unwrap().restored, 0);
    assert_eq!(e.sh.restore(&[9999]).unwrap().restored, 0);
}

#[test]
fn restore_all_restores_everything_in_one_call() {
    let e = env("restoreall");
    trash_one(&e);
    assert_eq!(e.sh.restore_all().unwrap().restored, 1);
    assert_eq!(quarantined(&e.lib), 0);
    assert_eq!(e.sh.restore_all().unwrap().restored, 0);
    // Kjernen foreslår ikke et gjenopprettet bilde på nytt (brukeren har bestemt seg): ingen kort etter ny skann.
    e.scan();
    assert!(e.dup_ids().is_empty());
    assert!(exists(&e.lib.join("a.jpg")) && exists(&e.lib.join("b.jpg")));
}

#[test]
fn restore_never_calls_the_recycle_bin() {
    let e = env("restorebin");
    let id = trash_one(&e);
    e.sh.restore(&[id]).unwrap();
    assert!(e.bin.calls.lock().unwrap().is_empty());
}

#[test]
fn settings_show_quarantine_dir_and_recycle_default() {
    let e = env("settings");
    let s = e.sh.settings().unwrap();
    assert_eq!(
        (s.trash_days, s.quarantine_dir, s.empty_default),
        (30, None, "recycle")
    );
    e.open();
    let s = e.sh.settings().unwrap();
    let dir = s.quarantine_dir.unwrap();
    assert!(dir.ends_with(".fjell-karantene"), "{dir}");
    assert!(dir.starts_with(e.lib.to_str().unwrap()));
}

#[test]
fn set_trash_days_validates_and_applies_to_new_items() {
    let e = env("days");
    e.open();
    assert_eq!(e.sh.set_trash_days(0).unwrap_err().code, "bad_request");
    assert_eq!(e.sh.set_trash_days(366).unwrap_err().code, "bad_request");
    assert_eq!(
        e.sh.settings().unwrap().trash_days,
        30,
        "uendret etter avvist verdi"
    );
    assert_eq!(e.sh.set_trash_days(7).unwrap().trash_days, 7);
    assert_eq!(e.sh.state().trash_days, 7);
    e.scan();
    e.sh.decide(&e.dup_ids(), DecisionDto::Trash).unwrap();
    let t = e.sh.trashed().unwrap();
    assert_eq!(t[0].expires_at - t[0].trashed_at, 7 * 86_400);
}

#[test]
fn first_run_until_onboarding_done_or_library_chosen() {
    let e = env("first");
    assert!(e.sh.first_run());
    e.sh.complete_onboarding().unwrap();
    assert!(!e.sh.first_run());
    // Overlever ny Shell på samme app-data (omstart), og valg av mappe overskriver ikke flagget.
    let again = Shell::with_data_dir(&e.data, e.bin.clone() as Arc<dyn RecycleBin>);
    assert!(!again.first_run());
    assert!(e.sh.set_library(e.lib.to_str().unwrap()).is_ok());
    assert!(!Shell::with_data_dir(&e.data, e.bin.clone() as Arc<dyn RecycleBin>).first_run());
}

#[test]
fn choosing_a_library_alone_ends_first_run() {
    let e = env("firstlib");
    assert!(e.sh.first_run());
    e.sh.set_library(e.lib.to_str().unwrap()).unwrap();
    assert!(!e.sh.first_run());
}

#[test]
fn pause_marks_state_and_resume_scans_again() {
    let e = env("pause");
    e.open();
    e.scan();
    e.sh.pause_scan().unwrap();
    assert!(e.sh.state().scan_paused);
    e.sh.resume_scan().unwrap();
    assert!(!e.sh.state().scan_paused);
    let t = Instant::now();
    while e.sh.state().scan_phase != "done" {
        assert!(
            t.elapsed() < Duration::from_secs(20),
            "ny skanning ble ikke ferdig"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(e.dup_ids().len(), 1, "funnet beholdes");
    // Resume uten pause gjør ingenting; cancel nullstiller pause.
    e.sh.pause_scan().unwrap();
    e.sh.cancel_scan().unwrap();
    assert!(!e.sh.state().scan_paused);
    assert!(e.sh.resume_scan().is_ok());
}

#[test]
fn mock_mode_reports_unsupported_for_trash_listing() {
    let sh = Shell::mock();
    assert_eq!(sh.trashed().unwrap_err().code, "unsupported");
    assert_eq!(sh.restore(&[1]).unwrap_err().code, "unsupported");
    assert!(!sh.first_run(), "ingen app-data: aldri onboarding i mock");
}

/// Første oppstart på ekte Windows: app-data-mappen finnes ikke ennå. Tauri gir bare stien.
/// Feilen var at indeksen åpnes før mappen opprettes (SQLite lager ikke mapper).
#[test]
fn set_library_works_when_app_data_dir_does_not_exist_yet() {
    let root = std::env::temp_dir().join(format!("fjell-shell-forste-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    let (lib, data) = (
        root.join("bibliotek"),
        root.join("ikke").join("opprettet").join("ennå"),
    );
    fs::create_dir_all(&lib).unwrap();
    fs::write(lib.join("a.jpg"), b"x").unwrap();
    assert!(!data.exists());
    let bin = Arc::new(FakeRecycleBin::default());
    let sh = Shell::with_data_dir(&data, bin as Arc<dyn RecycleBin>);
    let r = sh.set_library(lib.to_str().unwrap());
    assert!(r.is_ok(), "set_library feilet på første oppstart: {r:?}");
    assert!(data.is_dir(), "app-data-mappen skal opprettes");
    assert_eq!(sh.library().as_deref(), lib.to_str());
    let _ = fs::remove_dir_all(&root);
}
