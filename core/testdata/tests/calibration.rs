//! Kalibrering (V1): feier terskler over det syntetiske korpuset og sjekker at kurvene oppfører seg som de skal
//! (monotone der de må være det), at Low/Normal/High er ordnet, at Normal ikke svekker presisjonen under
//! gulvene vi har målt, og at tabellene i docs/v1/KALIBRERING-V1.md er de som verktøyet skriver nå.
//!
//! Syntetisk korpus er ikke ekte bilder. Gulvene under er regresjonsvern for denne fasiten, ikke krav til
//! ekte ytelse. Tallene skrives med `--nocapture`; tabellene lages også av `examples/calibrate.rs`.

mod common;

use common::*;
use fjell_core::api::Sensitivity;
use fjell_core::model::SuggestionKind;
use fjell_core::series::SeriesParams;
use fjell_testdata::calib::{eye_cases, series_corpus, shot_cases, CALIB_SEED};

const DOC: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../docs/v1/KALIBRERING-V1.md"
);
const START: &str = "<!-- kalibrering:start -->";
const END: &str = "<!-- kalibrering:end -->";

fn nondecreasing(v: &[f64]) -> bool {
    v.windows(2).all(|w| w[0] <= w[1] + 1e-12)
}

#[test]
fn corpora_are_deterministic_and_labelled() {
    let (a, b) = (series_corpus(CALIB_SEED), series_corpus(CALIB_SEED));
    assert_eq!(a.frames.len(), b.frames.len());
    assert!(a
        .frames
        .iter()
        .zip(&b.frames)
        .all(|(x, y)| x.thumb == y.thumb && x.created_at == y.created_at && x.series == y.series));
    let other = series_corpus(CALIB_SEED ^ 1);
    assert!(
        a.frames
            .iter()
            .zip(&other.frames)
            .any(|(x, y)| x.thumb != y.thumb),
        "annen seed må gi andre bilder"
    );
    // Hver klasse er med, og tidene er strengt stigende innen en gruppe (ingen klokke, bare seed).
    for c in ["calm", "medium", "lively", "slow", "revisit", "walk"] {
        assert!(a.frames.iter().any(|f| f.class == c), "mangler klasse {c}");
    }
    assert!(a
        .frames
        .iter()
        .filter(|f| f.class == "walk")
        .all(|f| f.series.is_none()));
    assert!(a
        .frames
        .iter()
        .filter(|f| f.class != "walk")
        .all(|f| f.series.is_some()));
    assert_eq!(eye_cases(CALIB_SEED).len(), eye_cases(CALIB_SEED).len());
    assert!(eye_cases(CALIB_SEED)
        .iter()
        .all(|c| c.frames.iter().any(|f| !f.truth_closed)));
    assert!(shot_cases().iter().any(|c| c.truth) && shot_cases().iter().any(|c| !c.truth));
}

#[test]
fn visual_hamming_sweep() {
    let rows: Vec<VisualRow> = (2..=12).map(visual_row).collect();
    for r in &rows {
        eprintln!(
            "visuelt max_dist {:2}: P {} R {} falske {} serie-par slukt {}/{}",
            r.max_dist,
            pct(r.metric.precision()),
            pct(r.metric.recall()),
            r.metric.fp,
            r.burst_swallowed,
            r.burst_total
        );
    }
    // Recall vokser med terskelen så lenge kjedevernet (`max_span` = 10) ikke griper inn.
    let rec: Vec<f64> = rows
        .iter()
        .filter(|r| r.max_dist <= 10)
        .map(|r| r.metric.recall().unwrap())
        .collect();
    assert!(nondecreasing(&rec), "{rec:?}");
    // Falske par vokser (eller står) med terskelen.
    let fps: Vec<f64> = rows
        .iter()
        .filter(|r| r.max_dist <= 10)
        .map(|r| r.metric.fp as f64)
        .collect();
    assert!(nondecreasing(&fps), "{fps:?}");
    // Gulv for Low/Normal/High (nåværende fasit har ingen falske par; dette fanger regresjon).
    for d in [4u32, 6, 8] {
        let r = &rows[(d - 2) as usize];
        assert!(r.metric.precision().unwrap() >= 0.99, "d={d}");
    }
    // Serie-par slukt vokser fra Low til Normal: det er den reelle prisen for høyere terskel.
    assert!(rows[(4 - 2) as usize].burst_swallowed < rows[(6 - 2) as usize].burst_swallowed);
}

#[test]
fn blur_scale_sweep() {
    let rows: Vec<BlurRow> = (4..=20).map(|i| blur_row(i as f32 / 10.0)).collect();
    let flagged: Vec<f64> = rows
        .iter()
        .map(|r| (r.metric.tp + r.metric.fp) as f64)
        .collect();
    assert!(nondecreasing(&flagged), "flere flagges ved høyere terskel");
    let rec: Vec<f64> = rows.iter().map(|r| r.metric.recall().unwrap()).collect();
    assert!(nondecreasing(&rec), "{rec:?}");
    for r in &rows {
        eprintln!(
            "uskarpt x{:.1} (terskel {:.5}): P {} R {} feilflagg {} oversett {}",
            r.scale,
            r.threshold,
            pct(r.metric.precision()),
            pct(r.metric.recall()),
            r.metric.fp,
            r.metric.fn_
        );
    }
    let at = |s: f32| rows.iter().find(|r| (r.scale - s).abs() < 1e-6).unwrap();
    let (low, normal, high) = (at(0.7), at(1.0), at(1.3));
    assert!(low.metric.recall() <= normal.metric.recall());
    assert!(normal.metric.recall() < high.metric.recall());
    // Normal og Low holder presisjon over K8-porten (0,90); High betaler med presisjon for liten recall-vinning.
    assert!(low.metric.precision().unwrap() >= 0.95);
    assert!(normal.metric.precision().unwrap() >= 0.95);
    assert!(high.metric.precision().unwrap() >= 0.85);
    assert!(high.metric.precision() < normal.metric.precision());
    // P10 på korpuset ligger under taket fordi korpuset har så mange uskarpe (se `p10_threshold`).
    assert!(p10_threshold() < fjell_core::quality::BLUR_ABS_MAX);
}

#[test]
fn series_gap_and_hamming_sweeps() {
    let frames = series_frames();
    let gaps = [2u32, 3, 5, 8, 10, 15, 20, 30, 60, 300];
    let rows: Vec<SeriesRow> = gaps
        .iter()
        .map(|&g| {
            series_row(
                frames,
                SeriesParams {
                    gap_secs: g,
                    ..SeriesParams::default()
                },
            )
        })
        .collect();
    let rec: Vec<f64> = rows.iter().map(|r| r.metric.recall().unwrap()).collect();
    assert!(nondecreasing(&rec), "gap: {rec:?}");
    // Ingen falske par fra «gange» uansett glipp; «samme sted senere» (2–15 min) slipper først inn over 300 s.
    assert!(rows
        .iter()
        .all(|r| r.false_walk == 0 && r.false_revisit == 0));
    assert!(
        series_row(
            frames,
            SeriesParams {
                gap_secs: 1200,
                ..SeriesParams::default()
            }
        )
        .false_revisit
            > 0
    );

    let dists = [6u32, 8, 10, 12, 14, 16];
    let rows: Vec<SeriesRow> = dists
        .iter()
        .map(|&d| {
            series_row(
                frames,
                SeriesParams {
                    max_dist: d,
                    ..SeriesParams::default()
                },
            )
        })
        .collect();
    let rec: Vec<f64> = rows.iter().map(|r| r.metric.recall().unwrap()).collect();
    assert!(nondecreasing(&rec), "max_dist: {rec:?}");
    for (d, r) in dists.iter().zip(&rows) {
        eprintln!(
            "serie max_dist {d:2}: P {} R {} falske samme-sted {} gange {}",
            pct(r.metric.precision()),
            pct(r.metric.recall()),
            r.false_revisit,
            r.false_walk
        );
    }
}

#[test]
fn sensitivity_levels_are_ordered_per_kind() {
    let frames = series_frames();
    let eyes = eye_cases(CALIB_SEED);
    let levels = Sensitivity::ALL;
    let vis: Vec<VisualRow> = levels
        .iter()
        .map(|&l| visual_row(tuned(SuggestionKind::VisualDup, l).visual.max_dist))
        .collect();
    let blur: Vec<BlurRow> = levels
        .iter()
        .map(|&l| blur_row(tuned(SuggestionKind::Blurry, l).blur_scale))
        .collect();
    let ser: Vec<SeriesRow> = levels
        .iter()
        .map(|&l| series_row(frames, tuned(SuggestionKind::Series, l).series))
        .collect();
    let eye: Vec<EyeRow> = levels
        .iter()
        .map(|&l| {
            eye_row(
                &eyes,
                tuned(SuggestionKind::Series, l).series.eyes_min_confidence,
            )
        })
        .collect();
    let rec = |m: &[Metric]| -> Vec<f64> { m.iter().map(|x| x.recall().unwrap()).collect() };
    assert!(nondecreasing(&rec(&vis
        .iter()
        .map(|r| r.metric)
        .collect::<Vec<_>>())));
    assert!(nondecreasing(&rec(&blur
        .iter()
        .map(|r| r.metric)
        .collect::<Vec<_>>())));
    assert!(nondecreasing(&rec(&ser
        .iter()
        .map(|r| r.metric)
        .collect::<Vec<_>>())));
    // Øyne: Low (0,9) merker færre lukkede enn Normal (0,8) og High (0,7): recall stiger, presisjon faller.
    assert!(nondecreasing(&rec(&eye
        .iter()
        .map(|r| r.metric)
        .collect::<Vec<_>>())));
    let prec: Vec<f64> = eye.iter().map(|r| r.metric.precision().unwrap()).collect();
    assert!(prec[0] >= prec[1] && prec[1] >= prec[2], "{prec:?}");
    // Serier er presise på alle nivåer (ingen falske par fra «gange» og «samme sted senere»).
    for r in &ser {
        assert_eq!(r.false_walk + r.false_revisit, 0);
    }
    // Uskarphet: presisjonen faller fra Low til High, og Low og Normal er over 0,95.
    let bp: Vec<f64> = blur.iter().map(|r| r.metric.precision().unwrap()).collect();
    assert!(bp[0] >= bp[1] && bp[1] >= bp[2], "{bp:?}");
    assert!(bp[1] >= 0.95, "{bp:?}");
}

#[test]
fn eye_placeholder_and_screenshot_rules() {
    let cases = eye_cases(CALIB_SEED);
    let rows: Vec<EyeRow> = [0.5f32, 0.6, 0.7, 0.8, 0.9, 0.95]
        .iter()
        .map(|&c| eye_row(&cases, c))
        .collect();
    let rec: Vec<f64> = rows.iter().map(|r| r.metric.recall().unwrap()).collect();
    assert!(
        rec.windows(2).all(|w| w[0] >= w[1]),
        "recall faller med sikkerhetskravet: {rec:?}"
    );
    let bad: Vec<f64> = rows.iter().map(|r| r.keeper_closed as f64).collect();
    assert!(
        nondecreasing(&bad),
        "keeper med lukkede øyne vokser: {bad:?}"
    );
    assert!(rows[3].metric.precision().unwrap() > rows[0].metric.precision().unwrap());
    // Skjermbilde: standardregelen (S1 eller S2 kreves) er mer presis, den løse tar flere.
    let (strong, loose) = (shot_row(true), shot_row(false));
    assert!(strong.metric.precision() > loose.metric.precision());
    assert!(strong.metric.recall() < loose.metric.recall());
    assert!(strong.metric.precision().unwrap() >= 0.9);
}

/// Tabellene i dokumentet er de verktøyet skriver nå. Oppdater med
/// `FJELL_UPDATE_CALIBRATION=1 cargo test --release --test calibration calibration_doc_is_current`
/// eller lim inn utskriften av `cargo run --release --locked --example calibrate`.
#[test]
fn calibration_doc_is_current() {
    let doc = std::fs::read_to_string(DOC).expect("docs/v1/KALIBRERING-V1.md");
    let (a, b) = (
        doc.find(START).expect("start-markør") + START.len(),
        doc.find(END).expect("slutt-markør"),
    );
    let fresh = report();
    if std::env::var("FJELL_UPDATE_CALIBRATION").is_ok() {
        let new = format!("{}\n\n{}\n{}", &doc[..a], fresh.trim_end(), &doc[b..]);
        std::fs::write(DOC, new).unwrap();
        return;
    }
    assert_eq!(
        doc[a..b].trim(),
        fresh.trim(),
        "KALIBRERING-V1.md er ikke i takt med korpuset. Kjør: FJELL_UPDATE_CALIBRATION=1 cargo test --release --test calibration calibration_doc_is_current"
    );
}
