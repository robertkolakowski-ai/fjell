//! Dekoder-grensesnittet (`thumb::FrameDecoder`): `image`-stien er uendret, ekstra dekodere kobles
//! inn bak trekket, og manglende kodek gir `Unavailable`. Kjører på Linux med en fake-dekoder;
//! ekte WIC testes i `tests/windows.rs` (bare på windows-latest).

mod common;
use common::*;
use fjell_core::model::{SourceError, ThumbFmt};
use fjell_fs::thumb::{
    is_decodable, make_thumb, make_thumb_with, oriented_dims, oriented_dims_with, Decoded,
    FrameDecoder,
};
use image::metadata::Orientation;
use image::{DynamicImage, ImageDecoder, ImageReader};
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};

/// Fake som «dekoder» heic: svarer med oppgitt resultat og teller kall.
struct Fake {
    calls: AtomicUsize,
    result: Result<(u32, u32, u8), SourceError>,
}

impl Fake {
    fn ok(w: u32, h: u32, o: u8) -> Fake {
        Fake {
            calls: AtomicUsize::new(0),
            result: Ok((w, h, o)),
        }
    }
    fn err(e: SourceError) -> Fake {
        Fake {
            calls: AtomicUsize::new(0),
            result: Err(e),
        }
    }
}

impl FrameDecoder for Fake {
    fn extensions(&self) -> &[&str] {
        &["heic"]
    }
    fn probe(&self, _: &Path) -> Result<(u32, u32, u8), SourceError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.result.clone()
    }
    fn decode(&self, _: &Path, _: u32) -> Result<Decoded, SourceError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        let (w, h, o) = self.result.clone()?;
        // Venstre halvdel rød, høyre blå (som `common::jpeg`).
        let mut rgba = Vec::new();
        for _ in 0..h {
            for x in 0..w {
                rgba.extend_from_slice(if x < w / 2 {
                    &[255, 0, 0, 255]
                } else {
                    &[0, 0, 255, 255]
                });
            }
        }
        Ok(Decoded {
            w,
            h,
            rgba,
            exif_orientation: o,
        })
    }
}

/// Referanse: nøyaktig den gamle `image`-stien (dekod, nedskaler, orientering, RGBA8).
fn reference(path: &Path) -> (u32, u32, Vec<u8>) {
    let mut dec = ImageReader::open(path)
        .unwrap()
        .with_guessed_format()
        .unwrap()
        .into_decoder()
        .unwrap();
    let o = dec.orientation().unwrap_or(Orientation::NoTransforms);
    let img = DynamicImage::from_decoder(dec).unwrap();
    let mut small = if img.width().max(img.height()) > 256 {
        img.thumbnail(256, 256)
    } else {
        img
    };
    small.apply_orientation(o);
    let r = small.to_rgba8();
    (r.width(), r.height(), r.into_raw())
}

fn png(w: u32, h: u32) -> Vec<u8> {
    let img = image::RgbaImage::from_fn(w, h, |x, y| image::Rgba([x as u8, y as u8, 7, 255]));
    let mut out = std::io::Cursor::new(Vec::new());
    img.write_to(&mut out, image::ImageFormat::Png).unwrap();
    out.into_inner()
}

#[test]
fn jpeg_and_png_paths_are_byte_identical_to_image_crate() {
    let root = tempdir("dec-image");
    write(&root, "a.jpg", &jpeg(600, 300, Some(6)));
    write(&root, "b.png", &png(40, 30));
    let fake = Fake::ok(10, 10, 1);
    for name in ["a.jpg", "b.png"] {
        let p = root.join(name);
        let (w, h, data) = reference(&p);
        for t in [
            make_thumb(&p).unwrap(),
            make_thumb_with(&p, Some(&fake)).unwrap(),
        ] {
            assert_eq!((t.w, t.h, t.fmt), (w, h, ThumbFmt::Rgba8), "{name}");
            assert_eq!(t.data, data, "{name}");
        }
    }
    assert_eq!(oriented_dims(&root.join("a.jpg")), Some((300, 600)));
    assert_eq!(
        fake.calls.load(Ordering::SeqCst),
        0,
        "image-stien velges først"
    );
    let _ = std::fs::remove_dir_all(&root);
}

#[cfg(not(windows))]
#[test]
fn heic_and_raw_are_unavailable_without_wic() {
    let root = tempdir("dec-unavail");
    for name in ["a.heic", "b.heif", "c.dng", "d.cr2", "e.nef", "f.arw"] {
        write(&root, name, b"ikke en ekte fil");
        let p = root.join(name);
        assert!(!is_decodable(&p), "{name}");
        assert_eq!(
            make_thumb(&p).unwrap_err(),
            SourceError::Unavailable,
            "{name}"
        );
        assert_eq!(oriented_dims(&p), None, "{name}");
    }
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn fake_decoder_gets_orientation_applied_and_is_downscaled() {
    let root = tempdir("dec-fake");
    write(&root, "a.heic", b"x");
    let p = root.join("a.heic");
    // 600x300 med orientering 6 (Rotate90): 300x600 etter orientering, så ned til 128x256.
    let fake = Fake::ok(600, 300, 6);
    assert_eq!(oriented_dims_with(&p, Some(&fake)), Some((300, 600)));
    let t = make_thumb_with(&p, Some(&fake)).unwrap();
    assert_eq!((t.w, t.h), (128, 256));
    assert_eq!(t.data.len(), 128 * 256 * 4);
    // Rotate90 flytter venstre (rød) halvdel til toppen.
    assert_eq!(&t.data[0..4], &[255, 0, 0, 255]);
    let last = t.data.len() - 4;
    assert_eq!(&t.data[last..], &[0, 0, 255, 255]);
    // Uten orientering og under grensen: uendret størrelse.
    let t = make_thumb_with(&p, Some(&Fake::ok(20, 10, 1))).unwrap();
    assert_eq!((t.w, t.h), (20, 10));
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn decoder_errors_pass_through_and_missing_decoder_is_unavailable() {
    let root = tempdir("dec-err");
    write(&root, "a.heic", b"x");
    let p = root.join("a.heic");
    for e in [
        SourceError::Unavailable,
        SourceError::Corrupt,
        SourceError::NotFound,
    ] {
        assert_eq!(
            make_thumb_with(&p, Some(&Fake::err(e.clone()))).unwrap_err(),
            e
        );
        assert_eq!(oriented_dims_with(&p, Some(&Fake::err(e))), None);
    }
    assert_eq!(
        make_thumb_with(&p, None).unwrap_err(),
        SourceError::Unavailable
    );
    // Ukjent endelse går aldri til den ekstra dekoderen.
    write(&root, "a.xyz", b"x");
    let fake = Fake::ok(4, 4, 1);
    assert_eq!(
        make_thumb_with(&root.join("a.xyz"), Some(&fake)).unwrap_err(),
        SourceError::Unavailable
    );
    assert_eq!(fake.calls.load(Ordering::SeqCst), 0);
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn inconsistent_frame_buffer_is_corrupt_not_panic() {
    struct Bad;
    impl FrameDecoder for Bad {
        fn extensions(&self) -> &[&str] {
            &["heic"]
        }
        fn probe(&self, _: &Path) -> Result<(u32, u32, u8), SourceError> {
            Ok((4, 4, 1))
        }
        fn decode(&self, _: &Path, _: u32) -> Result<Decoded, SourceError> {
            Ok(Decoded {
                w: 4,
                h: 4,
                rgba: vec![0; 3],
                exif_orientation: 1,
            })
        }
    }
    let root = tempdir("dec-bad");
    write(&root, "a.heic", b"x");
    assert_eq!(
        make_thumb_with(&root.join("a.heic"), Some(&Bad)).unwrap_err(),
        SourceError::Corrupt
    );
    let _ = std::fs::remove_dir_all(&root);
}
