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

const VECTORS: [u32; 4] = [1039478234, 993368672, 1051124735, 1039458161];

/// K3-7: miniatyr mindre enn `MIN_SIDE` (og ugyldig datalengde) gir `LOW_CONTRAST`, ikke panikk.
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
    // Under `MIN_SIDE` (retningsstatistikk trenger lag 2 i alle retninger): ikke vurderbar.
    let t = gray(3, 3, |x, y| if (x + y) % 2 == 0 { 0 } else { 255 });
    assert_eq!(blur_score(&t), LOW_CONTRAST);
    let t = gray(7, 40, |x, y| if (x + y) % 2 == 0 { 0 } else { 255 });
    assert_eq!(blur_score(&t), LOW_CONTRAST);
    // Akkurat `MIN_SIDE`: vurderbar, endelig og positiv.
    let t = gray(8, 8, |x, y| (x * 40 + y * 7) as u8 ^ ((x * y) as u8));
    let s = blur_score(&t);
    assert!(s.is_finite() && s > 0.0 && s < LOW_CONTRAST, "{s}");
}

/// Tekstur med fin detalj (hvit støy glattet svakt), ikke periodisk.
fn texture(w: u32, h: u32, seed: u64) -> Thumb {
    let mut r = Lcg(seed);
    let n: Vec<i32> = (0..w * h).map(|_| (r.next() >> 56) as i32).collect();
    let at = |x: i32, y: i32| {
        n[(y.clamp(0, h as i32 - 1) * w as i32 + x.clamp(0, w as i32 - 1)) as usize]
    };
    gray(w, h, |x, y| {
        let (x, y) = (x as i32, y as i32);
        ((at(x, y) * 2 + at(x + 1, y) + at(x, y + 1)) / 4).clamp(0, 255) as u8
    })
}

fn blur_dir(t: &Thumb, dx: i32, dy: i32, len: i32) -> Thumb {
    let (w, h) = (t.w as i32, t.h as i32);
    gray(t.w, t.h, |x, y| {
        let mut s = 0i32;
        for i in 0..len {
            let (xx, yy) = (
                (x as i32 + dx * (i - len / 2)).clamp(0, w - 1),
                (y as i32 + dy * (i - len / 2)).clamp(0, h - 1),
            );
            s += t.data[(yy * w + xx) as usize] as i32;
        }
        (s / len) as u8
    })
}

/// K8: lineær bevegelsesuskarphet (13 px ≈ 8 % av bredden; vannrett, loddrett, diagonal) og isotrop
/// uskarphet flagges;
/// den skarpe teksturen gjør det ikke. Samme motiv, bare uskarpheten er ulik.
#[test]
fn motion_and_isotropic_blur_are_flagged_sharp_is_not() {
    for seed in 1..=6u64 {
        let sharp = texture(160, 120, seed);
        let s0 = blur_score(&sharp);
        assert!(s0 >= BLUR_ABS_MAX, "skarp flagget, seed {seed}: {s0}");
        for (dx, dy) in [(1, 0), (0, 1), (1, 1), (1, -1)] {
            let s = blur_score(&blur_dir(&sharp, dx, dy, 13));
            assert!(
                s < BLUR_ABS_MAX,
                "bevegelse ({dx},{dy}) ikke flagget, seed {seed}: {s}"
            );
        }
        // Tolv 3×3-passer ≈ Gauss-σ 2,8 px (grensen ligger rundt 1,3 px; svakere uskarphet på ren støy ligner glatt struktur).
        let iso = (0..12).fold(sharp.clone(), |t, _| box_blur(&t));
        assert!(
            blur_score(&iso) < BLUR_ABS_MAX,
            "isotrop uskarphet ikke flagget, seed {seed}: {} (skarp {s0})",
            blur_score(&iso)
        );
    }
}

/// Transponering og speiling bytter bare retningene, så scoren er bit for bit lik.
#[test]
fn score_is_invariant_under_transpose_and_mirror() {
    let t = texture(97, 61, 5);
    let (w, h) = (t.w as usize, t.h as usize);
    let tr = Thumb::new(
        h as u32,
        w as u32,
        ThumbFmt::Gray8,
        (0..w)
            .flat_map(|x| (0..h).map(move |y| (x, y)))
            .map(|(x, y)| t.data[y * w + x])
            .collect(),
    )
    .unwrap();
    let mir = Thumb::new(
        w as u32,
        h as u32,
        ThumbFmt::Gray8,
        (0..h)
            .flat_map(|y| (0..w).map(move |x| (x, y)))
            .map(|(x, y)| t.data[y * w + (w - 1 - x)])
            .collect(),
    )
    .unwrap();
    let s = blur_score(&t).to_bits();
    assert_eq!(blur_score(&tr).to_bits(), s);
    assert_eq!(blur_score(&mir).to_bits(), s);
}

/// Scoren er strengt positiv og under `LOW_CONTRAST` for vurderbare bilder, og faller med uskarphet.
#[test]
fn score_is_positive_and_falls_with_blur() {
    let mut cur = texture(128, 96, 11);
    let mut prev = f32::INFINITY;
    for _ in 0..6 {
        let s = blur_score(&cur);
        assert!(s > 0.0 && s < LOW_CONTRAST && s < prev, "{s} {prev}");
        prev = s;
        cur = box_blur(&cur);
    }
}
