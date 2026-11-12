//! Ekte Windows-tester (`cfg(windows)`): Papirkurv, lange stier, UNC, stasjoner og karantene.
//! Kjøres bare i CI-jobben `desktop` (windows-latest); er aldri kjørt på Linux.
//! Tester skriver kun til egne temp-mapper og røre aldri andre filer.
#![cfg(windows)]

mod common;
use common::*;
use fjell_core::model::{AssetId, BatchId, SourceId};
use fjell_fs::delete::{RecycleBin, RecycleError};
use fjell_fs::drives::{drive_status, list_drives, DriveKind, DriveStatus};
use fjell_fs::path::{extended_str, strip_extended};
use fjell_fs::windows::WindowsRecycleBin;
use fjell_fs::{DeleteError, DeleteMode, FakeRecycleBin, FsDeleter, FsQuarantine, Quarantine};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

fn ext(p: &Path) -> PathBuf {
    PathBuf::from(extended_str(p.to_str().unwrap(), true))
}

/// Mappe dypere enn MAX_PATH (260) under `root`, som relativ `/`-sti.
fn deep_rel() -> String {
    (0..8)
        .map(|i| format!("nivaa{i}-{}", "x".repeat(34)))
        .collect::<Vec<_>>()
        .join("/")
}

#[test]
fn tempfile_goes_to_recycle_bin_and_is_gone() {
    let root = tempdir("w-recycle");
    write(&root, "a.jpg", &jpeg(8, 8, None));
    let p = root.join("a.jpg");
    assert_eq!(WindowsRecycleBin.recycle(&p), Ok(()));
    assert!(!p.exists(), "borte fra opprinnelig sted");
    // Papirkurv-bevis: FOFX_RECYCLEONDELETE gjør at operasjonen feiler i stedet for å slette permanent.
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn missing_file_is_failed_not_ok() {
    let root = tempdir("w-missing");
    let r = WindowsRecycleBin.recycle(&root.join("finnes-ikke.jpg"));
    assert!(matches!(r, Err(RecycleError::Failed(_))), "{r:?}");
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn long_path_over_max_path_goes_to_recycle_bin() {
    let root = tempdir("w-long");
    let rel = deep_rel();
    let dir = root.join(&rel);
    assert!(dir.to_str().unwrap().len() > 260);
    fs::create_dir_all(ext(&dir)).unwrap();
    let file = dir.join("lang.jpg");
    fs::write(ext(&file), jpeg(8, 8, None)).unwrap();
    assert!(file.to_str().unwrap().len() > 260);
    assert_eq!(WindowsRecycleBin.recycle(&file), Ok(()));
    assert!(!ext(&file).exists());
    let _ = fs::remove_dir_all(ext(&root));
}

#[test]
fn permanent_requires_ack_and_nothing_is_deleted_without_it() {
    let root = tempdir("w-ack");
    write(&root, "a.jpg", b"x");
    let bin = Arc::new(FakeRecycleBin::default());
    let r = FsDeleter::new(&root, None, bin.clone(), DeleteMode::Permanent, None);
    assert!(matches!(r, Err(DeleteError::AckRequired)));
    assert!(root.join("a.jpg").exists());
    assert!(bin.calls.lock().unwrap().is_empty());
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn unc_path_has_no_recycle_bin_and_file_stays() {
    let root = tempdir("w-unc");
    write(&root, "a.jpg", b"unc");
    let s = root.to_str().unwrap();
    let b = s.as_bytes();
    if b.len() < 3 || b[1] != b':' {
        eprintln!("SKIPPED unc: temp-mappen er ikke på en stasjonsbokstav: {s}");
        return;
    }
    // Administrativ deling \\localhost\C$\...
    let unc = PathBuf::from(format!(r"\\localhost\{}${}", &s[..1], &s[2..]));
    let file = unc.join("a.jpg");
    if fs::metadata(ext(&file)).is_err() {
        eprintln!(
            "SKIPPED unc: {} er ikke nåbar (admin-deling av)",
            file.display()
        );
        return;
    }
    assert_eq!(fs::read(ext(&file)).unwrap(), b"unc");
    assert_eq!(
        WindowsRecycleBin.recycle(&file),
        Err(RecycleError::Unsupported)
    );
    assert!(root.join("a.jpg").exists(), "ikke stille slettet");
    assert!(strip_extended(&extended_str(file.to_str().unwrap(), true)).starts_with(r"\\localhost"));
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn system_drive_is_fixed_and_unused_letter_is_unavailable() {
    let root = tempdir("w-drive");
    let s = root.to_str().unwrap();
    let letter = s[..1].to_ascii_uppercase();
    assert_eq!(
        drive_status(&format!("{letter}:\\")),
        DriveStatus::Online(DriveKind::Fixed)
    );
    let used: Vec<String> = list_drives().into_iter().map(|(r, _)| r).collect();
    assert!(used.contains(&format!("{letter}:\\")));
    let free = ('D'..='Z')
        .map(|c| format!("{c}:\\"))
        .find(|r| !used.contains(r))
        .expect("ledig stasjonsbokstav");
    assert_eq!(drive_status(&free), DriveStatus::Unavailable);
    // Frakoblet rot: ingen Papirkurv-forsøk, ingenting meldes slettet.
    let r = WindowsRecycleBin.recycle(Path::new(&format!("{free}a.jpg")));
    assert!(r.is_err(), "{r:?}");
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn quarantine_rename_same_volume_on_long_path_keeps_content() {
    let root = tempdir("w-quar");
    let rel = format!("{}/lang.jpg", deep_rel());
    let full = root.join(&rel);
    fs::create_dir_all(ext(full.parent().unwrap())).unwrap();
    let bytes = jpeg(16, 16, None);
    fs::write(ext(&full), &bytes).unwrap();
    let hash = blake3::hash(&bytes);

    let q = FsQuarantine::in_library(&root);
    let items = vec![(AssetId(1), SourceId(rel.clone()))];
    let r = q.stage(BatchId(1), &items);
    assert!(r.iter().all(|(_, x)| x.is_ok()), "{r:?}");
    assert!(!ext(&full).exists());
    let staged = q.locate(AssetId(1)).unwrap();
    assert_eq!(blake3::hash(&fs::read(ext(&staged)).unwrap()), hash);

    let u = q.unstage(&[AssetId(1)]);
    assert_eq!(u[0].1, Ok(SourceId(rel)));
    assert_eq!(blake3::hash(&fs::read(ext(&full)).unwrap()), hash);
    let _ = fs::remove_dir_all(ext(&root));
}
