//! Grensesnitt: `series_corpus(seed) -> SeriesCorpus`, `eye_cases(seed) -> Vec<EyeCase>`, `shot_cases() ->
//! Vec<ShotCase>`, `gray_thumb(&Img) -> Gray`. Syntetisk, merket korpus for kalibreringen
//! (docs/v1/KALIBRERING-V1.md). Alt er rene funksjoner av `seed`; ingen filer, ingen klokke, ingen tråder og
//! aldri ekte brukerbilder. Kjenner ikke fjell-core (typene her er rene data).
//!
//! Hva som er merket (og dermed hva «fasit» betyr her):
//! - Serier: rask serie (samme motiv, små endringer, kort tid mellom bildene) = én serie, uansett hvor
//!   stor hashavstanden ble. Tre bevegelsesklasser (rolig, middels, livlig) og noen langsomme serier med
//!   glipp opp til 19 s. Samme motiv på nytt etter 2–15 min = en annen serie. «Gange» (ulike komposisjoner,
//!   få sekunder mellom) = ingen serie.
//! - Øyne: plassholder. Fordelingen av analysatorens sikkerhet er ANTATT, ikke målt (ingen modell er valgt).
//! - Skjermbilder: metadatasignaler (S1..S4) for typiske og vanskelige tilfeller.
//!
//! Fasiten er konstruert av oss. Den sier ikke hva en ekte bruker ville merket.

use crate::ops::*;
use crate::rng::Rng;
use crate::synth::scene;

pub const CALIB_SEED: u64 = 0xCA11_B8A7_E000_0001;
const T0: i64 = 1_700_000_000;

/// Gray8-miniatyr (lengste side 256, area-average), samme referansenedskalering som golden-korpuset.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Gray {
    pub w: usize,
    pub h: usize,
    pub px: Vec<u8>,
}

pub fn gray_thumb(img: &Img) -> Gray {
    let s = (256.0 / img.w.max(img.h) as f32).min(1.0);
    let t = resize_area(
        img,
        (img.w as f32 * s).round() as usize,
        (img.h as f32 * s).round() as usize,
    );
    Gray {
        w: t.w,
        h: t.h,
        px: luma(&t),
    }
}

/// Ett bilde i serie-korpuset.
#[derive(Clone, Debug)]
pub struct Frame {
    pub name: String,
    pub thumb: Gray,
    /// Opptakstid (Unix-sekunder).
    pub created_at: i64,
    /// `Some(id)`: hører til serie `id`. `None`: ingen serie.
    pub series: Option<u32>,
    /// Hva bildet er (til rapportering): `calm`, `medium`, `lively`, `slow`, `revisit`, `walk`.
    pub class: &'static str,
}

#[derive(Clone, Debug)]
pub struct SeriesCorpus {
    pub frames: Vec<Frame>,
}

/// Legger `level` lyse eller mørke rektangler (7–13 % av sidene) oppå motivet: «noe flyttet seg».
fn perturb(base: &Img, r: &mut Rng, level: usize) -> Img {
    let mut o = base.clone();
    for _ in 0..level {
        let rw = (base.w as f32 * r.range(0.07, 0.13)) as usize;
        let rh = (base.h as f32 * r.range(0.07, 0.13)) as usize;
        let x0 = r.below((base.w - rw) as u64) as usize;
        let y0 = r.below((base.h - rh) as u64) as usize;
        let off: i32 = if r.below(2) == 0 { 70 } else { -70 };
        for y in y0..y0 + rh {
            for x in x0..x0 + rw {
                let i = (y * base.w + x) * 3;
                for c in 0..3 {
                    o.rgb[i + c] = (o.rgb[i + c] as i32 + off).clamp(0, 255) as u8;
                }
            }
        }
    }
    o
}

/// Bevegelsesklasse: (navn, antall serier, bildenivå (min,maks) i `perturb`, tid mellom bilder (min,maks) s).
#[allow(clippy::type_complexity)]
const CLASSES: [(&str, usize, (usize, usize), (i64, i64)); 4] = [
    ("calm", 12, (0, 1), (1, 3)),
    ("medium", 10, (1, 2), (2, 6)),
    ("lively", 10, (1, 3), (3, 12)),
    ("slow", 6, (1, 2), (12, 19)),
];

/// Serie-korpuset: 38 serier à 4–8 bilder, 12 «samme sted senere» (to delserier hver) og 12 «gange».
pub fn series_corpus(seed: u64) -> SeriesCorpus {
    let mut frames: Vec<Frame> = Vec::new();
    let mut slot = 0i64; // hver gruppe får egen time, så ingen gruppe grenser mot en annen i tid
    let mut sid = 0u32;
    let push = |frames: &mut Vec<Frame>, img: &Img, t: i64, series, class| {
        let name = format!("f{:04}", frames.len());
        frames.push(Frame {
            name,
            thumb: gray_thumb(img),
            created_at: t,
            series,
            class,
        });
    };
    for (ci, &(class, n_series, (lo, hi), (g0, g1))) in CLASSES.iter().enumerate() {
        for s in 0..n_series {
            let sd = seed ^ ((ci as u64 + 1) << 40) ^ (s as u64).wrapping_mul(0x9E37_79B1);
            let mut r = Rng::new(sd);
            let base = scene(sd, 512, 384);
            let n = 4 + r.below(5) as usize;
            let mut t = T0 + slot * 7200;
            slot += 1;
            for _ in 0..n {
                let level = lo + r.below((hi - lo + 1) as u64) as usize;
                let img = perturb(&base, &mut r, level);
                push(&mut frames, &img, t, Some(sid), class);
                t += g0 + r.below((g1 - g0 + 1) as u64) as i64;
            }
            sid += 1;
        }
    }
    // Samme sted senere: to delserier av samme motiv, 2–15 minutter fra hverandre.
    for s in 0..12 {
        let sd = seed ^ (9 << 40) ^ (s as u64).wrapping_mul(0x9E37_79B1);
        let mut r = Rng::new(sd);
        let base = scene(sd, 512, 384);
        let mut t = T0 + slot * 7200;
        slot += 1;
        for part in 0..2 {
            for _ in 0..3 {
                let level = 1 + r.below(2) as usize;
                let img = perturb(&base, &mut r, level);
                push(&mut frames, &img, t, Some(sid), "revisit");
                t += 1 + r.below(3) as i64;
            }
            sid += 1;
            if part == 0 {
                t += 120 + r.below(780) as i64;
            }
        }
    }
    // Gange: ulike komposisjoner med få sekunder mellom (ingen serie). Samme palett med flyttet komposisjon,
    // og samme scene med 30 % beskjæring (som golden-korpusets negative par).
    for s in 0..12 {
        let sd = seed ^ (10 << 40) ^ (s as u64).wrapping_mul(0x9E37_79B1);
        let mut r = Rng::new(sd);
        let a = scene(sd, 384, 288);
        let mut shifted = Img::new(384, 288);
        for y in 0..288 {
            for x in 0..384 {
                let o = (y * 384 + (x + 190) % 384) * 3;
                shifted.set(x, y, [a.rgb[o], a.rgb[o + 1], a.rgb[o + 2]]);
            }
        }
        let wide = scene(sd ^ 0x55, 512, 384);
        let mut part = Img::new(300, 384);
        for y in 0..384 {
            for x in 0..300 {
                let o = (y * 512 + x + 212 * (s % 2)) * 3;
                part.set(x, y, [wide.rgb[o], wide.rgb[o + 1], wide.rgb[o + 2]]);
            }
        }
        let mut t = T0 + slot * 7200;
        slot += 1;
        for img in [&a, &shifted, &wide, &part] {
            push(&mut frames, img, t, None, "walk");
            t += 3 + r.below(6) as i64;
        }
    }
    SeriesCorpus { frames }
}

/// Én ramme i en øyesak: sannheten og hva en (antatt) analysator ville rapportert.
#[derive(Clone, Copy, Debug)]
pub struct EyeFrame {
    pub truth_closed: bool,
    pub reported_closed: bool,
    pub confidence: f32,
    /// Antatt skarphetsscore (høy = skarp), slik at rangeringen har noe å velge på.
    pub sharpness: f32,
}

/// En serie på 3–6 bilder med ett ansikt. Plassholder: se modulhodet.
#[derive(Clone, Debug)]
pub struct EyeCase {
    pub frames: Vec<EyeFrame>,
}

/// Antatt analysator: lukkede øyne rapporteres riktig i 88 % av tilfellene med sikkerhet 0,55..1,0 (ellers
/// «åpne» med 0,50..0,85); åpne øyne feilrapporteres lukket i 8 % av tilfellene med sikkerhet 0,50..0,85
/// (ellers «åpne» med 0,60..1,0). Hver serie har 0–2 bilder med lukkede øyne, og minst ett åpent.
pub fn eye_cases(seed: u64) -> Vec<EyeCase> {
    let mut r = Rng::new(seed ^ 0xE1E5);
    (0..400)
        .map(|_| {
            let n = 3 + r.below(4) as usize;
            let closed_n = r.below(3).min(n as u64 - 1) as usize;
            // Lukkede øyne på tilfeldige plasser (ikke alltid lavest id).
            let mut truth: Vec<bool> = (0..n).map(|i| i < closed_n).collect();
            for i in (1..n).rev() {
                truth.swap(i, r.below(i as u64 + 1) as usize);
            }
            let frames = truth
                .into_iter()
                .map(|truth_closed| {
                    let (reported_closed, confidence) = if truth_closed {
                        if r.unit() < 0.88 {
                            (true, r.range(0.55, 1.0))
                        } else {
                            (false, r.range(0.50, 0.85))
                        }
                    } else if r.unit() < 0.08 {
                        (true, r.range(0.50, 0.85))
                    } else {
                        (false, r.range(0.60, 1.0))
                    };
                    EyeFrame {
                        truth_closed,
                        reported_closed,
                        confidence,
                        sharpness: r.range(0.02, 0.05),
                    }
                })
                .collect();
            EyeCase { frames }
        })
        .collect()
}

/// Metadata for ett skjermbilde-/foto-tilfelle (signalene S1..S4 regnes av kjernen).
#[derive(Clone, Debug)]
pub struct ShotCase {
    pub name: &'static str,
    pub width: u32,
    pub height: u32,
    pub mime: &'static str,
    pub platform_hint: bool,
    pub camera_exif: Option<bool>,
    /// Er det egentlig et skjermbilde?
    pub truth: bool,
    /// Antall tilfeller av denne typen (vekt).
    pub count: u32,
}

/// Typiske og vanskelige tilfeller. Antallene er våre antakelser om en vanlig mobilsamling, ikke målt.
pub fn shot_cases() -> Vec<ShotCase> {
    #[rustfmt::skip]
    #[allow(clippy::type_complexity)]
    let table: [(&'static str, u32, u32, &'static str, bool, Option<bool>, bool, u32); 12] = [
        ("skjermbilde, plattformflagg, png", 1170, 2532, "image/png", true, Some(false), true, 40),
        ("skjermbilde, plattformflagg, jpeg", 1170, 2532, "image/jpeg", true, Some(false), true, 10),
        ("skjermbilde uten flagg, kjent størrelse, png", 1284, 2778, "image/png", false, Some(false), true, 20),
        ("skjermbilde uten flagg, kjent størrelse, jpeg", 1284, 2778, "image/jpeg", false, Some(false), true, 6),
        ("beskåret skjermbilde, png", 1100, 1400, "image/png", false, Some(false), true, 12),
        ("skjermbilde fra annen enhet, ukjent størrelse, png", 1366, 768, "image/png", false, None, true, 6),
        ("kamerabilde med exif", 4032, 3024, "image/jpeg", false, Some(true), false, 120),
        ("kamerabilde i skjermstørrelse", 1170, 2532, "image/jpeg", false, Some(true), false, 4),
        ("bilde fra meldingstjeneste (jpeg, uten exif)", 1280, 960, "image/jpeg", false, Some(false), false, 30),
        ("redigert foto eksportert som png", 3000, 2000, "image/png", false, Some(false), false, 12),
        ("nedlastet bilde i skjermstørrelse (png, uten exif)", 1170, 2532, "image/png", false, Some(false), false, 5),
        ("scannet dokument (png, uten exif)", 2480, 3508, "image/png", false, None, false, 6),
    ];
    table
        .into_iter()
        .map(
            |(name, width, height, mime, platform_hint, camera_exif, truth, count)| ShotCase {
                name,
                width,
                height,
                mime,
                platform_hint,
                camera_exif,
                truth,
                count,
            },
        )
        .collect()
}
