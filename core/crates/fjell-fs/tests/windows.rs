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
fn long_path_over_max_path_is_recycled_or_rejected_untouched() {
    let root = tempdir("w-long");
    let rel = deep_rel();
    let dir = root.join(&rel);
    assert!(dir.to_str().unwrap().len() > 260);
    fs::create_dir_all(ext(&dir)).unwrap();
    let file = dir.join("lang.jpg");
    fs::write(ext(&file), jpeg(8, 8, None)).unwrap();
    assert!(file.to_str().unwrap().len() > 260);
    // Papirkurv-APIet tar ikke stier over MAX_PATH. Godkjent utfall: enten Papirkurv via 8.3-kortnavn
    // (filen er borte), eller `Unsupported` og filen urørt. Aldri `Failed`, aldri tapt fil.
    match WindowsRecycleBin.recycle(&file) {
        Ok(()) => assert!(!ext(&file).exists(), "meldt Ok, men filen ligger der"),
        Err(RecycleError::Unsupported) => {
            assert!(ext(&file).exists(), "Unsupported skal la filen ligge")
        }
        other => panic!("uventet utfall for lang sti: {other:?}"),
    }
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

// --- WIC-dekoding (HEIC/RAW). Aldri kjørt: bare kompilert lokalt mot windows-gnu (uten linking);
// første ekte kjøring er windows-latest. Ingen ekte HEIC/RAW-fil brukes. Se docs/mvp/DEKODING.md.

/// Sender WIC-dekoderen under en endelse `image`-stien ikke tar, så WIC dekoder en vanlig JPEG.
struct WicAsWj;

impl fjell_fs::thumb::FrameDecoder for WicAsWj {
    fn extensions(&self) -> &[&str] {
        &["wj"]
    }
    fn probe(&self, p: &Path) -> Result<(u32, u32, u8), fjell_core::model::SourceError> {
        fjell_fs::thumb::FrameDecoder::probe(&fjell_fs::wic::WicDecoder, p)
    }
    fn decode(
        &self,
        p: &Path,
        m: u32,
    ) -> Result<fjell_fs::thumb::Decoded, fjell_core::model::SourceError> {
        fjell_fs::thumb::FrameDecoder::decode(&fjell_fs::wic::WicDecoder, p, m)
    }
}

#[test]
fn wic_jpeg_and_image_jpeg_hash_within_tolerance() {
    use fjell_core::hash::perceptual;
    use fjell_fs::thumb::{make_thumb, make_thumb_with, oriented_dims, oriented_dims_with};
    let root = tempdir("w-wic-jpeg");
    for (name, o) in [("plain", None), ("rot6", Some(6u8))] {
        let bytes = jpeg(640, 360, o);
        write(&root, &format!("{name}.jpg"), &bytes);
        write(&root, &format!("{name}.wj"), &bytes);
        let a = root.join(format!("{name}.jpg"));
        let b = root.join(format!("{name}.wj"));
        assert_eq!(
            oriented_dims(&a),
            oriented_dims_with(&b, Some(&WicAsWj)),
            "{name}: dimensjoner"
        );
        let ta = make_thumb(&a).unwrap();
        let tb = make_thumb_with(&b, Some(&WicAsWj)).unwrap();
        assert_eq!((ta.w, ta.h), (tb.w, tb.h), "{name}: miniatyrstørrelse");
        let ((da, pa), (db, pb)) = (perceptual(&ta), perceptual(&tb));
        // Toleransen (4 av 64 biter) er et anslag, ikke målt; justeres etter første CI-kjøring.
        assert!(da.hamming(db) <= 4, "{name}: dHash {}", da.hamming(db));
        assert!(pa.hamming(pb) <= 4, "{name}: pHash {}", pa.hamming(pb));
    }
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn wic_garbage_heic_never_panics_and_is_unavailable_or_corrupt() {
    use fjell_core::model::SourceError;
    use fjell_fs::thumb::make_thumb;
    let root = tempdir("w-wic-garbage");
    write(&root, "a.heic", b"dette er ikke et bilde");
    write(&root, "b.dng", b"");
    for n in ["a.heic", "b.dng"] {
        // Uten HEIF-utvidelse: Unavailable. Med: Corrupt. Aldri Ok, aldri panikk.
        let e = make_thumb(&root.join(n)).unwrap_err();
        assert!(
            matches!(e, SourceError::Unavailable | SourceError::Corrupt),
            "{n}: {e:?}"
        );
    }
    assert_eq!(
        make_thumb(&root.join("finnes-ikke.heic")).unwrap_err(),
        SourceError::NotFound
    );
    let _ = fs::remove_dir_all(&root);
}
