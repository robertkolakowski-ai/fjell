//! Miniatyrer og dimensjoner: dekoding i adapteren (ARKITEKTUR beslutning 6), kjernen har ingen
//! bildeformat-avhengighet.
//!
//! Grensesnitt:
//! - `is_decodable`: formater vi kan dekode. `image`-craten tar JPEG og PNG på alle plattformer;
//!   HEIC/HEIF og RAW (heic, heif, dng, cr2, nef, arw ...) tas bare på Windows via WIC (`wic.rs`).
//! - `oriented_dims`: bredde/høyde etter EXIF-orientering, uten full dekoding.
//! - `make_thumb`: 256 px RGBA-miniatyr, bildeforhold bevart, orientering brukt.
//! - `FrameDecoder`: trekk for en ekstra dekoder (WIC på Windows, fake i tester). `image`-stien er
//!   uendret og velges alltid først for jpg/jpeg/png.
//!
//! Feil: ukjent/ikke-støttet format, eller manglende kodek, gir `Unavailable`; ødelagt data i et
//! støttet format gir `Corrupt`. Se `docs/mvp/DEKODING.md`.

use fjell_core::model::{SourceError, Thumb, ThumbFmt, THUMB_MAX_SIDE};
use image::metadata::Orientation;
use image::{DynamicImage, ImageDecoder, ImageReader};
use std::path::Path;

fn ext_of(path: &Path) -> Option<String> {
    path.extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase)
}

/// Filendelser `image`-craten dekoder (alle plattformer).
fn is_image_crate(path: &Path) -> bool {
    matches!(ext_of(path).as_deref(), Some("jpg" | "jpeg" | "png"))
}

/// Filendelser WIC forsøkes på (bare Windows). Kodeken kan mangle; da gir dekoding `Unavailable`.
pub const WIC_EXTENSIONS: &[&str] = &[
    "heic", "heif", "dng", "cr2", "cr3", "nef", "arw", "orf", "rw2", "raf",
];

/// Et dekodet bilde fra en `FrameDecoder`: RGBA8 uten orientering brukt.
pub struct Decoded {
    pub w: u32,
    pub h: u32,
    /// `w * h * 4` byte. Kan være nedskalert av dekoderen; `make_thumb_with` skalerer uansett ned.
    pub rgba: Vec<u8>,
    /// EXIF-orientering 1..=8 (1 hvis ukjent).
    pub exif_orientation: u8,
}

/// Dekoder for formater `image`-craten ikke tar. Må ikke panikke; mangler kodeken skal den gi
/// `Unavailable`, ødelagt fil `Corrupt`, manglende fil `NotFound`.
pub trait FrameDecoder: Send + Sync {
    /// Filendelser (små bokstaver) denne dekoderen forsøkes på.
    fn extensions(&self) -> &[&str];
    /// Kildedimensjoner (uten orientering) og EXIF-orientering, uten full dekoding.
    fn probe(&self, path: &Path) -> Result<(u32, u32, u8), SourceError>;
    /// Første ramme som RGBA8. `max_side` er et hint om at større enn dette kan nedskaleres.
    fn decode(&self, path: &Path, max_side: u32) -> Result<Decoded, SourceError>;
}

/// Plattformens ekstra dekoder: WIC på Windows, ingen ellers.
pub fn platform_decoder() -> Option<&'static dyn FrameDecoder> {
    #[cfg(windows)]
    {
        Some(&crate::wic::WicDecoder)
    }
    #[cfg(not(windows))]
    {
        None
    }
}

fn extra_for<'a>(path: &Path, extra: Option<&'a dyn FrameDecoder>) -> Option<&'a dyn FrameDecoder> {
    let ext = ext_of(path)?;
    extra.filter(|d| d.extensions().contains(&ext.as_str()))
}

/// Filendelser adapteren kan dekode (på denne plattformen).
pub fn is_decodable(path: &Path) -> bool {
    is_image_crate(path) || extra_for(path, platform_decoder()).is_some()
}

fn swaps_axes(o: Orientation) -> bool {
    matches!(
        o,
        Orientation::Rotate90
            | Orientation::Rotate270
            | Orientation::Rotate90FlipH
            | Orientation::Rotate270FlipH
    )
}

fn open(path: &Path) -> Result<impl ImageDecoder, SourceError> {
    ImageReader::open(path)
        .map_err(|e| match e.kind() {
            std::io::ErrorKind::NotFound => SourceError::NotFound,
            _ => SourceError::Failed(e.to_string()),
        })?
        .with_guessed_format()
        .map_err(|e| SourceError::Failed(e.to_string()))?
        .into_decoder()
        .map_err(|_| SourceError::Corrupt)
}

/// Dimensjoner etter orientering. `None` hvis formatet ikke dekodes eller headeren er ugyldig.
pub fn oriented_dims(path: &Path) -> Option<(u32, u32)> {
    oriented_dims_with(path, platform_decoder())
}

/// Som `oriented_dims`, med eksplisitt ekstra dekoder (tester).
pub fn oriented_dims_with(path: &Path, extra: Option<&dyn FrameDecoder>) -> Option<(u32, u32)> {
    if is_image_crate(path) {
        let mut d = open(path).ok()?;
        let (w, h) = d.dimensions();
        let o = d.orientation().unwrap_or(Orientation::NoTransforms);
        return Some(if swaps_axes(o) { (h, w) } else { (w, h) });
    }
    let (w, h, o) = extra_for(path, extra)?.probe(path).ok()?;
    let o = Orientation::from_exif(o).unwrap_or(Orientation::NoTransforms);
    Some(if swaps_axes(o) { (h, w) } else { (w, h) })
}

/// 256 px miniatyr (RGBA8) med orientering brukt.
pub fn make_thumb(path: &Path) -> Result<Thumb, SourceError> {
    make_thumb_with(path, platform_decoder())
}

/// Som `make_thumb`, med eksplisitt ekstra dekoder (tester).
pub fn make_thumb_with(
    path: &Path,
    extra: Option<&dyn FrameDecoder>,
) -> Result<Thumb, SourceError> {
    if is_image_crate(path) {
        let mut dec = open(path)?;
        let orientation = dec.orientation().unwrap_or(Orientation::NoTransforms);
        let img = DynamicImage::from_decoder(dec).map_err(|_| SourceError::Corrupt)?;
        return Ok(finish(img, orientation));
    }
    // Fil finnes (sjekket av kalleren), men uten dekoder: ikke «ødelagt».
    let d = extra_for(path, extra).ok_or(SourceError::Unavailable)?;
    let frame = d.decode(path, THUMB_MAX_SIDE)?;
    let img = image::RgbaImage::from_raw(frame.w, frame.h, frame.rgba)
        .map(DynamicImage::ImageRgba8)
        .ok_or(SourceError::Corrupt)?;
    let o = Orientation::from_exif(frame.exif_orientation).unwrap_or(Orientation::NoTransforms);
    Ok(finish(img, o))
}

fn finish(img: DynamicImage, orientation: Orientation) -> Thumb {
    let mut small = if img.width().max(img.height()) > THUMB_MAX_SIDE {
        img.thumbnail(THUMB_MAX_SIDE, THUMB_MAX_SIDE)
    } else {
        img
    };
    small.apply_orientation(orientation);
    let rgba = small.to_rgba8();
    Thumb {
        w: rgba.width(),
        h: rgba.height(),
        fmt: ThumbFmt::Rgba8,
        data: rgba.into_raw(),
    }
}
