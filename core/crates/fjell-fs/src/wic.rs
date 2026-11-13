//! HEIC/HEIF og RAW via Windows Imaging Component (`cfg(windows)`). Implementerer
//! `thumb::FrameDecoder`; `image`-craten beholder JPEG/PNG. Se `docs/mvp/DEKODING.md`.
//!
//! Status: kompileres i CI (`windows-latest`); aldri kjørt mot en ekte HEIC/RAW-fil.
//!
//! Feilmapping (aldri panikk, aldri krasj):
//! - Manglende kodek (HEIF-utvidelse ikke installert), ukjent format, feil ved initialisering av
//!   kodek, COM-feil og alle ukjente HRESULT gir `SourceError::Unavailable`. UI-forklaringen
//!   («HEIC/RAW trenger Windows-kodek») ligger i `UNAVAILABLE_HINT`.
//! - Ødelagt fil (`WINCODEC_ERR_BADIMAGE`, `BADHEADER`, `BADSTREAMDATA` m.fl.) gir `Corrupt`.
//! - Fil/sti finnes ikke gir `NotFound`.
//!
//! `unsafe` er nødvendig for COM-kall; modulen er (som `windows.rs` og `drives::sys`) unntatt fra
//! `unsafe_code = "deny"`. Ingen peker lever lenger enn ett kall; alle grensesnitt eies av
//! `windows`-craten.
//!
//! Dekoding gir full oppløsning, uten WIC-skalering: `thumb::finish` skalerer ned med samme kode
//! som JPEG-stien, så hasher avviker minst mulig mellom WIC-JPEG og `image`-JPEG. Pris: RAW på
//! mange megapiksler bruker mye minne (4 byte/piksel); grensen er `MAX_PIXEL_BYTES`.

#![allow(unsafe_code)]

use crate::thumb::{Decoded, FrameDecoder, WIC_EXTENSIONS};
use fjell_core::model::SourceError;
use std::path::Path;
use windows::core::{w, HSTRING, PCWSTR};
use windows::Win32::Foundation::GENERIC_READ;
use windows::Win32::Graphics::Imaging::{
    CLSID_WICImagingFactory, GUID_WICPixelFormat32bppRGBA, IWICBitmapFrameDecode,
    IWICImagingFactory, IWICPalette, WICBitmapDitherTypeNone, WICBitmapPaletteTypeCustom,
    WICDecodeMetadataCacheOnDemand,
};
use windows::Win32::System::Com::StructuredStorage::PROPVARIANT;
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED,
};

/// Forklaring UI kan vise når `Unavailable` skyldes manglende kodek.
pub const UNAVAILABLE_HINT: &str =
    "Windows mangler kodek for dette formatet (for HEIC: HEIF-bildeutvidelsen fra Microsoft Store).";

/// Øvre grense for RGBA-buffer (byte). Større bilder gir `Failed`, ikke minnekrasj.
const MAX_PIXEL_BYTES: u64 = 512 * 1024 * 1024;

// HRESULT-verdier (winerror.h) som rå i32, for å slippe konstanttype-forskjeller mellom versjoner.
const WINCODEC_ERR_BADIMAGE: i32 = 0x8898_2F60_u32 as i32;
const WINCODEC_ERR_BADHEADER: i32 = 0x8898_2F61_u32 as i32;
const WINCODEC_ERR_BADMETADATAHEADER: i32 = 0x8898_2F63_u32 as i32;
const WINCODEC_ERR_BADSTREAMDATA: i32 = 0x8898_2F71_u32 as i32;
const WINCODEC_ERR_STREAMREAD: i32 = 0x8898_2F72_u32 as i32;
const HR_FILE_NOT_FOUND: i32 = 0x8007_0002_u32 as i32;
const HR_PATH_NOT_FOUND: i32 = 0x8007_0003_u32 as i32;

/// Ren mapping HRESULT -> feil. Alt ukjent er `Unavailable` (aldri «ødelagt» uten bevis).
fn map_hresult(code: i32) -> SourceError {
    match code {
        HR_FILE_NOT_FOUND | HR_PATH_NOT_FOUND => SourceError::NotFound,
        WINCODEC_ERR_BADIMAGE
        | WINCODEC_ERR_BADHEADER
        | WINCODEC_ERR_BADMETADATAHEADER
        | WINCODEC_ERR_BADSTREAMDATA
        | WINCODEC_ERR_STREAMREAD => SourceError::Corrupt,
        _ => SourceError::Unavailable,
    }
}

fn map_err(e: windows::core::Error) -> SourceError {
    map_hresult(e.code().0)
}

/// COM for denne tråden under ett kall. `RPC_E_CHANGED_MODE` (tråden har allerede annen modus)
/// er greit: vi bruker den eksisterende og avinitialiserer ikke.
struct Com(bool);

impl Com {
    fn new() -> Result<Com, SourceError> {
        // SAFETY: vanlig COM-init; paret med `CoUninitialize` i `Drop` bare når den lyktes.
        let hr = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
        if hr.is_ok() {
            Ok(Com(true))
        } else if hr.0 == 0x8001_0106_u32 as i32 {
            Ok(Com(false))
        } else {
            Err(SourceError::Unavailable)
        }
    }
}

impl Drop for Com {
    fn drop(&mut self) {
        if self.0 {
            // SAFETY: paret med vellykket `CoInitializeEx` i `new`.
            unsafe { CoUninitialize() };
        }
    }
}

/// EXIF-orientering fra rammens metadata: JPEG ligger under `/app1/ifd`, HEIF/TIFF/RAW under `/ifd`.
fn exif_orientation(frame: &IWICBitmapFrameDecode) -> u8 {
    // SAFETY: `reader` og `pv` lever gjennom kallet; `PROPVARIANT` frigjøres av `Drop`.
    unsafe {
        let Ok(reader) = frame.GetMetadataQueryReader() else {
            return 1;
        };
        for name in [w!("/app1/ifd/{ushort=274}"), w!("/ifd/{ushort=274}")] {
            let mut pv = PROPVARIANT::default();
            let name: PCWSTR = name;
            if reader.GetMetadataByName(name, &mut pv).is_ok() {
                if let Ok(v) = u16::try_from(&pv) {
                    if (1..=8).contains(&v) {
                        return v as u8;
                    }
                }
            }
        }
    }
    1
}

fn open_frame(path: &Path) -> Result<IWICBitmapFrameDecode, SourceError> {
    // SAFETY: COM-kall med gyldige argumenter; `HSTRING` lever gjennom kallet.
    unsafe {
        let factory: IWICImagingFactory =
            CoCreateInstance(&CLSID_WICImagingFactory, None, CLSCTX_INPROC_SERVER)
                .map_err(map_err)?;
        let name = HSTRING::from(path.as_os_str());
        let decoder = factory
            .CreateDecoderFromFilename(&name, None, GENERIC_READ, WICDecodeMetadataCacheOnDemand)
            .map_err(map_err)?;
        decoder.GetFrame(0).map_err(map_err)
    }
}

/// WIC-dekoder for HEIC/HEIF og RAW. Tilstandsløs; COM og fabrikk opprettes per kall.
pub struct WicDecoder;

impl FrameDecoder for WicDecoder {
    fn extensions(&self) -> &[&str] {
        WIC_EXTENSIONS
    }

    fn probe(&self, path: &Path) -> Result<(u32, u32, u8), SourceError> {
        let _com = Com::new()?;
        let frame = open_frame(path)?;
        let (mut w, mut h) = (0u32, 0u32);
        // SAFETY: gyldige utpekere til lokale variabler.
        unsafe { frame.GetSize(&mut w, &mut h) }.map_err(map_err)?;
        Ok((w, h, exif_orientation(&frame)))
    }

    fn decode(&self, path: &Path, _max_side: u32) -> Result<Decoded, SourceError> {
        let _com = Com::new()?;
        let frame = open_frame(path)?;
        let (mut w, mut h) = (0u32, 0u32);
        // SAFETY: som over; konverteren eier sin egen kilde-referanse.
        unsafe { frame.GetSize(&mut w, &mut h) }.map_err(map_err)?;
        let bytes = u64::from(w) * u64::from(h) * 4;
        if w == 0 || h == 0 {
            return Err(SourceError::Corrupt);
        }
        if bytes > MAX_PIXEL_BYTES {
            return Err(SourceError::Failed(
                "bildet er for stort til å dekodes".into(),
            ));
        }
        let mut rgba = vec![0u8; bytes as usize];
        // SAFETY: bufferen er nøyaktig `w * h * 4` byte med stride `w * 4`; `prc = null` gir hele bildet.
        unsafe {
            let factory: IWICImagingFactory =
                CoCreateInstance(&CLSID_WICImagingFactory, None, CLSCTX_INPROC_SERVER)
                    .map_err(map_err)?;
            let conv = factory.CreateFormatConverter().map_err(map_err)?;
            conv.Initialize(
                &frame,
                &GUID_WICPixelFormat32bppRGBA,
                WICBitmapDitherTypeNone,
                None::<&IWICPalette>,
                0.0,
                WICBitmapPaletteTypeCustom,
            )
            .map_err(map_err)?;
            conv.CopyPixels(std::ptr::null(), w * 4, &mut rgba)
                .map_err(map_err)?;
        }
        Ok(Decoded {
            w,
            h,
            rgba,
            exif_orientation: exif_orientation(&frame),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hresult_mapping() {
        assert_eq!(
            map_hresult(0x8898_2F50_u32 as i32),
            SourceError::Unavailable
        ); // COMPONENTNOTFOUND
        assert_eq!(
            map_hresult(0x8898_2F07_u32 as i32),
            SourceError::Unavailable
        ); // UNKNOWNIMAGEFORMAT
        assert_eq!(map_hresult(WINCODEC_ERR_BADIMAGE), SourceError::Corrupt);
        assert_eq!(map_hresult(HR_FILE_NOT_FOUND), SourceError::NotFound);
        assert_eq!(
            map_hresult(0x8000_4005_u32 as i32),
            SourceError::Unavailable
        );
    }
}
