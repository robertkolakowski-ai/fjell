//! `fjell-cli calibrate`: måler kjernens algoritmer mot et etikettert korpus (MVP-restanse 6/7).
//!
//! Grensesnitt: `run(dir, labels_csv, &Options, out) -> Result<Report, FlowError>`, `parse_labels`.
//! Etikettformatet står i `docs/mvp/KALIBRERING.md`. Kort: CSV med overskrift og kolonnene
//! `fil` (påkrevd, sti relativt til mappen med `/`), `visuell_gruppe`, `eksakt_gruppe` og `uskarp`
//! (minst én av de tre siste).
//!
//! Målt, med kjernens egne funksjoner og uendrede algoritmer (parametrene kan overstyres for å
//! sammenligne kandidater, f.eks. `max_span` 12 og `tight_dist` 5):
//! - eksakte duplikater: BLAKE3 over filbytes (`fjell_core::cluster::exact_groups`);
//! - visuell gruppering: `hash::perceptual` + `cluster::visual::visual_groups`, uten bilder med
//!   lav informasjon (som i skanningen). Målt både på alle filer og på én representant per
//!   identisk innhold (ellers teller eksakte kopier med som lette treff);
//! - uskarphet: `quality::blur_score` mot terskelen skanningen bruker (`blur_threshold`, P10 og
//!   `BLUR_ABS_MAX`) og mot den faste `BLUR_ABS_MAX`.
//!
//! Presisjon og recall er på parnivå (alle par i samme gruppe), som K8-testene i `core/testdata`.
//! Originalfiler leses bare (`AssetSource::read_bytes` og `thumbnail`); ingenting skrives, ingen
//! nettverk, ingen indeks. Bare JPEG og PNG dekodes i dag (beslutning 6): andre formater telles
//! som «ikke dekodbare» og kan ikke kalibreres før dekoderne finnes.

use crate::flow::FlowError;
use fjell_core::cluster::{exact_groups, visual_groups, Params};
use fjell_core::hash::{content_hash, low_info, perceptual};
use fjell_core::model::{AssetId, AssetSource, Hash64, SourceError, SourceId, ALGO_VER};
use fjell_core::quality::{blur_score, BLUR_ABS_MAX, LOW_CONTRAST};
use fjell_core::suggest::blur_threshold;
use std::collections::{BTreeMap, BTreeSet};
use std::io::Write;
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

/// Valg for kjøringen.
#[derive(Clone, Debug)]
pub struct Options {
    /// Terskler for visuell gruppering. Standard er kjernens `Params::default()`.
    pub params: Params,
    /// Høyeste antall rader i tabellene (falske sammenslåinger og uskarphetsfeil).
    pub rows: usize,
}

impl Default for Options {
    fn default() -> Options {
        Options {
            params: Params::default(),
            rows: 50,
        }
    }
}

/// Én rad i etikettfilen. `None` i en kolonne som finnes betyr tom celle.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Label {
    pub file: String,
    pub visual: Option<String>,
    pub exact: Option<String>,
    pub blurry: Option<bool>,
}

/// Etikettfilen: rader og hvilke kolonner som fantes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Labels {
    pub rows: Vec<Label>,
    pub has_visual: bool,
    pub has_exact: bool,
    pub has_blurry: bool,
}

fn usage(s: impl Into<String>) -> FlowError {
    FlowError::Usage(s.into())
}

fn fail(s: impl std::fmt::Display) -> FlowError {
    FlowError::Failed(s.to_string())
}

/// Deler én linje i felt. Støtter «…»-felt med `""` som anførselstegn; ingen linjeskift i felt.
fn split_line(line: &str, d: char) -> Vec<String> {
    let (mut out, mut cur, mut quoted) = (Vec::new(), String::new(), false);
    let mut it = line.chars().peekable();
    while let Some(c) = it.next() {
        match c {
            '"' if quoted && it.peek() == Some(&'"') => {
                cur.push('"');
                it.next();
            }
            '"' => quoted = !quoted,
            c if c == d && !quoted => out.push(std::mem::take(&mut cur).trim().to_string()),
            c => cur.push(c),
        }
    }
    out.push(cur.trim().to_string());
    out
}

fn norm_path(s: &str) -> String {
    let s = s.replace('\\', "/");
    s.strip_prefix("./").unwrap_or(&s).to_string()
}

fn parse_bool(s: &str, line: usize) -> Result<Option<bool>, FlowError> {
    match s.to_lowercase().as_str() {
        "" => Ok(None),
        "ja" | "j" | "yes" | "y" | "true" | "1" => Ok(Some(true)),
        "nei" | "n" | "no" | "false" | "0" => Ok(Some(false)),
        v => Err(usage(format!(
            "etikettfil linje {line}: ugyldig verdi «{v}» i uskarp (bruk ja eller nei, eller la stå tom)"
        ))),
    }
}

/// Leser etikettfilen. Skilletegn (komma, semikolon eller tabulator) velges fra overskriften, så
/// CSV fra norsk Excel (semikolon) virker. Overskrift er påkrevd; navnene kan være norske eller engelske.
pub fn parse_labels(text: &str) -> Result<Labels, FlowError> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let mut lines = text
        .lines()
        .enumerate()
        .map(|(i, l)| (i + 1, l))
        .filter(|(_, l)| !l.trim().is_empty());
    let Some((_, head)) = lines.next() else {
        return Err(usage("etikettfilen er tom"));
    };
    let d = [',', ';', '\t']
        .into_iter()
        .max_by_key(|c| (head.matches(*c).count(), *c == ','))
        .unwrap_or(',');
    let names: Vec<String> = split_line(head, d)
        .iter()
        .map(|s| s.to_lowercase())
        .collect();
    let col = |alts: &[&str]| names.iter().position(|n| alts.contains(&n.as_str()));
    let c_file =
        col(&["fil", "file"]).ok_or_else(|| usage("etikettfilen mangler kolonnen «fil»"))?;
    let c_vis = col(&["visuell_gruppe", "visual_group"]);
    let c_exact = col(&["eksakt_gruppe", "exact_group"]);
    let c_blur = col(&["uskarp", "blurry"]);
    if c_vis.is_none() && c_exact.is_none() && c_blur.is_none() {
        return Err(usage(
            "etikettfilen trenger minst én av visuell_gruppe, eksakt_gruppe, uskarp",
        ));
    }
    let mut rows = Vec::new();
    let mut seen = BTreeSet::new();
    for (n, line) in lines {
        let f = split_line(line, d);
        let get = |c: Option<usize>| c.and_then(|i| f.get(i)).filter(|s| !s.is_empty()).cloned();
        let Some(file) = get(Some(c_file)).map(|s| norm_path(&s)) else {
            return Err(usage(format!("etikettfil linje {n}: tom filsti")));
        };
        if !seen.insert(file.clone()) {
            return Err(usage(format!(
                "etikettfil linje {n}: {file} står to ganger"
            )));
        }
        rows.push(Label {
            file,
            visual: get(c_vis),
            exact: get(c_exact),
            blurry: parse_bool(&get(c_blur).unwrap_or_default(), n)?,
        });
    }
    Ok(Labels {
        rows,
        has_visual: c_vis.is_some(),
        has_exact: c_exact.is_some(),
        has_blurry: c_blur.is_some(),
    })
}

/// Presisjon og recall på parnivå.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PairStats {
    pub truth_pairs: u64,
    pub found_pairs: u64,
    pub true_pairs: u64,
}

impl PairStats {
    pub fn precision(&self) -> Option<f64> {
        (self.found_pairs > 0).then(|| self.true_pairs as f64 / self.found_pairs as f64)
    }
    pub fn recall(&self) -> Option<f64> {
        (self.truth_pairs > 0).then(|| self.true_pairs as f64 / self.truth_pairs as f64)
    }
}

/// Et par som ble slått sammen uten å høre sammen i fasiten.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FalsePair {
    pub a: String,
    pub b: String,
    /// Høyeste Hamming av dHash og pHash.
    pub dist: u32,
    pub truth_a: String,
    pub truth_b: String,
    pub group_size: usize,
}

/// Uskarphet mot én terskel.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct BlurStats {
    pub threshold: f32,
    pub tp: u64,
    pub fp: u64,
    pub fn_: u64,
    pub tn: u64,
}

impl BlurStats {
    pub fn precision(&self) -> Option<f64> {
        (self.tp + self.fp > 0).then(|| self.tp as f64 / (self.tp + self.fp) as f64)
    }
    pub fn recall(&self) -> Option<f64> {
        (self.tp + self.fn_ > 0).then(|| self.tp as f64 / (self.tp + self.fn_) as f64)
    }
}

/// Feil uskarphetsvurdering for én fil (ved skanningens terskel).
#[derive(Clone, Debug, PartialEq)]
pub struct BlurMiss {
    pub file: String,
    pub score: f32,
    /// `true` = flagget som uskarp men etikettert skarp; `false` = ikke flagget men etikettert uskarp.
    pub flagged: bool,
}

/// Alle tall fra en kjøring.
#[derive(Clone, Debug, Default)]
pub struct Report {
    pub files_in_dir: usize,
    pub labeled: usize,
    pub unlabeled: usize,
    pub undecodable: BTreeMap<String, usize>,
    pub corrupt: usize,
    pub low_info: usize,
    pub exact: Option<PairStats>,
    pub visual_all: Option<PairStats>,
    pub visual_reps: Option<PairStats>,
    pub false_pairs: Vec<FalsePair>,
    pub false_pair_total: usize,
    pub blur_app: Option<BlurStats>,
    pub blur_abs: Option<BlurStats>,
    pub blur_not_assessable: usize,
    pub blur_misses: Vec<BlurMiss>,
}

struct Dec {
    dhash: Hash64,
    phash: Hash64,
    low: bool,
    blur: f32,
}

struct Item {
    file: String,
    hash: [u8; 32],
    dec: Option<Dec>,
}

/// Kjører `f(0..n)` på flere tråder; resultatene kommer i indeksrekkefølge.
fn par_map<T: Send>(n: usize, f: impl Fn(usize) -> T + Sync) -> Vec<T> {
    let next = AtomicUsize::new(0);
    let out: Mutex<Vec<Option<T>>> = Mutex::new((0..n).map(|_| None).collect());
    let workers = std::thread::available_parallelism().map_or(2, |w| w.get());
    std::thread::scope(|s| {
        for _ in 0..workers.min(n.max(1)) {
            s.spawn(|| loop {
                let i = next.fetch_add(1, Ordering::Relaxed);
                if i >= n {
                    break;
                }
                let v = f(i);
                if let Ok(mut g) = out.lock() {
                    g[i] = Some(v);
                }
            });
        }
    });
    out.into_inner()
        .unwrap_or_default()
        .into_iter()
        .flatten()
        .collect()
}

fn c2(n: u64) -> u64 {
    n * n.saturating_sub(1) / 2
}

/// Paranalyse. `groups` er funnet gruppering (indekser i `truth`), `truth[i]` er fasitgruppen
/// (None = ingen kjente duplikater). Returnerer tall og falske par (indeksparene).
fn pair_stats(groups: &[Vec<usize>], truth: &[Option<&str>]) -> (PairStats, Vec<(usize, usize)>) {
    let mut by_truth: BTreeMap<&str, u64> = BTreeMap::new();
    for t in truth.iter().flatten() {
        *by_truth.entry(t).or_default() += 1;
    }
    let mut s = PairStats {
        truth_pairs: by_truth.values().map(|n| c2(*n)).sum(),
        ..PairStats::default()
    };
    let mut wrong = Vec::new();
    for g in groups {
        s.found_pairs += c2(g.len() as u64);
        for (k, &a) in g.iter().enumerate() {
            for &b in &g[k + 1..] {
                match (truth[a], truth[b]) {
                    (Some(x), Some(y)) if x == y => s.true_pairs += 1,
                    _ => wrong.push((a, b)),
                }
            }
        }
    }
    (s, wrong)
}

fn visual_eval(
    idx: &[usize],
    items: &[Item],
    truth: &[Option<String>],
    p: &Params,
) -> (PairStats, Vec<FalsePair>) {
    let hs: Vec<(AssetId, Hash64, Hash64)> = idx
        .iter()
        .enumerate()
        .filter_map(|(k, &i)| {
            let d = items[i].dec.as_ref()?;
            Some((AssetId(k as u64 + 1), d.dhash, d.phash))
        })
        .collect();
    let groups: Vec<Vec<usize>> = visual_groups(&hs, p)
        .into_iter()
        .map(|g| g.members.iter().map(|m| m.0 as usize - 1).collect())
        .collect();
    let tr: Vec<Option<&str>> = idx.iter().map(|&i| truth[i].as_deref()).collect();
    let (stats, wrong) = pair_stats(&groups, &tr);
    let size_of: BTreeMap<usize, usize> = groups
        .iter()
        .flat_map(|g| g.iter().map(|&m| (m, g.len())))
        .collect();
    let fp = wrong
        .into_iter()
        .filter_map(|(a, b)| {
            let (da, db) = (items[idx[a]].dec.as_ref()?, items[idx[b]].dec.as_ref()?);
            Some(FalsePair {
                a: items[idx[a]].file.clone(),
                b: items[idx[b]].file.clone(),
                dist: da.dhash.hamming(db.dhash).max(da.phash.hamming(db.phash)),
                truth_a: tr[a].unwrap_or("-").to_string(),
                truth_b: tr[b].unwrap_or("-").to_string(),
                group_size: size_of[&a],
            })
        })
        .collect();
    (stats, fp)
}

fn blur_eval(
    rows: &[(usize, bool, f32)],
    threshold: f32,
    items: &[Item],
    misses: Option<&mut Vec<BlurMiss>>,
) -> BlurStats {
    let mut s = BlurStats {
        threshold,
        ..BlurStats::default()
    };
    let mut m = Vec::new();
    for &(i, blurry, score) in rows {
        let flagged = score < threshold;
        match (blurry, flagged) {
            (true, true) => s.tp += 1,
            (false, true) => s.fp += 1,
            (true, false) => s.fn_ += 1,
            (false, false) => s.tn += 1,
        }
        if blurry != flagged {
            m.push(BlurMiss {
                file: items[i].file.clone(),
                score,
                flagged,
            });
        }
    }
    if let Some(out) = misses {
        out.extend(m);
    }
    s
}

fn classify(e: &SourceError) -> &'static str {
    match e {
        SourceError::Unavailable => "unavailable",
        SourceError::Corrupt => "corrupt",
        _ => "failed",
    }
}

/// Selve målingen, uten utskrift. `labels` leses mot filene i `src`.
pub fn evaluate(
    src: &dyn AssetSource,
    labels: &Labels,
    opt: &Options,
) -> Result<Report, FlowError> {
    let metas = src.list_changes(None).map_err(fail)?.changed;
    let ids: Vec<SourceId> = metas.iter().map(|m| m.source_id.clone()).collect();
    let processed = par_map(ids.len(), |i| {
        let hash = src
            .read_bytes(&ids[i])
            .ok()
            .and_then(|mut r| content_hash(&mut r).ok());
        let dec = match src.thumbnail(&ids[i]) {
            Ok(t) => {
                let (dhash, phash) = perceptual(&t);
                Ok(Dec {
                    dhash,
                    phash,
                    low: low_info(&t),
                    blur: blur_score(&t),
                })
            }
            Err(e) => Err(classify(&e)),
        };
        (hash, dec)
    });
    let mut items = Vec::new();
    let mut r = Report {
        files_in_dir: metas.len(),
        ..Report::default()
    };
    let mut all_scores = Vec::new();
    for (id, (hash, dec)) in ids.iter().zip(processed) {
        let Some(hash) = hash else {
            return Err(fail(format!("kunne ikke lese {}", id.0)));
        };
        let dec = match dec {
            Ok(d) => {
                all_scores.push(d.blur);
                Some(d)
            }
            Err(kind) => {
                if kind == "corrupt" {
                    r.corrupt += 1;
                } else {
                    let ext =
                        id.0.rsplit_once('.')
                            .map_or(String::new(), |(_, e)| format!(".{}", e.to_lowercase()));
                    *r.undecodable.entry(ext).or_default() += 1;
                }
                None
            }
        };
        items.push(Item {
            file: id.0.clone(),
            hash,
            dec,
        });
    }
    // Etikett per fil (None = ikke i etikettfilen).
    let by_name: BTreeMap<&str, usize> = items
        .iter()
        .enumerate()
        .map(|(i, it)| (it.file.as_str(), i))
        .collect();
    let lower: BTreeMap<String, Vec<usize>> =
        items
            .iter()
            .enumerate()
            .fold(BTreeMap::new(), |mut m, (i, it)| {
                m.entry(it.file.to_lowercase()).or_default().push(i);
                m
            });
    let mut lab: Vec<Option<&Label>> = vec![None; items.len()];
    let mut missing = Vec::new();
    for l in &labels.rows {
        let hit = by_name.get(l.file.as_str()).copied().or_else(|| {
            lower
                .get(&l.file.to_lowercase())
                .filter(|v| v.len() == 1)
                .map(|v| v[0])
        });
        match hit {
            Some(i) if lab[i].is_none() => lab[i] = Some(l),
            Some(_) => {
                return Err(usage(format!(
                    "{} står to ganger (ulik store/små bokstaver)",
                    l.file
                )))
            }
            None => missing.push(l.file.as_str()),
        }
    }
    if !missing.is_empty() {
        let shown: Vec<_> = missing.iter().take(10).copied().collect();
        return Err(fail(format!(
            "{} filer i etikettfilen finnes ikke i mappen (eller har format som ikke leses): {}{}",
            missing.len(),
            shown.join(", "),
            if missing.len() > 10 { ", ..." } else { "" }
        )));
    }
    r.labeled = lab.iter().flatten().count();
    r.unlabeled = items.len() - r.labeled;
    let lab_idx: Vec<usize> = (0..items.len()).filter(|&i| lab[i].is_some()).collect();

    // Eksakte duplikater.
    if labels.has_exact {
        let h: Vec<(AssetId, [u8; 32])> = lab_idx
            .iter()
            .enumerate()
            .map(|(k, &i)| (AssetId(k as u64 + 1), items[i].hash))
            .collect();
        let groups: Vec<Vec<usize>> = exact_groups(&h)
            .into_iter()
            .map(|g| g.members.iter().map(|m| m.0 as usize - 1).collect())
            .collect();
        let tr: Vec<Option<&str>> = lab_idx
            .iter()
            .map(|&i| lab[i].and_then(|l| l.exact.as_deref()))
            .collect();
        r.exact = Some(pair_stats(&groups, &tr).0);
    }

    // Visuell gruppering.
    if labels.has_visual {
        let truth: Vec<Option<String>> = (0..items.len())
            .map(|i| lab[i].and_then(|l| l.visual.clone()))
            .collect();
        let usable = |i: &usize| items[*i].dec.as_ref().is_some_and(|d| !d.low);
        r.low_info = lab_idx
            .iter()
            .filter(|&&i| items[i].dec.as_ref().is_some_and(|d| d.low))
            .count();
        let all: Vec<usize> = lab_idx.iter().copied().filter(usable).collect();
        let mut seen = BTreeSet::new();
        let reps: Vec<usize> = all
            .iter()
            .copied()
            .filter(|&i| seen.insert(items[i].hash))
            .collect();
        let (s_all, _) = visual_eval(&all, &items, &truth, &opt.params);
        let (s_reps, mut fp) = visual_eval(&reps, &items, &truth, &opt.params);
        r.visual_all = Some(s_all);
        r.visual_reps = Some(s_reps);
        fp.sort_by(|x, y| (x.dist, &x.a, &x.b).cmp(&(y.dist, &y.a, &y.b)));
        r.false_pair_total = fp.len();
        fp.truncate(opt.rows);
        r.false_pairs = fp;
    }

    // Uskarphet.
    if labels.has_blurry {
        let mut rows = Vec::new();
        for &i in &lab_idx {
            let (Some(b), Some(d)) = (lab[i].and_then(|l| l.blurry), items[i].dec.as_ref()) else {
                continue;
            };
            if d.blur < LOW_CONTRAST {
                rows.push((i, b, d.blur));
            } else {
                r.blur_not_assessable += 1;
            }
        }
        let thr = blur_threshold(&all_scores);
        let mut misses = Vec::new();
        r.blur_app = Some(blur_eval(&rows, thr, &items, Some(&mut misses)));
        r.blur_abs = Some(blur_eval(&rows, BLUR_ABS_MAX, &items, None));
        misses.sort_by(|x, y| (!x.flagged, &x.file).cmp(&(!y.flagged, &y.file)));
        misses.truncate(opt.rows);
        r.blur_misses = misses;
    }
    Ok(r)
}

fn pct(v: Option<f64>) -> String {
    v.map_or("-".to_string(), |v| format!("{v:.3}"))
}

fn w(out: &mut dyn Write, s: String) -> Result<(), FlowError> {
    writeln!(out, "{s}").map_err(fail)
}

fn pairs_line(out: &mut dyn Write, name: &str, s: &PairStats) -> Result<(), FlowError> {
    w(
        out,
        format!(
            "{name}: presisjon {} recall {} (fasitpar {}, funne par {}, riktige {}, falske {}, tapte {})",
            pct(s.precision()),
            pct(s.recall()),
            s.truth_pairs,
            s.found_pairs,
            s.true_pairs,
            s.found_pairs - s.true_pairs,
            s.truth_pairs - s.true_pairs
        ),
    )
}

fn blur_line(out: &mut dyn Write, name: &str, s: &BlurStats) -> Result<(), FlowError> {
    w(
        out,
        format!(
            "{name} (terskel {:.5}): presisjon {} recall {} (tp {}, fp {}, fn {}, tn {})",
            s.threshold,
            pct(s.precision()),
            pct(s.recall()),
            s.tp,
            s.fp,
            s.fn_,
            s.tn
        ),
    )
}

/// Skriver rapporten som ren tekst (tabeller med tabulator).
pub fn write_report(r: &Report, p: &Params, out: &mut dyn Write) -> Result<(), FlowError> {
    w(out, format!("ALGO_VER {}", ALGO_VER.0))?;
    w(
        out,
        format!(
            "parametre: max_dist={} max_span={} tight_dist={} max_group={}",
            p.max_dist, p.max_span, p.tight_dist, p.max_group
        ),
    )?;
    w(
        out,
        format!(
            "filer: {} i mappen, {} etikettert, {} uten etikett (utelatt)",
            r.files_in_dir, r.labeled, r.unlabeled
        ),
    )?;
    if !r.undecodable.is_empty() {
        let v: Vec<String> = r
            .undecodable
            .iter()
            .map(|(e, n)| format!("{e} {n}"))
            .collect();
        w(
            out,
            format!(
                "ikke dekodbare (bare JPEG og PNG dekodes; utelatt fra visuell og uskarp): {}",
                v.join(", ")
            ),
        )?;
    }
    if r.corrupt > 0 {
        w(out, format!("ødelagte (utelatt): {}", r.corrupt))?;
    }
    if let Some(s) = &r.exact {
        w(out, "[eksakte duplikater]".into())?;
        pairs_line(out, "eksakt", s)?;
    }
    if let (Some(a), Some(b)) = (&r.visual_all, &r.visual_reps) {
        w(out, "[visuell hash-gruppering]".into())?;
        w(out, format!("lav informasjon (utelatt): {}", r.low_info))?;
        pairs_line(out, "alle filer", a)?;
        pairs_line(out, "én per identisk innhold", b)?;
        w(
            out,
            format!(
                "[falske sammenslåinger] {} par (viser {})",
                r.false_pair_total,
                r.false_pairs.len()
            ),
        )?;
        w(
            out,
            "fil_a\tfil_b\tmaks_hamming\tfasit_a\tfasit_b\tgruppestørrelse".into(),
        )?;
        for f in &r.false_pairs {
            w(
                out,
                format!(
                    "{}\t{}\t{}\t{}\t{}\t{}",
                    f.a, f.b, f.dist, f.truth_a, f.truth_b, f.group_size
                ),
            )?;
        }
    }
    if let (Some(a), Some(b)) = (&r.blur_app, &r.blur_abs) {
        w(out, "[uskarphet]".into())?;
        w(
            out,
            format!("ikke vurderbare (utelatt): {}", r.blur_not_assessable),
        )?;
        blur_line(out, "skanningens terskel", a)?;
        blur_line(out, "fast BLUR_ABS_MAX", b)?;
        w(out, "fil\tscore\tfeil".into())?;
        for m in &r.blur_misses {
            let kind = if m.flagged {
                "flagget_men_skarp"
            } else {
                "ikke_flagget_men_uskarp"
            };
            w(out, format!("{}\t{:.5}\t{kind}", m.file, m.score))?;
        }
    }
    Ok(())
}

/// Leser `labels_csv`, måler mot `src` og skriver rapporten.
pub fn run(
    src: &dyn AssetSource,
    labels_csv: &Path,
    opt: &Options,
    out: &mut dyn Write,
) -> Result<Report, FlowError> {
    let text = std::fs::read(labels_csv)
        .map_err(|e| fail(format!("kan ikke lese {}: {e}", labels_csv.display())))?;
    let labels = parse_labels(&String::from_utf8_lossy(&text))?;
    let r = evaluate(src, &labels, opt)?;
    write_report(&r, &opt.params, out)?;
    Ok(r)
}
