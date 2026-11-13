//! Skannlisten (`SCAN_FORMATS`): HEIC/HEIF/AVIF, RAW og video er synlige, men dekodes bare der det
//! finnes dekoder. Uten dekoder: bredde/høyde 0, `thumbnail` = `Unavailable`, bytes kan leses.

mod common;
use common::*;
use fjell_core::model::{AssetSource, Kind, SourceError, SourceId};
use fjell_fs::source::{mime_and_kind, SCAN_FORMATS};
use fjell_fs::thumb::{Decoded, FrameDecoder, WIC_EXTENSIONS};
use fjell_fs::FsSource;
use std::path::Path;

const PHOTO: &[&str] = &[
    "jpg", "jpeg", "png", "heic", "heif", "avif", "dng", "cr2", "cr3", "nef", "arw", "orf", "rw2",
    "raf",
];
const VIDEO: &[&str] = &["mp4", "m4v", "mov", "avi", "mkv", "webm", "wmv", "3gp"];

#[test]
fn skannlisten_er_som_avtalt() {
    let kind = |k: Kind| -> Vec<&str> {
        SCAN_FORMATS
            .iter()
            .filter(|f| f.2 == k)
            .map(|f| f.0)
            .collect()
    };
    assert_eq!(kind(Kind::Photo), PHOTO);
    assert_eq!(kind(Kind::Video), VIDEO);
    assert_eq!(SCAN_FORMATS.len(), PHOTO.len() + VIDEO.len());
    for (ext, mime, k) in SCAN_FORMATS {
        let want = if *k == Kind::Video {
            "video/"
        } else {
            "image/"
        };
        assert!(mime.starts_with(want), "{ext}: {mime}");
        // Store bokstaver i filendelsen er samme format.
        let p = format!("x.{}", ext.to_uppercase());
        assert_eq!(mime_and_kind(Path::new(&p)), Some((*mime, *k)));
    }
    for no in [
        "a.txt",
        "a.gif",
        "a.pdf",
        "a",
        "a.jpg.tmp",
        "Thumbs.db",
        "a.xmp",
    ] {
        assert_eq!(mime_and_kind(Path::new(no)), None, "{no}");
    }
    // Alle bildeformater unntatt JPEG/PNG går til WIC på Windows; video aldri.
    for ext in &PHOTO[3..] {
        assert!(WIC_EXTENSIONS.contains(ext), "{ext}");
    }
    assert!(VIDEO.iter().all(|v| !WIC_EXTENSIONS.contains(v)));
}

fn library(tag: &str) -> std::path::PathBuf {
    let root = tempdir(tag);
    write(&root, "ok.jpg", &jpeg(30, 20, None));
    for ext in PHOTO[3..].iter().chain(VIDEO) {
        write(
            &root,
            &format!("f.{ext}"),
            format!("ikke ekte {ext}").as_bytes(),
        );
    }
    write(&root, "notat.txt", b"x");
    root
}

#[test]
fn uten_dekoder_er_heic_raw_og_video_synlige_men_unavailable() {
    let root = library("fmt-nodec");
    let src = FsSource::new(&root).with_frame_decoder(None);
    let set = src.list_changes(None).expect("skann må ikke feile");
    assert_eq!(set.changed.len(), 1 + PHOTO.len() - 3 + VIDEO.len());
    for m in &set.changed {
        let ext = m.source_id.0.rsplit('.').next().unwrap();
        if m.source_id.0 == "ok.jpg" {
            assert_eq!((m.width, m.height, m.kind), (30, 20, Kind::Photo));
            assert!(src.thumbnail(&m.source_id).is_ok());
            continue;
        }
        assert_eq!((m.width, m.height), (0, 0), "{ext}");
        assert!(m.bytes > 0, "størrelsen telles: {ext}");
        assert_eq!(m.kind == Kind::Video, VIDEO.contains(&ext), "{ext}");
        // Ikke feil og ikke «ødelagt»: Unavailable. Bytes kan fortsatt hashes.
        assert_eq!(
            src.thumbnail(&m.source_id).unwrap_err(),
            SourceError::Unavailable,
            "{ext}"
        );
        let mut b = Vec::new();
        src.read_bytes(&m.source_id)
            .unwrap()
            .read_to_end(&mut b)
            .unwrap();
        assert_eq!(b, format!("ikke ekte {ext}").as_bytes());
    }
}

/// Fake som «dekoder» alt WIC ville tatt: 40×10, EXIF-orientering 6 (roteres til 10×40).
struct Fake;
impl FrameDecoder for Fake {
    fn extensions(&self) -> &[&str] {
        WIC_EXTENSIONS
    }
    fn probe(&self, _: &Path) -> Result<(u32, u32, u8), SourceError> {
        Ok((40, 10, 6))
    }
    fn decode(&self, _: &Path, _: u32) -> Result<Decoded, SourceError> {
        Ok(Decoded {
            w: 40,
            h: 10,
            rgba: vec![128; 40 * 10 * 4],
            exif_orientation: 6,
        })
    }
}
static FAKE: Fake = Fake;

/// Fake der kodeken mangler (som WIC uten HEIF-utvidelsen).
struct NoCodec;
impl FrameDecoder for NoCodec {
    fn extensions(&self) -> &[&str] {
        WIC_EXTENSIONS
    }
    fn probe(&self, _: &Path) -> Result<(u32, u32, u8), SourceError> {
        Err(SourceError::Unavailable)
    }
    fn decode(&self, _: &Path, _: u32) -> Result<Decoded, SourceError> {
        Err(SourceError::Unavailable)
    }
}
static NO_CODEC: NoCodec = NoCodec;

#[test]
fn med_fake_dekoder_dekodes_heic_og_raw_men_aldri_video() {
    let root = library("fmt-fake");
    let src = FsSource::new(&root).with_frame_decoder(Some(&FAKE));
    for m in src.list_changes(None).unwrap().changed {
        let ext = m.source_id.0.rsplit('.').next().unwrap().to_string();
        if VIDEO.contains(&ext.as_str()) {
            assert_eq!((m.width, m.height), (0, 0), "{ext}");
            assert_eq!(
                src.thumbnail(&m.source_id).unwrap_err(),
                SourceError::Unavailable
            );
        } else if ext != "jpg" {
            assert_eq!((m.width, m.height), (10, 40), "orientering brukt: {ext}");
            let t = src.thumbnail(&m.source_id).unwrap();
            assert_eq!((t.w, t.h), (10, 40), "{ext}");
        }
    }
}

#[test]
fn manglende_kodek_gir_unavailable_uten_a_odelegge_skann() {
    let root = library("fmt-nocodec");
    let src = FsSource::new(&root).with_frame_decoder(Some(&NO_CODEC));
    let set = src.list_changes(None).unwrap();
    assert_eq!(set.changed.len(), 1 + PHOTO.len() - 3 + VIDEO.len());
    let heic = SourceId("f.heic".into());
    assert_eq!(src.thumbnail(&heic).unwrap_err(), SourceError::Unavailable);
    let m = set.changed.iter().find(|m| m.source_id == heic).unwrap();
    assert_eq!((m.width, m.height), (0, 0));
}
