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

/// Starter skann via skallet og venter til den er ferdig (ikke avhengig av forrige fase).
fn rescan(e: &Env, full: bool) {
    e.sh.start_scan(full).unwrap();
    let core = e.sh.core().unwrap();
    let _ = core.fjell.wait_scan();
}

fn exact_count(e: &Env) -> u64 {
    e.sh.state()
        .per_kind
        .iter()
        .find(|k| k.kind == "exact_dup")
        .map_or(0, |k| k.count)
}

#[test]
fn production_source_detects_removed_files_after_restart() {
    // T11 (restanse 12): kjente id-er fra indeksen sendes til `FsSource`, så en fil som er slettet
    // utenfor Fjell mens appen var lukket, oppdages ved neste (inkrementelle) skann.
    let e = env("removed");
    e.sh.set_library(e.lib.to_str().unwrap()).unwrap(); // produksjonskilden
    rescan(&e, true);
    assert_eq!(exact_count(&e), 1);
    fs::remove_file(e.lib.join("b.jpg")).unwrap(); // utenfor appen
    let sh = Shell::with_data_dir(&e.data, e.bin.clone() as Arc<dyn RecycleBin>);
    sh.start_scan(false).unwrap();
    let _ = sh.core().unwrap().fjell.wait_scan();
    let exact = sh
        .state()
        .per_kind
        .into_iter()
        .find(|k| k.kind == "exact_dup");
    assert_eq!(
        exact.map_or(0, |k| k.count),
        0,
        "fjernet fil gir ikke lenger et duplikatpar"
    );
}

#[test]
fn production_source_treats_empty_mount_as_offline_not_removed() {
    // En tom mappe (frakoblet disk som viser tom monteringsmappe) skal aldri tolkes som tomt bibliotek.
    let e = env("emptymount");
    e.sh.set_library(e.lib.to_str().unwrap()).unwrap();
    rescan(&e, true);
    assert_eq!(exact_count(&e), 1);
    for n in ["a.jpg", "b.jpg", "c.jpg"] {
        fs::remove_file(e.lib.join(n)).unwrap();
    }
    rescan(&e, false);
    assert_eq!(exact_count(&e), 1, "ingenting regnes som fjernet");
}

#[test]
fn trashed_files_in_quarantine_are_not_reported_removed() {
    // Papirkurv-bilder ligger i karantenen med vilje; de er ikke «aktive» og skal ikke meldes fjernet.
    let e = env("quarantine-known");
    e.sh.set_library(e.lib.to_str().unwrap()).unwrap();
    rescan(&e, true);
    let ids = e.dup_ids();
    e.sh.decide(&ids, DecisionDto::Trash).unwrap();
    rescan(&e, false);
    assert_eq!(e.sh.state().trash_count, 1, "fortsatt i Nylig slettet");
    assert_eq!(quarantined(&e.lib), 1);
}

#[test]
fn production_source_caches_thumbnails_in_app_data_only() {
    use fjell_core::model::{AssetSource, SourceId};
    let e = env("thumbs");
    // Ekte (liten) JPEG så dekoding gir miniatyr.
    let jpg = TINY_JPEG.to_vec();
    fs::write(e.lib.join("d.jpg"), &jpg).unwrap();
    let lib_before = fs::read_dir(&e.lib).unwrap().count();
    let src = crate::production_source(&e.data, &e.lib);
    let t = src.thumbnail(&SourceId("d.jpg".into())).unwrap();
    assert_eq!((t.w, t.h), (8, 6));
    let cache = crate::thumb_cache_dir(&e.data);
    assert!(cache.starts_with(&e.data) && !cache.starts_with(&e.lib));
    assert_eq!(fs::read_dir(&cache).unwrap().count(), 1);
    assert_eq!(
        fs::read_dir(&e.lib).unwrap().count(),
        lib_before,
        "biblioteket er urørt"
    );
    assert_eq!(fs::read(e.lib.join("d.jpg")).unwrap(), jpg);
    assert_eq!(src.thumbnail(&SourceId("d.jpg".into())).unwrap(), t);
}

#[test]
fn language_is_stored_in_shell_settings_and_survives_restart() {
    let e = env("lang");
    assert_eq!(e.sh.language(), None, "aldri valgt");
    assert_eq!(e.sh.set_language("en").unwrap(), "en");
    assert_eq!(e.sh.language().as_deref(), Some("en"));
    // Valget overlever omstart og overskrives uten å miste andre innstillinger.
    e.sh.set_library(e.lib.to_str().unwrap()).unwrap();
    e.sh.set_language("system").unwrap();
    let again = Shell::with_data_dir(&e.data, e.bin.clone() as Arc<dyn RecycleBin>);
    assert_eq!(again.language().as_deref(), Some("system"));
    assert_eq!(again.library().as_deref(), e.lib.to_str());
    assert!(!again.first_run());
}

#[test]
fn language_rejects_unknown_values_and_mock_has_no_storage() {
    let e = env("lang-bad");
    for bad in ["", "pseudo", "nn", "de", "EN", "nb "] {
        assert_eq!(
            e.sh.set_language(bad).unwrap_err().code,
            "bad_request",
            "{bad}"
        );
    }
    assert_eq!(e.sh.language(), None, "avvist verdi lagres ikke");
    let mock = Shell::mock();
    assert_eq!(mock.set_language("nb").unwrap_err().code, "unsupported");
    assert_eq!(mock.language(), None);
}

#[test]
fn unknown_language_in_file_counts_as_not_chosen() {
    let e = env("lang-file");
    fs::write(
        e.data.join("shell.json"),
        br#"{"library":null,"language":"klingon"}"#,
    )
    .unwrap();
    assert_eq!(e.sh.language(), None);
    // Gamle filer uten feltet leses fortsatt.
    fs::write(
        e.data.join("shell.json"),
        br#"{"library":null,"onboarded":true}"#,
    )
    .unwrap();
    assert_eq!(e.sh.language(), None);
    assert!(!e.sh.first_run());
}

/// 8x6 px JPEG (generert én gang; testen trenger ingen bildebibliotek).
const TINY_JPEG: [u8; 632] = [
    255, 216, 255, 224, 0, 16, 74, 70, 73, 70, 0, 1, 1, 0, 0, 1, 0, 1, 0, 0, 255, 219, 0, 67, 0,
    16, 11, 12, 14, 12, 10, 16, 14, 13, 14, 18, 17, 16, 19, 24, 40, 26, 24, 22, 22, 24, 49, 35, 37,
    29, 40, 58, 51, 61, 60, 57, 51, 56, 55, 64, 72, 92, 78, 64, 68, 87, 69, 55, 56, 80, 109, 81,
    87, 95, 98, 103, 104, 103, 62, 77, 113, 121, 112, 100, 120, 92, 101, 103, 99, 255, 219, 0, 67,
    1, 17, 18, 18, 24, 21, 24, 47, 26, 26, 47, 99, 66, 56, 66, 99, 99, 99, 99, 99, 99, 99, 99, 99,
    99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99,
    99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 255, 192, 0, 17, 8, 0, 6,
    0, 8, 3, 1, 34, 0, 2, 17, 1, 3, 17, 1, 255, 196, 0, 31, 0, 0, 1, 5, 1, 1, 1, 1, 1, 1, 0, 0, 0,
    0, 0, 0, 0, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 255, 196, 0, 181, 16, 0, 2, 1, 3, 3, 2, 4, 3,
    5, 5, 4, 4, 0, 0, 1, 125, 1, 2, 3, 0, 4, 17, 5, 18, 33, 49, 65, 6, 19, 81, 97, 7, 34, 113, 20,
    50, 129, 145, 161, 8, 35, 66, 177, 193, 21, 82, 209, 240, 36, 51, 98, 114, 130, 9, 10, 22, 23,
    24, 25, 26, 37, 38, 39, 40, 41, 42, 52, 53, 54, 55, 56, 57, 58, 67, 68, 69, 70, 71, 72, 73, 74,
    83, 84, 85, 86, 87, 88, 89, 90, 99, 100, 101, 102, 103, 104, 105, 106, 115, 116, 117, 118, 119,
    120, 121, 122, 131, 132, 133, 134, 135, 136, 137, 138, 146, 147, 148, 149, 150, 151, 152, 153,
    154, 162, 163, 164, 165, 166, 167, 168, 169, 170, 178, 179, 180, 181, 182, 183, 184, 185, 186,
    194, 195, 196, 197, 198, 199, 200, 201, 202, 210, 211, 212, 213, 214, 215, 216, 217, 218, 225,
    226, 227, 228, 229, 230, 231, 232, 233, 234, 241, 242, 243, 244, 245, 246, 247, 248, 249, 250,
    255, 196, 0, 31, 1, 0, 3, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0, 0, 0, 0, 0, 0, 1, 2, 3, 4, 5, 6, 7, 8,
    9, 10, 11, 255, 196, 0, 181, 17, 0, 2, 1, 2, 4, 4, 3, 4, 7, 5, 4, 4, 0, 1, 2, 119, 0, 1, 2, 3,
    17, 4, 5, 33, 49, 6, 18, 65, 81, 7, 97, 113, 19, 34, 50, 129, 8, 20, 66, 145, 161, 177, 193, 9,
    35, 51, 82, 240, 21, 98, 114, 209, 10, 22, 36, 52, 225, 37, 241, 23, 24, 25, 26, 38, 39, 40,
    41, 42, 53, 54, 55, 56, 57, 58, 67, 68, 69, 70, 71, 72, 73, 74, 83, 84, 85, 86, 87, 88, 89, 90,
    99, 100, 101, 102, 103, 104, 105, 106, 115, 116, 117, 118, 119, 120, 121, 122, 130, 131, 132,
    133, 134, 135, 136, 137, 138, 146, 147, 148, 149, 150, 151, 152, 153, 154, 162, 163, 164, 165,
    166, 167, 168, 169, 170, 178, 179, 180, 181, 182, 183, 184, 185, 186, 194, 195, 196, 197, 198,
    199, 200, 201, 202, 210, 211, 212, 213, 214, 215, 216, 217, 218, 226, 227, 228, 229, 230, 231,
    232, 233, 234, 242, 243, 244, 245, 246, 247, 248, 249, 250, 255, 218, 0, 12, 3, 1, 0, 2, 17, 3,
    17, 0, 63, 0, 109, 20, 81, 94, 89, 236, 159, 255, 217,
];

/// Bibliotek i en mappe som heter «OneDrive»: skysynket, så papirkurv nektes.
#[test]
fn cloud_synced_library_refuses_trash_but_allows_keep() {
    let e = env("cloud");
    let lib = e.root.join("OneDrive").join("bilder");
    fs::create_dir_all(&lib).unwrap();
    fs::write(lib.join("a.jpg"), b"same-bytes-same-bytes").unwrap();
    fs::write(lib.join("b.jpg"), b"same-bytes-same-bytes").unwrap();
    let src = FsSource::new(&lib).without_decoding();
    e.sh.open_library(&lib, Arc::new(src)).unwrap();
    assert_eq!(e.sh.state().cloud_sync, Some("onedrive"));
    e.sh.start_scan(true).unwrap();
    let t = Instant::now();
    while e.sh.state().scan_phase != "done" {
        assert!(
            t.elapsed() < Duration::from_secs(20),
            "skanning ble ikke ferdig"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
    let ids = e.dup_ids();
    assert!(!ids.is_empty(), "skanning og forslag virker som før");

    let err = e.sh.decide(&ids, DecisionDto::Trash).unwrap_err();
    assert_eq!(err.code, "cloud_synced_readonly");
    assert_eq!(err.detail, "onedrive");
    assert_eq!(e.sh.state().trash_count, 0);
    assert_eq!(quarantined(&lib), 0);
    assert!(exists(&lib.join("a.jpg")) && exists(&lib.join("b.jpg")));
    assert_eq!(e.dup_ids(), ids, "kortene er fortsatt ventende");

    e.sh.decide(&ids, DecisionDto::Keep).unwrap();
    assert_eq!(quarantined(&lib), 0);
    assert!(exists(&lib.join("a.jpg")) && exists(&lib.join("b.jpg")));
}

#[test]
fn local_library_has_no_cloud_sync_flag() {
    let e = env("nocloud");
    e.open();
    assert_eq!(e.sh.state().cloud_sync, None);
    assert_eq!(Shell::mock().state().cloud_sync, None);
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
