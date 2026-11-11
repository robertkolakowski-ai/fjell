//! Enhetstester for `screenshot` (K4-1, K4-4, K4-5, K4-6). Golden-testene (K4-2, K4-3) ligger i
//! `core/testdata/tests/golden_quality_screenshot.rs`.

use super::*;
use crate::model::{Kind, SourceId, ThumbFmt};

fn thumb() -> Thumb {
    Thumb::new(1, 1, ThumbFmt::Gray8, vec![0]).unwrap()
}

fn meta(hint: bool, size: (u32, u32), exif: Option<bool>, mime: &str) -> AssetMeta {
    AssetMeta {
        source_id: SourceId("x".into()),
        kind: Kind::Photo,
        created_at: 0,
        mod_token: "t".into(),
        width: size.0,
        height: size.1,
        bytes: 1,
        mime: mime.into(),
        platform_screenshot_hint: hint,
        has_camera_exif: exif,
        is_local: true,
        protect_flags: Default::default(),
        albums: vec![],
        backup: None,
    }
}

const SCREEN: (u32, u32) = (1170, 2532);
const OTHER: (u32, u32) = (4032, 3024);

/// K4-1: alle 16 kombinasjoner av S1..S4 gir riktig bitmaske, antall, forslag og konfidens.
#[test]
fn all_sixteen_combinations() {
    for combo in 0u8..16 {
        let (s1, s2, s3, s4) = (
            combo & 1 != 0,
            combo & 2 != 0,
            combo & 4 != 0,
            combo & 8 != 0,
        );
        let m = meta(
            s1,
            if s2 { SCREEN } else { OTHER },
            if s3 { Some(false) } else { Some(true) },
            if s4 { "image/png" } else { "image/jpeg" },
        );
        let sig = signals(&m, &thumb());
        assert_eq!(sig.0, combo, "bitmaske for kombinasjon {combo:04b}");
        let n = combo.count_ones();
        assert_eq!(sig.count(), n);
        let expect_conf = match n {
            2 => Some(0.70),
            3 => Some(0.90),
            4 => Some(0.99),
            _ => None,
        };
        assert_eq!(sig.suggestion(false), expect_conf, "forslag {combo:04b}");
        let strong = (s1 || s2) && n >= 2;
        assert_eq!(sig.suggestion(true).is_some(), strong, "strong {combo:04b}");
        if strong {
            assert_eq!(sig.suggestion(true), expect_conf);
        }
    }
}

/// K4-4: ukjent EXIF (`None`) gir ikke S3; `Some(true)` heller ikke.
#[test]
fn unknown_exif_is_not_s3() {
    let png = "image/png";
    let none = signals(&meta(false, OTHER, None, png), &thumb());
    assert!(!none.has(ShotSignals::S3_NO_CAMERA_EXIF));
    assert_eq!(none.suggestion(false), None, "bare S4 er ikke forslag");
    let cam = signals(&meta(false, OTHER, Some(true), png), &thumb());
    assert!(!cam.has(ShotSignals::S3_NO_CAMERA_EXIF));
    let no_cam = signals(&meta(false, OTHER, Some(false), png), &thumb());
    assert!(no_cam.has(ShotSignals::S3_NO_CAMERA_EXIF));
}

/// K4-5: landskap (bredde og høyde byttet) treffer S2.
#[test]
fn landscape_resolution_hits_s2() {
    for &(w, h) in SCREEN_SIZES {
        for (a, b) in [(w, h), (h, w)] {
            let sig = signals(&meta(false, (a, b), Some(true), "image/jpeg"), &thumb());
            assert!(sig.has(ShotSignals::S2_SCREEN_SIZE), "{a}x{b}");
        }
    }
    assert!(!is_known_screen_size(1171, 2532));
    assert!(!is_known_screen_size(0, 0));
}

/// K4-6: begrunnelseskodene kommer fra bitmasken i fast rekkefølge.
#[test]
fn reasons_from_bitmask() {
    let all = ShotSignals(15);
    assert_eq!(
        all.reasons(),
        vec![
            ReasonCode::ShotPlatformFlag,
            ReasonCode::ShotScreenSize,
            ReasonCode::ShotNoCameraExif,
            ReasonCode::ShotPng
        ]
    );
    assert_eq!(
        ShotSignals(ShotSignals::S3_NO_CAMERA_EXIF | ShotSignals::S4_PNG).reasons(),
        vec![ReasonCode::ShotNoCameraExif, ReasonCode::ShotPng]
    );
    assert!(ShotSignals(0).reasons().is_empty());
}
