//! Enhetstester for `quality` (K3-1, K3-6, K3-7). Golden-testene (K3-2…K3-5) ligger i
//! `core/testdata/tests/golden_quality_screenshot.rs`.

use super::*;

/// Enkel deterministisk generator (xorshift64*), uavhengig av eksterne crates.
struct Lcg(u64);
impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
}

fn gray(w: u32, h: u32, f: impl Fn(u32, u32) -> u8) -> Thumb {
    let data = (0..h)
        .flat_map(|y| (0..w).map(move |x| (x, y)))
        .map(|(x, y)| f(x, y))
        .collect();
    Thumb::new(w, h, ThumbFmt::Gray8, data).unwrap()
}

fn checker(w: u32, h: u32) -> Thumb {
    gray(w, h, |x, y| if (x / 2 + y / 2) % 2 == 0 { 40 } else { 215 })
}

/// 3×3 gjennomsnitt (kantpiksler beholdes), brukt som «samme motiv, uskarpet».
fn box_blur(t: &Thumb) -> Thumb {
    let (w, h) = (t.w as i32, t.h as i32);
    gray(t.w, t.h, |x, y| {
        let (mut s, mut n) = (0u32, 0u32);
        for dy in -1..=1 {
            for dx in -1..=1 {
                let (xx, yy) = (x as i32 + dx, y as i32 + dy);
                if xx >= 0 && yy >= 0 && xx < w && yy < h {
                    s += t.data[(yy * w + xx) as usize] as u32;
                    n += 1;
                }
            }
        }
        (s / n) as u8
    })
}

/// K3-1: flatt bilde gir `LOW_CONTRAST`; skakkrutemønster scorer høyere enn det uskarpede.
#[test]
fn flat_is_low_contrast_and_checker_beats_blurred() {
    for v in [0u8, 128, 255] {
        assert_eq!(blur_score(&gray(64, 48, |_, _| v)), LOW_CONTRAST);
    }
    // Svak støy under varians 4 regnes også som flatt.
    assert_eq!(
        blur_score(&gray(64, 48, |x, y| 100 + ((x + y) % 2) as u8)),
        LOW_CONTRAST
    );
    let sharp = checker(64, 48);
    let (b1, b2) = (box_blur(&sharp), box_blur(&box_blur(&sharp)));
    let (s0, s1, s2) = (blur_score(&sharp), blur_score(&b1), blur_score(&b2));
    assert!(s0 < LOW_CONTRAST && s0 > s1 && s1 > s2, "{s0} {s1} {s2}");
}

/// Rgba og Gray8 med samme luma gir samme score.
#[test]
fn rgba_matches_gray() {
    let g = checker(32, 32);
    let rgba: Vec<u8> = g.data.iter().flat_map(|&v| [v, v, v, 255]).collect();
    let t = Thumb::new(32, 32, ThumbFmt::Rgba8, rgba).unwrap();
    assert_eq!(blur_score(&t).to_bits(), blur_score(&g).to_bits());
}

fn vector_thumb() -> Thumb {
    let mut r = Lcg(0x1234_5678_9ABC_DEF1);
    let noise: Vec<u8> = (0..64 * 48).map(|_| (r.next() >> 56) as u8).collect();
    gray(64, 48, |x, y| {
        let g = (x * 3 + y * 2) as u8;
        ((g as u16 + noise[(y * 64 + x) as usize] as u16 / 2) / 2 + 40) as u8
    })
}

/// K3-6: faste miniatyrer gir faste bitmønstre (feiler hvis algoritmen endres: øk `ALGO_VER`).
#[test]
fn deterministic_vectors() {
    let noisy = vector_thumb();
    let rgba: Vec<u8> = noisy
        .data
        .iter()
        .enumerate()
        .flat_map(|(i, &v)| [v, v.wrapping_add((i % 7) as u8), 255 - v, 255])
        .collect();
    let rgba = Thumb::new(64, 48, ThumbFmt::Rgba8, rgba).unwrap();
    let got = [
        blur_score(&noisy).to_bits(),
        blur_score(&box_blur(&noisy)).to_bits(),
        blur_score(&checker(64, 48)).to_bits(),
        blur_score(&rgba).to_bits(),
    ];
    let expected: [u32; 4] = VECTORS;
    assert_eq!(
        got, expected,
        "algoritme endret: øk `ALGO_VER` og oppdater vektorene"
    );
    // Samme kall to ganger gir samme bits.
    assert_eq!(blur_score(&noisy).to_bits(), got[0]);
}

const VECTORS: [u32; 4] = [1085665849, 1043099570, 1098905457, 1085650167];

/// K3-7: miniatyr mindre enn 3×3 (og ugyldig datalengde) gir `LOW_CONTRAST`, ikke panikk.
/// Deterministisk fuzz i stedet for `proptest` (ikke i avhengighetsbufferen offline).
#[test]
fn tiny_and_malformed_never_panic() {
    let mut r = Lcg(42);
    for _ in 0..2000 {
        let (w, h) = ((r.next() % 6) as u32, (r.next() % 6) as u32);
        let fmt = if r.next().is_multiple_of(2) {
            ThumbFmt::Gray8
        } else {
            ThumbFmt::Rgba8
        };
        let len = (r.next() % 80) as usize;
        let data: Vec<u8> = (0..len).map(|_| (r.next() >> 56) as u8).collect();
        // Direkte struct-bygging omgår `Thumb::new`, så også ugyldige miniatyrer prøves.
        let t = Thumb { w, h, fmt, data };
        let s = blur_score(&t);
        if w < 3 || h < 3 {
            assert_eq!(s, LOW_CONTRAST, "{w}x{h}");
        }
        assert!(s.is_finite());
    }
    for (w, h) in [(1, 1), (2, 2), (2, 9), (9, 2), (3, 1)] {
        let t = Thumb::new(w, h, ThumbFmt::Gray8, vec![7; (w * h) as usize]).unwrap();
        assert_eq!(blur_score(&t), LOW_CONTRAST);
    }
    // Akkurat 3×3: ett indre punkt gir Laplace-varians 0, uten panikk eller NaN.
    let t = gray(3, 3, |x, y| if (x + y) % 2 == 0 { 0 } else { 255 });
    assert_eq!(blur_score(&t), 0.0);
}
