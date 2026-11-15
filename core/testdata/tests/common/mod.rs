//! Felles kode for kalibreringen: laster korpusene, feier terskler og bygger rapporten som står i
//! docs/v1/KALIBRERING-V1.md. Brukes av `tests/calibration.rs` og `examples/calibrate.rs`.
//!
//! Alt går gjennom kjernens egne funksjoner (`cluster::visual_groups`, `series::build_series`,
//! `suggest::blur_threshold`/`blurry_suggestion`, `screenshot::signals`, `pipeline::tuning`); ingenting her
//! gjenskaper algoritmer. Korpusene er syntetiske (aldri ekte bilder).
#![allow(dead_code)]

use fjell_core::api::Sensitivity;
use fjell_core::cluster::{visual_groups, Params};
use fjell_core::hash::{low_info, perceptual};
use fjell_core::model::{
    AssetId, AssetMeta, EyeReport, Hash64, Kind, ProtectFlags, SourceId, SuggestionKind, Thumb,
    ThumbFmt,
};
use fjell_core::pipeline::{tuning, Tuning};
use fjell_core::quality::{blur_score, BLUR_ABS_MAX};
use fjell_core::screenshot::signals;
use fjell_core::series::{build_series, rank_series, SeriesItem, SeriesParams};
use fjell_core::suggest::{blur_threshold, blurry_suggestion};
use fjell_testdata::blurset::{generate_blur, BLUR_SEED};
use fjell_testdata::calib::{eye_cases, series_corpus, shot_cases, EyeCase, Gray, CALIB_SEED};
use fjell_testdata::corpus::{generate, DEFAULT_SEED};
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::sync::OnceLock;

pub type Pair = (u64, u64);

/// Midlertidig mappe: `CARGO_TARGET_TMPDIR` i tester, ellers systemets temp (eksempelet).
pub fn work_dir(name: &str) -> PathBuf {
    let base = option_env!("CARGO_TARGET_TMPDIR")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    let p = base.join(name);
    let _ = std::fs::remove_dir_all(&p);
    p
}

pub fn thumb_of(g: &Gray) -> Thumb {
    Thumb::new(g.w as u32, g.h as u32, ThumbFmt::Gray8, g.px.clone()).unwrap()
}

/// Presisjon/recall fra tellere. `None` når nevneren er 0 (ingen forslag, eller ingenting å finne).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Metric {
    pub tp: u64,
    pub fp: u64,
    pub fn_: u64,
}

impl Metric {
    pub fn precision(&self) -> Option<f64> {
        (self.tp + self.fp > 0).then(|| self.tp as f64 / (self.tp + self.fp) as f64)
    }
    pub fn recall(&self) -> Option<f64> {
        (self.tp + self.fn_ > 0).then(|| self.tp as f64 / (self.tp + self.fn_) as f64)
    }
    pub fn from_pairs(truth: &BTreeSet<Pair>, got: &BTreeSet<Pair>) -> Metric {
        let tp = truth.intersection(got).count() as u64;
        Metric {
            tp,
            fp: got.len() as u64 - tp,
            fn_: truth.len() as u64 - tp,
        }
    }
}

pub fn pct(v: Option<f64>) -> String {
    v.map_or("–".into(), |x| format!("{x:.3}"))
}

pub fn pairs_of<'a>(groups: impl Iterator<Item = &'a Vec<u64>>) -> BTreeSet<Pair> {
    let mut s = BTreeSet::new();
    for g in groups {
        for (i, a) in g.iter().enumerate() {
            for b in &g[i + 1..] {
                s.insert((*a.min(b), *a.max(b)));
            }
        }
    }
    s
}

// --- korpus: golden (visuelle duplikater og skjermbilder) --------------------------------------------

pub struct GItem {
    pub id: AssetId,
    pub dhash: Hash64,
    pub phash: Hash64,
    pub visual_group: Option<u32>,
}

/// Golden-korpuset redusert til det Fjell ville gruppert visuelt: ikke flatt, én representant per identisk
/// innhold (eksakte kopier er lette treff og hører til en annen forslagstype).
pub fn golden_items() -> &'static Vec<GItem> {
    static C: OnceLock<Vec<GItem>> = OnceLock::new();
    C.get_or_init(|| {
        let dir = work_dir("calib_golden");
        let m = generate(&dir, DEFAULT_SEED).unwrap();
        let mut seen_exact = BTreeSet::new();
        let mut out = Vec::new();
        for (i, e) in m.entries.iter().enumerate() {
            if let Some(g) = e.exact_group {
                if !seen_exact.insert(g) {
                    continue;
                }
            }
            let img = image::open(dir.join(&e.thumb)).unwrap().to_luma8();
            let t = Thumb::new(img.width(), img.height(), ThumbFmt::Gray8, img.into_raw()).unwrap();
            if low_info(&t) {
                continue;
            }
            let (dhash, phash) = perceptual(&t);
            out.push(GItem {
                id: AssetId(i as u64 + 1),
                dhash,
                phash,
                visual_group: e.visual_group,
            });
        }
        out
    })
}

// --- korpus: uskarphet -------------------------------------------------------------------------------

pub struct BItem {
    pub score: f32,
    pub blurry: bool,
    pub assessable: bool,
}

pub fn blur_items() -> &'static Vec<BItem> {
    static C: OnceLock<Vec<BItem>> = OnceLock::new();
    C.get_or_init(|| {
        let dir = work_dir("calib_blurset");
        let m = generate_blur(&dir, BLUR_SEED).unwrap();
        m.entries
            .iter()
            .map(|e| {
                let g = image::open(dir.join(&e.thumb)).unwrap().to_luma8();
                let t = Thumb::new(g.width(), g.height(), ThumbFmt::Gray8, g.into_raw()).unwrap();
                BItem {
                    score: blur_score(&t),
                    blurry: e.blurry,
                    assessable: e.assessable,
                }
            })
            .collect()
    })
}

// --- korpus: serier ----------------------------------------------------------------------------------

pub struct SFrame {
    pub item: SeriesItem,
    pub series: Option<u32>,
    pub class: &'static str,
}

pub fn series_frames_from(seed: u64) -> Vec<SFrame> {
    series_corpus(seed)
        .frames
        .iter()
        .enumerate()
        .map(|(i, f)| {
            let t = thumb_of(&f.thumb);
            let (dhash, phash) = perceptual(&t);
            SFrame {
                item: SeriesItem {
                    id: AssetId(i as u64 + 1),
                    taken_at: f.created_at,
                    dhash,
                    phash,
                    blur: blur_score(&t),
                    eyes: None,
                },
                series: f.series,
                class: f.class,
            }
        })
        .collect()
}

pub fn series_frames() -> &'static Vec<SFrame> {
    static C: OnceLock<Vec<SFrame>> = OnceLock::new();
    C.get_or_init(|| series_frames_from(CALIB_SEED))
}

// --- feiing ------------------------------------------------------------------------------------------

pub struct VisualRow {
    pub max_dist: u32,
    pub metric: Metric,
    /// Par i samme (sann) serie som ville blitt slukt som visuelt duplikat, og antall slike par totalt.
    pub burst_swallowed: u64,
    pub burst_total: u64,
}

pub fn visual_row(max_dist: u32) -> VisualRow {
    let p = Params {
        max_dist,
        ..Params::default()
    };
    let g = golden_items();
    let by_group: BTreeMap<u32, Vec<u64>> = g.iter().fold(BTreeMap::new(), |mut m, i| {
        if let Some(v) = i.visual_group {
            m.entry(v).or_default().push(i.id.0);
        }
        m
    });
    let truth = pairs_of(by_group.values());
    let hs: Vec<_> = g.iter().map(|i| (i.id, i.dhash, i.phash)).collect();
    let got_groups: Vec<Vec<u64>> = visual_groups(&hs, &p)
        .into_iter()
        .map(|g| g.members.iter().map(|a| a.0).collect())
        .collect();
    let metric = Metric::from_pairs(&truth, &pairs_of(got_groups.iter()));
    // Serie-korpuset: hvor mange par fra samme serie ville visuell klynging ha tatt?
    let s = series_frames();
    let sh: Vec<_> = s
        .iter()
        .map(|f| (f.item.id, f.item.dhash, f.item.phash))
        .collect();
    let swallowed = pairs_of(
        visual_groups(&sh, &p)
            .into_iter()
            .map(|g| g.members.iter().map(|a| a.0).collect::<Vec<_>>())
            .collect::<Vec<_>>()
            .iter(),
    );
    let burst_pairs = series_truth_pairs(s.iter().collect::<Vec<_>>().as_slice());
    VisualRow {
        max_dist,
        metric,
        burst_swallowed: swallowed.intersection(&burst_pairs).count() as u64,
        burst_total: burst_pairs.len() as u64,
    }
}

/// Appens terskel `min(P10, BLUR_ABS_MAX)` regnet over hele uskarphetskorpuset. Korpuset har ca. 45 % uskarpe
/// bilder, så P10 blir langt under taket; i et vanlig bibliotek (få uskarpe) er P10 over taket og taket gjelder.
/// Sveipet bruker derfor taket som grunnlag.
pub fn p10_threshold() -> f32 {
    let scores: Vec<f32> = blur_items().iter().map(|i| i.score).collect();
    blur_threshold(&scores)
}

pub struct BlurRow {
    pub scale: f32,
    pub threshold: f32,
    pub metric: Metric,
}

/// Terskel = `BLUR_ABS_MAX * skala` (taket i `blur_threshold`; se `p10_threshold` for hvorfor ikke P10). Bare
/// vurderbare bilder telles.
pub fn blur_row(scale: f32) -> BlurRow {
    let items = blur_items();
    let threshold = BLUR_ABS_MAX * scale;
    let mut m = Metric::default();
    for i in items.iter().filter(|i| i.assessable) {
        match (blurry_suggestion(i.score, threshold).is_some(), i.blurry) {
            (true, true) => m.tp += 1,
            (true, false) => m.fp += 1,
            (false, true) => m.fn_ += 1,
            _ => {}
        }
    }
    BlurRow {
        scale,
        threshold,
        metric: m,
    }
}

pub fn series_truth_pairs(frames: &[&SFrame]) -> BTreeSet<Pair> {
    let mut by: BTreeMap<u32, Vec<u64>> = BTreeMap::new();
    for f in frames {
        if let Some(s) = f.series {
            by.entry(s).or_default().push(f.item.id.0);
        }
    }
    pairs_of(by.values())
}

pub struct SeriesRow {
    pub params: SeriesParams,
    pub metric: Metric,
    /// Per klasse: (funnet, totalt) blant sanne seriepar.
    pub per_class: BTreeMap<&'static str, (u64, u64)>,
    /// Falske par fra «samme sted senere» og «gange».
    pub false_revisit: u64,
    pub false_walk: u64,
    /// Bilder som alt er visuelle duplikater (Normal) og dermed ikke går til serier, slik appen gjør det.
    pub absorbed: usize,
}

pub fn series_row(frames: &[SFrame], p: SeriesParams) -> SeriesRow {
    let hs: Vec<_> = frames
        .iter()
        .map(|f| (f.item.id, f.item.dhash, f.item.phash))
        .collect();
    let taken: BTreeSet<u64> = visual_groups(&hs, &Params::default())
        .iter()
        .flat_map(|g| g.members.iter().map(|a| a.0))
        .collect();
    let rest: Vec<&SFrame> = frames
        .iter()
        .filter(|f| !taken.contains(&f.item.id.0))
        .collect();
    let items: Vec<SeriesItem> = rest.iter().map(|f| f.item).collect();
    let got: BTreeSet<Pair> = pairs_of(
        build_series(&items, &p)
            .iter()
            .map(|s| s.members.iter().map(|m| m.id.0).collect::<Vec<_>>())
            .collect::<Vec<_>>()
            .iter(),
    );
    let truth = series_truth_pairs(&rest);
    let class_of: BTreeMap<u64, &'static str> =
        frames.iter().map(|f| (f.item.id.0, f.class)).collect();
    let mut per_class: BTreeMap<&'static str, (u64, u64)> = BTreeMap::new();
    for t in &truth {
        let e = per_class.entry(class_of[&t.0]).or_default();
        e.1 += 1;
        e.0 += got.contains(t) as u64;
    }
    let count_false = |c: &str| {
        got.iter()
            .filter(|p| !truth.contains(p) && class_of[&p.0] == c)
            .count() as u64
    };
    SeriesRow {
        params: p,
        metric: Metric::from_pairs(&truth, &got),
        per_class,
        false_revisit: count_false("revisit"),
        false_walk: count_false("walk"),
        absorbed: taken.len(),
    }
}

pub struct EyeRow {
    pub min_conf: f32,
    /// Merking «lukkede øyne» per bilde (sant = foreslått nedprioritert).
    pub metric: Metric,
    /// Serier der keeper har lukkede øyne selv om et bilde med åpne øyne finnes, og antall slike serier.
    pub keeper_closed: u64,
    pub cases: u64,
}

/// PLASSHOLDER: bruker den antatte fordelingen i `calib::eye_cases`, ikke en modell.
pub fn eye_row(cases: &[EyeCase], min_conf: f32) -> EyeRow {
    let mut m = Metric::default();
    let (mut bad, mut n) = (0u64, 0u64);
    for c in cases {
        let items: Vec<SeriesItem> = c
            .frames
            .iter()
            .enumerate()
            .map(|(i, f)| SeriesItem {
                id: AssetId(i as u64 + 1),
                taken_at: i as i64,
                dhash: Hash64(0),
                phash: Hash64(0),
                blur: f.sharpness,
                eyes: Some(EyeReport {
                    faces: 1,
                    closed: f.reported_closed as u8,
                    confidence: f.confidence,
                }),
            })
            .collect();
        for f in &c.frames {
            let demoted = f.reported_closed && f.confidence >= min_conf;
            match (demoted, f.truth_closed) {
                (true, true) => m.tp += 1,
                (true, false) => m.fp += 1,
                (false, true) => m.fn_ += 1,
                _ => {}
            }
        }
        let p = SeriesParams {
            eyes_min_confidence: min_conf,
            ..SeriesParams::default()
        };
        let s = rank_series(&items, &p).unwrap();
        if c.frames.iter().any(|f| !f.truth_closed) {
            n += 1;
            bad += c.frames[s.keeper.0 as usize - 1].truth_closed as u64;
        }
    }
    EyeRow {
        min_conf,
        metric: m,
        keeper_closed: bad,
        cases: n,
    }
}

pub struct ShotRow {
    pub require_strong: bool,
    pub metric: Metric,
}

pub fn shot_row(require_strong: bool) -> ShotRow {
    let dummy = Thumb::new(8, 8, ThumbFmt::Gray8, vec![0; 64]).unwrap();
    let mut m = Metric::default();
    for c in shot_cases() {
        let meta = AssetMeta {
            source_id: SourceId(c.name.into()),
            kind: Kind::Photo,
            created_at: 0,
            mod_token: "1".into(),
            width: c.width,
            height: c.height,
            bytes: 1,
            mime: c.mime.into(),
            platform_screenshot_hint: c.platform_hint,
            has_camera_exif: c.camera_exif,
            is_local: true,
            protect_flags: ProtectFlags::default(),
            albums: vec![],
            backup: None,
        };
        let flagged = signals(&meta, &dummy).suggestion(require_strong).is_some();
        let n = c.count as u64;
        match (flagged, c.truth) {
            (true, true) => m.tp += n,
            (true, false) => m.fp += n,
            (false, true) => m.fn_ += n,
            _ => {}
        }
    }
    ShotRow {
        require_strong,
        metric: m,
    }
}

/// Effektive terskler for ett nivå på én forslagstype (resten Normal).
pub fn tuned(kind: SuggestionKind, level: Sensitivity) -> Tuning {
    tuning(|k| (k == kind).then(|| level.as_str().to_string()))
}

// --- rapport -----------------------------------------------------------------------------------------

fn table(head: &[&str], rows: Vec<Vec<String>>) -> String {
    let mut s = format!("| {} |\n|{}\n", head.join(" | "), "---|".repeat(head.len()));
    for r in rows {
        s.push_str(&format!("| {} |\n", r.join(" | ")));
    }
    s
}

fn level_name(l: Sensitivity) -> &'static str {
    match l {
        Sensitivity::Low => "Low",
        Sensitivity::Normal => "Normal",
        Sensitivity::High => "High",
    }
}

fn mark(values: &[(Sensitivity, u32)], v: u32) -> String {
    let m: Vec<&str> = values
        .iter()
        .filter(|(_, x)| *x == v)
        .map(|(l, _)| level_name(*l))
        .collect();
    m.join(", ")
}

/// Hele rapporten (Markdown). Samme tekst skrives av `examples/calibrate.rs` og sjekkes mot
/// docs/v1/KALIBRERING-V1.md av `tests/calibration.rs`.
pub fn report() -> String {
    let mut out = String::new();
    let levels = Sensitivity::ALL;

    // 1. Visuelt duplikat
    let vis: Vec<(Sensitivity, u32)> = levels
        .iter()
        .map(|&l| (l, tuned(SuggestionKind::VisualDup, l).visual.max_dist))
        .collect();
    out.push_str("#### Visuelt duplikat: Hamming (`max_dist`)\n\n");
    let rows = (2..=12)
        .map(|d| {
            let r = visual_row(d);
            vec![
                d.to_string(),
                mark(&vis, d),
                pct(r.metric.precision()),
                pct(r.metric.recall()),
                r.metric.fp.to_string(),
                format!("{}/{}", r.burst_swallowed, r.burst_total),
            ]
        })
        .collect();
    out.push_str(&table(
        &[
            "max_dist",
            "nivå",
            "presisjon",
            "recall",
            "falske par",
            "serie-par slukt",
        ],
        rows,
    ));

    // 2. Uskarphet
    let blur_levels: Vec<(Sensitivity, u32)> = levels
        .iter()
        .map(|&l| {
            let s = tuned(SuggestionKind::Blurry, l).blur_scale;
            (l, (s * 100.0).round() as u32)
        })
        .collect();
    out.push_str(
        "\n#### Uskarphet: terskelfaktor (`blur_scale`, grunnlag `BLUR_ABS_MAX` = 0,006)\n\n",
    );
    let rows = (4..=20)
        .map(|i| {
            let r = blur_row(i as f32 / 10.0);
            vec![
                format!("{:.1}", i as f32 / 10.0),
                mark(&blur_levels, i * 10),
                format!("{:.5}", r.threshold),
                pct(r.metric.precision()),
                pct(r.metric.recall()),
                r.metric.fp.to_string(),
                r.metric.fn_.to_string(),
            ]
        })
        .collect();
    out.push_str(&table(
        &[
            "faktor",
            "nivå",
            "terskel",
            "presisjon",
            "recall",
            "feilflagg",
            "oversett",
        ],
        rows,
    ));

    // 3. Serier
    let frames = series_frames();
    out.push_str("\n#### Serier: glipp i tid (`gap_secs`, `max_dist` = 10)\n\n");
    let gaps: Vec<(Sensitivity, u32)> = levels
        .iter()
        .map(|&l| (l, tuned(SuggestionKind::Series, l).series.gap_secs))
        .collect();
    let rows = [2u32, 3, 5, 8, 10, 15, 20, 30, 60, 300, 1200]
        .into_iter()
        .map(|g| {
            let r = series_row(
                frames,
                SeriesParams {
                    gap_secs: g,
                    ..SeriesParams::default()
                },
            );
            vec![
                g.to_string(),
                mark(&gaps, g),
                pct(r.metric.precision()),
                pct(r.metric.recall()),
                r.false_revisit.to_string(),
                r.false_walk.to_string(),
            ]
        })
        .collect();
    out.push_str(&table(
        &[
            "gap_secs",
            "nivå",
            "presisjon",
            "recall",
            "falske par (samme sted senere)",
            "falske par (gange)",
        ],
        rows,
    ));
    out.push_str("\n#### Serier: Hamming (`max_dist`, `gap_secs` = 10)\n\n");
    let sd: Vec<(Sensitivity, u32)> = levels
        .iter()
        .map(|&l| (l, tuned(SuggestionKind::Series, l).series.max_dist))
        .collect();
    let rows = (4..=16)
        .map(|d| {
            let r = series_row(
                frames,
                SeriesParams {
                    max_dist: d,
                    ..SeriesParams::default()
                },
            );
            vec![
                d.to_string(),
                mark(&sd, d),
                pct(r.metric.precision()),
                pct(r.metric.recall()),
                r.false_revisit.to_string(),
                r.false_walk.to_string(),
            ]
        })
        .collect();
    out.push_str(&table(
        &[
            "max_dist",
            "nivå",
            "presisjon",
            "recall",
            "falske par (samme sted senere)",
            "falske par (gange)",
        ],
        rows,
    ));

    // 4. Øyne (plassholder)
    out.push_str("\n#### Øyesikkerhet (PLASSHOLDER, antatt fordeling, ingen modell)\n\n");
    let cases = eye_cases(CALIB_SEED);
    let eyes: Vec<(Sensitivity, u32)> = levels
        .iter()
        .map(|&l| {
            let c = tuned(SuggestionKind::Series, l).series.eyes_min_confidence;
            (l, (c * 100.0).round() as u32)
        })
        .collect();
    let rows = [50u32, 60, 70, 80, 90, 95]
        .into_iter()
        .map(|c| {
            let r = eye_row(&cases, c as f32 / 100.0);
            vec![
                format!("{:.2}", c as f32 / 100.0),
                mark(&eyes, c),
                pct(r.metric.precision()),
                pct(r.metric.recall()),
                format!("{}/{}", r.keeper_closed, r.cases),
            ]
        })
        .collect();
    out.push_str(&table(
        &[
            "min. sikkerhet",
            "nivå",
            "presisjon",
            "recall",
            "keeper med lukkede øyne",
        ],
        rows,
    ));

    // 5. Skjermbilde
    out.push_str("\n#### Skjermbilde: `require_strong` (ingen sensitivitet)\n\n");
    let rows = [true, false]
        .into_iter()
        .map(|s| {
            let r = shot_row(s);
            vec![
                if s { "ja (standard)" } else { "nei" }.into(),
                pct(r.metric.precision()),
                pct(r.metric.recall()),
                r.metric.fp.to_string(),
                r.metric.fn_.to_string(),
            ]
        })
        .collect();
    out.push_str(&table(
        &[
            "require_strong",
            "presisjon",
            "recall",
            "feilflagg",
            "oversett",
        ],
        rows,
    ));

    // 6. Samlet per sensitivitet
    out.push_str("\n#### Samlet per sensitivitet (dagens startverdier)\n\n");
    let mut rows = Vec::new();
    for &l in &levels {
        let v = visual_row(tuned(SuggestionKind::VisualDup, l).visual.max_dist);
        let b = blur_row(tuned(SuggestionKind::Blurry, l).blur_scale);
        let sp = tuned(SuggestionKind::Series, l).series;
        let s = series_row(frames, sp);
        let e = eye_row(&cases, sp.eyes_min_confidence);
        rows.push(vec![
            level_name(l).into(),
            format!("{} / {}", pct(v.metric.precision()), pct(v.metric.recall())),
            format!("{} / {}", pct(b.metric.precision()), pct(b.metric.recall())),
            format!("{} / {}", pct(s.metric.precision()), pct(s.metric.recall())),
            format!("{} / {}", pct(e.metric.precision()), pct(e.metric.recall())),
        ]);
    }
    out.push_str(&table(
        &[
            "nivå",
            "visuelt (P / R)",
            "uskarpt (P / R)",
            "serie (P / R)",
            "øyne, plassholder (P / R)",
        ],
        rows,
    ));
    out
}
