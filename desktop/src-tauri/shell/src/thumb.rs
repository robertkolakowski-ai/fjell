//! Miniatyrer over Tauri-grensen: JPEG-koding, base64 og feilkoder.
//!
//! Avhenger av: `fjell-core::model` (`Thumb`, `SourceError`), `image` (kun JPEG-koding), `base64`.
//! Grensesnitt: `encode`, `mock`, `map_error`.
//!
//! Bildedata forlater aldri maskinen: bare IPC og minne, ingenting skrives til disk her
//! (bufferen i `fjell-fs::thumbcache` ligger i app-data og eies av kilden).

use crate::dto::{ErrorDto, ThumbDto};
use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use fjell_core::model::{SourceError, Thumb, ThumbFmt};
use image::codecs::jpeg::JpegEncoder;
use image::ExtendedColorType;

const QUALITY: u8 = 80;

/// Koder `Thumb` (Gray8/Rgba8) som JPEG. Alfa legges over hvitt (JPEG har ingen alfa).
pub fn encode(t: &Thumb) -> Result<ThumbDto, ErrorDto> {
    let px = (t.w as usize).checked_mul(t.h as usize);
    if t.w == 0 || t.h == 0 || px.map(|p| p * t.fmt.bytes_per_pixel()) != Some(t.data.len()) {
        return Err(ErrorDto::new("corrupt", "ugyldig miniatyr"));
    }
    let (raw, color): (Vec<u8>, ExtendedColorType) = match t.fmt {
        ThumbFmt::Gray8 => (t.data.clone(), ExtendedColorType::L8),
        ThumbFmt::Rgba8 => {
            let mut rgb = Vec::with_capacity(t.data.len() / 4 * 3);
            for p in t.data.chunks_exact(4) {
                let a = u32::from(p[3]);
                for c in &p[..3] {
                    rgb.push(((u32::from(*c) * a + 255 * (255 - a) + 127) / 255) as u8);
                }
            }
            (rgb, ExtendedColorType::Rgb8)
        }
    };
    let mut out = Vec::new();
    JpegEncoder::new_with_quality(&mut out, QUALITY)
        .encode(&raw, t.w, t.h, color)
        .map_err(|e| ErrorDto::new("unavailable", e.to_string()))?;
    Ok(ThumbDto {
        mime: "image/jpeg",
        data: STANDARD.encode(out),
        width: t.w,
        height: t.h,
    })
}

/// Liten deterministisk miniatyr for mock-modus (gradient avledet av `id`).
pub fn mock(id: u64) -> Result<ThumbDto, ErrorDto> {
    let (w, h) = (32u32, 24u32);
    let seed = (id.wrapping_mul(2_654_435_761) % 256) as u32;
    let mut data = Vec::with_capacity((w * h * 4) as usize);
    for y in 0..h {
        for x in 0..w {
            data.extend([
                ((x * 8 + seed) % 256) as u8,
                ((y * 10 + seed / 2) % 256) as u8,
                ((x + y) * 4 % 256) as u8,
                255,
            ]);
        }
    }
    encode(&Thumb {
        w,
        h,
        fmt: ThumbFmt::Rgba8,
        data,
    })
}

/// `SourceError` til kontraktskodene. «Ikke lokal» (skyplassholder) avgjøres før kilden kalles.
pub fn map_error(e: SourceError) -> ErrorDto {
    match e {
        SourceError::Unavailable => ErrorDto::new("unavailable", ""),
        SourceError::Corrupt => ErrorDto::new("corrupt", ""),
        SourceError::NotFound => ErrorDto::new("not_found", ""),
        SourceError::Failed(m) => ErrorDto::new("unavailable", m),
    }
}
