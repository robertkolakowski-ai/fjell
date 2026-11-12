//! Golden-tester for K3 (uskarphetsscore) og K4 (skjermbildegjenkjenning) mot korpuset fra
//! `corpus::generate`. Grensesnitt: kjører `fjell_core::quality` og `fjell_core::screenshot`
//! på korpusminiatyrene (Gray8-PNG) og på varianter generert fra samme seed (K3-2).
//! Tallene skrives med `--nocapture`; portene er PLAN-KJERNE del 5–6.

use fjell_core::model::{AssetMeta, Kind, SourceId, Thumb, ThumbFmt};
use fjell_core::quality::{blur_score, BLUR_ABS_MAX, LOW_CONTRAST};
use fjell_core::screenshot::signals;
use fjell_testdata::corpus::{generate, DEFAULT_SEED};
use fjell_testdata::manifest::{Entry, Manifest};
use fjell_testdata::ops::{gaussian, luma, resize_area, Img};
use fjell_testdata::synth::scene;
use std::{path::PathBuf, sync::OnceLock};

fn corpus() -> &'static (PathBuf, Manifest) {
    static C: OnceLock<(PathBuf, Manifest)> = OnceLock::new();
    C.get_or_init(|| {
        let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("golden_quality");
        let _ = std::fs::remove_dir_all(&dir);
        let m = generate(&dir, DEFAULT_SEED).unwrap();
        (dir, m)
    })
}

fn corpus_thumb(e: &Entry) -> Thumb {
    let g = image::open(corpus().0.join(&e.thumb)).unwrap().to_luma8();
    Thumb::new(g.width(), g.height(), ThumbFmt::Gray8, g.into_raw()).unwrap()
}

/// Referanse-nedskalering som i generatoren: lengste side 256, aldri oppskalert.
fn thumb_of(img: &Img) -> Thumb {
    let s = (256.0 / img.w.max(img.h) as f32).min(1.0);
    let t = resize_area(
        img,
        (img.w as f32 * s).round() as usize,
        (img.h as f32 * s).round() as usize,
    );
    Thumb::new(t.w as u32, t.h as u32, ThumbFmt::Gray8, luma(&t)).unwrap()
}

fn entries(group: &str) -> Vec<&'static Entry> {
    corpus()
        .1
        .entries
        .iter()
        .filter(|e| e.group == group)
        .collect()
}

/// K3-2: score faller monotont med økende sigma (10 motiver × 5 sigma, sigma som andel av bredden).
/// Over ca. 2 % flater scoren ut mot støygulvet fra 8-bits kvantisering og stiger igjen (rapportert).
#[test]
fn k3_2_score_falls_monotonically_with_sigma() {
    for i in 0..10u64 {
        let m = scene(
            DEFAULT_SEED ^ (50 << 32) ^ i.wrapping_mul(0x9E37_79B1),
            512,
            384,
        );
        let scores: Vec<f32> = [0.0f32, 0.0025, 0.005, 0.01, 0.02]
            .iter()
            .map(|&p| {
                let img = if p == 0.0 {
                    m.clone()
                } else {
                    gaussian(&m, p * 512.0)
                };
                blur_score(&thumb_of(&img))
            })
            .collect();
        assert!(
            scores.iter().all(|&s| s < LOW_CONTRAST),
            "motiv {i}: {scores:?}"
        );
        assert!(
            scores.windows(2).all(|w| w[0] > w[1]),
            "motiv {i} ikke monotont: {scores:?}"
        );
    }
}

/// K3-3 og K3-4: presisjon ≥ 0,90 på uskarpe mot skarpe kontroller; lav kontrast flagges ikke.
#[test]
fn k3_3_and_k3_4_blur_precision_and_low_contrast() {
    let all = &corpus().1.entries;
    let (mut tp, mut fp, mut fneg) = (0, 0, 0);
    for e in all.iter().filter(|e| e.blurry.is_some()) {
        let score = blur_score(&corpus_thumb(e));
        let flagged = score < BLUR_ABS_MAX;
        match (e.blurry.unwrap(), flagged) {
            (true, true) => tp += 1,
            (true, false) => fneg += 1,
            (false, true) => {
                fp += 1;
                eprintln!("falsk positiv: {} score {score}", e.file);
            }
            _ => {}
        }
    }
    let precision = tp as f32 / (tp + fp) as f32;
    let recall = tp as f32 / (tp + fneg) as f32;
    eprintln!("K3-3: ABS_MAX {BLUR_ABS_MAX}, presisjon {precision:.3}, recall {recall:.3} (tp {tp}, fp {fp}, fn {fneg})");
    assert!(precision >= 0.90, "presisjon {precision}");
    // K3-4: de ti lavkontrast-kontrollene (hvit vegg, himmel) og degenererte er aldri flagget.
    let sharp = entries("sharp");
    assert_eq!(sharp.len(), 40);
    for e in sharp.iter().skip(30).chain(entries("degenerate").iter()) {
        let s = blur_score(&corpus_thumb(e));
        assert!(
            s >= BLUR_ABS_MAX,
            "{} flagget som uskarp, score {s}",
            e.file
        );
    }
    let low = sharp
        .iter()
        .skip(30)
        .filter(|e| blur_score(&corpus_thumb(e)) == LOW_CONTRAST)
        .count();
    eprintln!("K3-4: {low} av 10 lavkontrast-kontroller gir LOW_CONTRAST");
}

/// K3-5: samme motiv ved to oppløsninger (1024 og 512 px bredde, begge til 256 px miniatyr via ulik
/// nedskalering) gir score innen ±25 %. Korpusets 25 %-varianter gir bare 128 px miniatyr
/// (aldri oppskalert) og avviker mer; tallet rapporteres, se «Kjent begrensning» i PLAN-KJERNE 5.
#[test]
fn k3_5_scores_stable_across_resolutions() {
    let mut worst = 0.0f32;
    for i in 0..10u64 {
        let hi = scene(
            DEFAULT_SEED ^ (60 << 32) ^ i.wrapping_mul(0x9E37_79B1),
            1024,
            768,
        );
        let mid = resize_area(&hi, 512, 384);
        let (a, b) = (blur_score(&thumb_of(&hi)), blur_score(&thumb_of(&mid)));
        worst = worst.max((b / a - 1.0).abs());
    }
    let mut worst_corpus = 0.0f32;
    for d in entries("downscaled") {
        let o = entries("original")
            .into_iter()
            .find(|o| o.visual_group == d.visual_group)
            .unwrap();
        let dev = (blur_score(&corpus_thumb(d)) / blur_score(&corpus_thumb(o)) - 1.0).abs();
        worst_corpus = worst_corpus.max(dev);
    }
    eprintln!("K3-5: maks avvik 1024 mot 512 px = {worst:.3}; korpus 50 %/25 % mot original = {worst_corpus:.3}");
    assert!(worst <= 0.25, "avvik {worst}");
}

fn meta(e: &Entry) -> AssetMeta {
    AssetMeta {
        source_id: SourceId(e.file.clone()),
        kind: Kind::Photo,
        created_at: e.created_at,
        mod_token: e.fnv64.clone(),
        width: e.width,
        height: e.height,
        bytes: e.bytes,
        mime: e.mime.clone(),
        platform_screenshot_hint: e.platform_screenshot_hint,
        has_camera_exif: e.has_camera_exif,
        is_local: true,
        protect_flags: Default::default(),
        albums: vec![],
        backup: None,
    }
}

/// `None` = standardregelen (`suggest_default`); `Some(b)` = eksplisitt `require_strong`.
fn precision_recall(rule: Option<bool>) -> (f32, f32, usize, usize) {
    let (mut tp, mut fp, mut fneg) = (0usize, 0usize, 0usize);
    for e in corpus().1.entries.iter().filter(|e| e.screenshot.is_some()) {
        let s = signals(&meta(e), &corpus_thumb(e));
        let flagged = match rule {
            None => s.suggest_default(),
            Some(strong) => s.suggestion(strong),
        }
        .is_some();
        match (e.screenshot.unwrap(), flagged) {
            (true, true) => tp += 1,
            (true, false) => fneg += 1,
            (false, true) => fp += 1,
            _ => {}
        }
    }
    (
        tp as f32 / (tp + fp) as f32,
        tp as f32 / (tp + fneg) as f32,
        tp,
        fp,
    )
}

/// K4-2: presisjon ≥ 0,95 med standardkonfigurasjonen (`suggest_default`). Rå to-av-fire rapporteres
/// og dokumenterer hvorfor standarden er streng (presisjon under porten).
#[test]
fn k4_2_screenshot_precision() {
    assert!(fjell_core::screenshot::DEFAULT_REQUIRE_STRONG);
    let (p_raw, r_raw, _, fp_raw) = precision_recall(Some(false));
    let (p, r, tp, fp) = precision_recall(None);
    eprintln!(
        "K4-2 to av fire (ikke standard): presisjon {p_raw:.3}, recall {r_raw:.3} (fp {fp_raw})"
    );
    eprintln!("K4-2 standard: presisjon {p:.3}, recall {r:.3} (tp {tp}, fp {fp})");
    assert!(p >= 0.95, "presisjon {p}");
    assert!(r >= 0.95, "recall {r}");
    assert!(
        p_raw < 0.95,
        "to-av-fire skulle ligge under porten: {p_raw}"
    );
}

/// K4-3: PNG uten EXIF i vilkårlig størrelse flagges ikke med standardregelen; tallet uten rapporteres.
#[test]
fn k4_3_arbitrary_size_png_not_flagged_when_strong() {
    let neg: Vec<_> = entries("screenshot_neg")
        .into_iter()
        .filter(|e| e.mime == "image/png")
        .collect();
    assert_eq!(neg.len(), 10);
    let count = |strong: Option<bool>| {
        neg.iter()
            .filter(|e| {
                let s = signals(&meta(e), &corpus_thumb(e));
                match strong {
                    None => s.suggest_default(),
                    Some(b) => s.suggestion(b),
                }
                .is_some()
            })
            .count()
    };
    let (without, with) = (count(Some(false)), count(None));
    eprintln!("K4-3: PNG uten EXIF i vilkårlig størrelse flagget: {without} av 10 med to-av-fire, {with} med standardregelen");
    assert_eq!(with, 0);
}
