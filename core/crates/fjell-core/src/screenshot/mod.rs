//! Modul `screenshot` (L1). Se docs/mvp/ARKITEKTUR.md del 2 og PLAN-KJERNE del 6 (K4).
//!
//! Avhenger av: model
//! Grensesnitt (hovedpunkter):
//! - Ren funksjon: `signals(&AssetMeta, &Thumb) -> ShotSignals` (bitmaske S1–S4, lagres som
//!   `fingerprint.shot_signals`). Miniatyren brukes ikke i MVP.
//! - `ShotSignals::suggestion(require_strong) -> Option<f32>` (forslag og konfidens) og
//!   `reasons()` (begrunnelseskoder fra bitmasken). Tabell i `screen_sizes.rs`.
//!
//! Signaler: S1 plattformflagg, S2 kjent skjermstørrelse (begge retninger), S3 ingen kamera-EXIF
//! (`Some(false)`; `None` teller ikke), S4 `image/png`. Forslag fra to signaler.
//!
//! Standardregel (K4-3, PLAN-KJERNE del 9): minst to signaler OG minst én av S1/S2
//! (`DEFAULT_REQUIRE_STRONG = true`, brukes av `ShotSignals::suggest_default`). Begrunnelse: S3
//! (ingen kamera-EXIF) og S4 (PNG) alene er svake; eksport, nedlasting og redigerte PNG-er har
//! begge uten å være skjermbilder. Regelen er satt ut fra denne logikken, ikke tunet mot korpuset:
//! to-av-fire ga presisjon 0,75 (10 PNG uten EXIF i vilkårlig størrelse var falske positive),
//! den strenge regelen 1,0 (port ≥ 0,95). Rå «to av fire» er fortsatt tilgjengelig via
//! `suggestion(false)`.

mod screen_sizes;

pub use screen_sizes::{is_known_screen_size, SCREEN_SIZES};

use crate::model::{AssetMeta, ReasonCode, Thumb};

/// Standard for `require_strong`: forslag krever minst én av S1/S2 i tillegg til to signaler.
pub const DEFAULT_REQUIRE_STRONG: bool = true;

/// Bitmaske over de fire signalene.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub struct ShotSignals(pub u8);

impl ShotSignals {
    /// S1: plattformflagg eller album «Skjermbilder».
    pub const S1_PLATFORM_FLAG: u8 = 1;
    /// S2: oppløsning lik kjent skjermstørrelse.
    pub const S2_SCREEN_SIZE: u8 = 2;
    /// S3: ingen kamera-EXIF.
    pub const S3_NO_CAMERA_EXIF: u8 = 4;
    /// S4: `image/png`.
    pub const S4_PNG: u8 = 8;

    pub fn has(self, flag: u8) -> bool {
        self.0 & flag != 0
    }

    /// Antall signaler som slo til.
    pub fn count(self) -> u32 {
        (self.0 & 0x0F).count_ones()
    }

    /// Begrunnelseskoder i fast rekkefølge S1..S4.
    pub fn reasons(self) -> Vec<ReasonCode> {
        [
            (Self::S1_PLATFORM_FLAG, ReasonCode::ShotPlatformFlag),
            (Self::S2_SCREEN_SIZE, ReasonCode::ShotScreenSize),
            (Self::S3_NO_CAMERA_EXIF, ReasonCode::ShotNoCameraExif),
            (Self::S4_PNG, ReasonCode::ShotPng),
        ]
        .into_iter()
        .filter(|&(bit, _)| self.has(bit))
        .map(|(_, code)| code)
        .collect()
    }

    /// Konfidens fra antall signaler (startverdier): 2 → 0,70, 3 → 0,90, 4 → 0,99; ellers 0.
    pub fn confidence(self) -> f32 {
        match self.count() {
            2 => 0.70,
            3 => 0.90,
            4 => 0.99,
            _ => 0.0,
        }
    }

    /// Forslag (konfidens) når minst to signaler slår til. Med `require_strong` kreves i tillegg
    /// minst én av S1/S2 (standard er `DEFAULT_REQUIRE_STRONG`, se `suggest_default`).
    pub fn suggestion(self, require_strong: bool) -> Option<f32> {
        if self.count() < 2 {
            return None;
        }
        if require_strong && !(self.has(Self::S1_PLATFORM_FLAG) || self.has(Self::S2_SCREEN_SIZE)) {
            return None;
        }
        Some(self.confidence())
    }

    /// Forslag med standardregelen (`DEFAULT_REQUIRE_STRONG`).
    pub fn suggest_default(self) -> Option<f32> {
        self.suggestion(DEFAULT_REQUIRE_STRONG)
    }
}

/// Beregner signalbitmasken fra metadata. Miniatyren er ikke i bruk i MVP.
pub fn signals(meta: &AssetMeta, _thumb: &Thumb) -> ShotSignals {
    let mut bits = 0u8;
    if meta.platform_screenshot_hint {
        bits |= ShotSignals::S1_PLATFORM_FLAG;
    }
    if is_known_screen_size(meta.width, meta.height) {
        bits |= ShotSignals::S2_SCREEN_SIZE;
    }
    if meta.has_camera_exif == Some(false) {
        bits |= ShotSignals::S3_NO_CAMERA_EXIF;
    }
    if meta.mime == "image/png" {
        bits |= ShotSignals::S4_PNG;
    }
    ShotSignals(bits)
}

#[cfg(test)]
mod tests;
