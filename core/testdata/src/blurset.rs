//! Grensesnitt: `generate_blur(out_dir, seed) -> io::Result<BManifest>`. Utvidet korpus for
//! uskarphetskalibrering (K8): sju motivfamilier × to deler (`train`/`eval`, helt ulike motiver)
//! × 8 motiver × 9 varianter (sju familier, den siste med golden-korpusets motivtype). Skarpe varianter: original, lavkontrast (0,25–0,5), lett myk
//! (σ 0,30–0,40 % av bredden), JPEG q60 og halv oppløsning. Uskarpe (σ_ekv ≥ 1 % av bredden):
//! Gauss, fokusfeil (skivekjerne), lineær bevegelsesuskarphet (vilkårlig vinkel, 4–8 %) og
//! uskarphet med sensorstøy. Alle varianter av et motiv deler `visual_group` (brukes til
//! hash-portene). Skriver også `thumbs/` (Gray8-PNG, lengste side 256) og `blur_manifest.json`.
//! Ren funksjon av seed; ingen klokke, ingen tråder.

use crate::ops::*;
use crate::rng::fnv64;
use crate::synth::lowcontrast;
use crate::textures::{texture, FAMILIES};
use serde::Serialize;
use std::{fs, io, path::Path};

pub const BLUR_SEED: u64 = 0xB1AB_5EED_0000_0008;
pub const MOTIFS_PER_CELL: usize = 8;

#[derive(Serialize, Clone, Debug)]
pub struct BEntry {
    pub file: String,
    pub thumb: String,
    pub family: String,
    /// "train" eller "eval".
    pub split: String,
    /// sharp, lowcontrast, soft, recoded, down, gauss, defocus, motion, noisyblur.
    pub kind: String,
    /// Parameter som tekst (σ/radius/lengde i % av bredden, vinkel).
    pub param: String,
    pub blurry: bool,
    /// Miniatyrens lumavarians ≥ 4 (`MIN_LUMA_VARIANCE`): ellers er bildet «ikke vurderbart»
    /// (nesten flatt, f.eks. kraftig uskarpet tre-tekstur) og regnes ikke med i presisjon/recall.
    pub assessable: bool,
    pub visual_group: u32,
    pub mime: String,
    pub width: u32,
    pub height: u32,
    pub bytes: u64,
    pub fnv64: String,
}

#[derive(Serialize, Debug)]
pub struct BManifest {
    pub seed: u64,
    pub entries: Vec<BEntry>,
}

fn thumb_png(img: &Img) -> Vec<u8> {
    let s = (256.0 / img.w.max(img.h) as f32).min(1.0);
    let t = resize_area(
        img,
        (img.w as f32 * s).round() as usize,
        (img.h as f32 * s).round() as usize,
    );
    encode_gray_png(t.w, t.h, &luma(&t))
}

/// Populasjonsvarians av Gray8-miniatyren (samme definisjon som kjernens lavkontrastregel).
fn thumb_variance(img: &Img) -> f64 {
    let s = (256.0 / img.w.max(img.h) as f32).min(1.0);
    let t = resize_area(
        img,
        (img.w as f32 * s).round() as usize,
        (img.h as f32 * s).round() as usize,
    );
    let l = luma(&t);
    let n = l.len() as f64;
    let mean = l.iter().map(|&v| v as f64).sum::<f64>() / n;
    l.iter().map(|&v| (v as f64 - mean).powi(2)).sum::<f64>() / n
}

struct Built {
    entry: BEntry,
    img: Vec<u8>,
    thumb: Vec<u8>,
}

/// Bygger én celle (familie × del): 8 motiver × 9 varianter. Ren funksjon av argumentene.
fn build_cell(fi: usize, si: usize, seed: u64) -> Vec<Built> {
    let fam = FAMILIES[fi];
    let split = ["train", "eval"][si];
    let (w, h) = (512usize, 384usize);
    let wf = w as f32;
    let mut out = Vec::new();
    for m in 0..MOTIFS_PER_CELL {
        let gid = ((fi * 2 + si) * MOTIFS_PER_CELL + m) as u32;
        let mseed = seed ^ ((gid as u64 + 1) << 20).wrapping_mul(0x9E37_79B1_85EB_CA87);
        let base = texture(fam, mseed, w, h);
        let angle = [0.0f32, 30.0, 60.0, 90.0, 120.0, 150.0, 15.0, 75.0][m];
        let pick = |v: [f32; 3], k: usize| v[(m + k) % 3];
        let g = pick([1.0, 1.5, 2.5], 0);
        let d = pick([2.0, 3.0, 4.0], 1);
        let l = pick([4.0, 6.0, 8.0], 2);
        let low = 0.25 + 0.05 * (m % 6) as f32;
        // Skarpe kontroller opp til σ 0,5 % (som golden-korpusets `g05`); uskarpt er σ_ekv ≥ 1 %.
        let soft = pick([0.30, 0.40, 0.50], 0);
        // Sensorstøy kommer etter optisk uskarphet: 0…±3 nivåer per motiv (0 = støyfritt, som golden-korpuset), på alle varianter.
        let amp = (m % 4) as i32;
        let nz = |img: &Img, k: u64, a: i32| add_noise(img, mseed ^ (k * 0x51), a);
        let sharp = nz(&base, 1, amp);
        // (kind, param, bilde, uskarp, jpeg?)
        let variants: Vec<(&str, String, Img, bool, bool)> = vec![
            ("sharp", format!("støy ±{amp}"), sharp.clone(), false, false),
            (
                "lowcontrast",
                format!("{low:.2}"),
                nz(&lowcontrast(&base, low), 2, amp),
                false,
                false,
            ),
            (
                "soft",
                format!("sigma {soft:.2}%"),
                nz(&gaussian(&base, soft / 100.0 * wf), 3, amp),
                false,
                false,
            ),
            ("recoded", "q60".into(), sharp.clone(), false, true),
            (
                "down",
                "50%".into(),
                resize_area(&sharp, w / 2, h / 2),
                false,
                false,
            ),
            (
                "gauss",
                format!("sigma {g:.1}%"),
                nz(&gaussian(&base, g / 100.0 * wf), 4, amp),
                true,
                false,
            ),
            (
                "defocus",
                format!("r {d:.0}% (sigma_ekv {:.1}%)", d / 2.0),
                nz(&defocus(&base, d / 100.0 * wf), 5, amp),
                true,
                false,
            ),
            (
                "motion",
                format!("len {l:.0}% vinkel {angle:.0}"),
                nz(&motion_dir(&base, l / 100.0 * wf, angle), 6, amp),
                true,
                false,
            ),
            (
                "noisyblur",
                format!("sigma 1.5% + støy ±{}", amp + 3),
                nz(&gaussian(&base, 0.015 * wf), 7, amp + 3),
                true,
                false,
            ),
        ];
        for (kind, param, img, blurry, jpeg) in variants {
            let (ext, mime, bytes) = if jpeg {
                ("jpg", "image/jpeg", encode_jpeg(&img, 60))
            } else {
                ("png", "image/png", encode_png(&img))
            };
            let stem = format!("{}_{}_{m:02}_{kind}", fam.name(), split);
            // Miniatyr fra det som faktisk ligger i fila (jpeg dekodes på nytt).
            let decoded = if jpeg { decode(&bytes) } else { img };
            let assessable = thumb_variance(&decoded) >= 4.0;
            out.push(Built {
                entry: BEntry {
                    file: format!("images/{stem}.{ext}"),
                    thumb: format!("thumbs/{stem}.png"),
                    family: fam.name().into(),
                    split: split.into(),
                    kind: kind.into(),
                    param,
                    blurry,
                    assessable,
                    visual_group: gid,
                    mime: mime.into(),
                    width: decoded.w as u32,
                    height: decoded.h as u32,
                    bytes: bytes.len() as u64,
                    fnv64: format!("{:016x}", fnv64(&bytes)),
                },
                thumb: thumb_png(&decoded),
                img: bytes,
            });
        }
    }
    out
}

/// Genererer korpuset. Cellene bygges parallelt (hver er en ren funksjon av seed), men samles og
/// skrives i fast rekkefølge, så resultatet er byte-identisk mellom kjøringer.
pub fn generate_blur(out: &Path, seed: u64) -> io::Result<BManifest> {
    let cells: Vec<(usize, usize)> = (0..FAMILIES.len())
        .flat_map(|f| (0..2).map(move |s| (f, s)))
        .collect();
    let workers = std::thread::available_parallelism()
        .map_or(2, |n| n.get())
        .min(8);
    let next = std::sync::atomic::AtomicUsize::new(0);
    let results: std::sync::Mutex<Vec<(usize, Vec<Built>)>> = std::sync::Mutex::new(Vec::new());
    std::thread::scope(|sc| {
        for _ in 0..workers {
            sc.spawn(|| loop {
                let i = next.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                if i >= cells.len() {
                    break;
                }
                let built = build_cell(cells[i].0, cells[i].1, seed);
                results.lock().unwrap().push((i, built));
            });
        }
    });
    let mut all = results.into_inner().unwrap();
    all.sort_by_key(|(i, _)| *i);
    let mut entries = Vec::new();
    for (_, cell) in all {
        for b in cell {
            for (p, bytes) in [(&b.entry.file, &b.img), (&b.entry.thumb, &b.thumb)] {
                let path = out.join(p);
                fs::create_dir_all(path.parent().unwrap())?;
                fs::write(path, bytes)?;
            }
            entries.push(b.entry);
        }
    }
    let m = BManifest { seed, entries };
    let mut s = serde_json::to_string_pretty(&m).expect("json");
    s.push('\n');
    fs::write(out.join("blur_manifest.json"), s)?;
    Ok(m)
}

/// Seed for det uberørte kontrollsettet: ingen kalibreringsvalg er tatt mot dette (K8).
pub const BLUR_HOLDOUT_SEED: u64 = 0x0A0B_C0DE_5EED_0017;
