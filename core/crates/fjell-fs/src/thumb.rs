//! Miniatyrer og dimensjoner: dekoding i adapteren (ARKITEKTUR beslutning 6), kjernen har ingen
//! bildeformat-avhengighet.
//!
//! Grensesnitt:
//! - `is_decodable`: formater vi dekoder selv (JPEG, PNG). HEIC/RAW krever Windows-kodek (senere).
//! - `oriented_dims`: bredde/høyde etter EXIF-orientering, uten full dekoding.
//! - `make_thumb`: 256 px RGBA-miniatyr, bildeforhold bevart, orientering brukt.
//!
//! Feil: ukjent/ikke-støttet format gir `Unavailable`; ødelagt data i et støttet format gir `Corrupt`.

use fjell_core::model::{SourceError, Thumb, ThumbFmt, THUMB_MAX_SIDE};
use image::metadata::Orientation;
use image::{DynamicImage, ImageDecoder, ImageReader};
use std::path::Path;

/// Filendelser adapteren dekoder selv.
pub fn is_decodable(path: &Path) -> bool {
    matches!(
        path.extension()
            .and_then(|e| e.to_str())
            .map(str::to_ascii_lowercase)
            .as_deref(),
        Some("jpg" | "jpeg" | "png")
    )
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
    if !is_decodable(path) {
        return None;
    }
    let mut d = open(path).ok()?;
    let (w, h) = d.dimensions();
    let o = d.orientation().unwrap_or(Orientation::NoTransforms);
    Some(if swaps_axes(o) { (h, w) } else { (w, h) })
}

/// 256 px miniatyr (RGBA8) med orientering brukt.
pub fn make_thumb(path: &Path) -> Result<Thumb, SourceError> {
    if !is_decodable(path) {
        // Fil finnes (sjekket av kalleren), men vi har ingen dekoder: ikke «ødelagt».
        return Err(SourceError::Unavailable);
    }
    let mut dec = open(path)?;
    let orientation = dec.orientation().unwrap_or(Orientation::NoTransforms);
    let img = DynamicImage::from_decoder(dec).map_err(|_| SourceError::Corrupt)?;
    let mut small = if img.width().max(img.height()) > THUMB_MAX_SIDE {
        img.thumbnail(THUMB_MAX_SIDE, THUMB_MAX_SIDE)
    } else {
        img
    };
    small.apply_orientation(orientation);
    let rgba = small.to_rgba8();
    Ok(Thumb {
        w: rgba.width(),
        h: rgba.height(),
        fmt: ThumbFmt::Rgba8,
        data: rgba.into_raw(),
    })
}
