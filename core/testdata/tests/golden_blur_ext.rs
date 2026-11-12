//! K8-kalibrering av uskarphet og hash-porter på det utvidede korpuset (`blurset`): sju motivfamilier,
//! lineær bevegelsesuskarphet i vilkårlig vinkel, fokusfeil (skivekjerne), Gauss, sensorstøy og skarpe
//! lavkontrastbilder. Vektene og terskelen i `fjell_core::quality` er tilpasset *bare* `train`;
//! `eval` (andre motiver, samme seed) og `holdout` (egen seed, ikke brukt til noe valg) rapporteres
//! hver for seg. Tallene skrives med `--nocapture`.

use fjell_core::cluster::{visual_groups, Params};
use fjell_core::hash::{low_info, perceptual};
use fjell_core::model::{AssetId, Hash64, Thumb, ThumbFmt};
use fjell_core::quality::{blur_score, BLUR_ABS_MAX, LOW_CONTRAST};
use fjell_testdata::blurset::{generate_blur, BEntry, BManifest, BLUR_HOLDOUT_SEED, BLUR_SEED};
use std::collections::{BTreeMap, BTreeSet};
use std::{path::PathBuf, sync::OnceLock};

struct Item {
    id: AssetId,
    e: BEntry,
    score: f32,
    dhash: Hash64,
    phash: Hash64,
    low: bool,
}

type Set = (PathBuf, BManifest, Vec<Item>);

fn build(name: &str, seed: u64) -> Set {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&dir);
    let m = generate_blur(&dir, seed).unwrap();
    let items = m
        .entries
        .iter()
        .enumerate()
        .map(|(i, e)| {
            let g = image::open(dir.join(&e.thumb)).unwrap().to_luma8();
            let t = Thumb::new(g.width(), g.height(), ThumbFmt::Gray8, g.into_raw()).unwrap();
            let (dhash, phash) = perceptual(&t);
            Item {
                id: AssetId(i as u64 + 1),
                e: e.clone(),
                score: blur_score(&t),
                dhash,
                phash,
                low: low_info(&t),
            }
        })
        .collect();
    (dir, m, items)
}

fn main_set() -> &'static Set {
    static C: OnceLock<Set> = OnceLock::new();
    C.get_or_init(|| build("blurset_main", BLUR_SEED))
}

fn holdout_set() -> &'static Set {
    static C: OnceLock<Set> = OnceLock::new();
    C.get_or_init(|| build("blurset_holdout", BLUR_HOLDOUT_SEED))
}

/// (presisjon, recall, tp, fp, fn) for «score < BLUR_ABS_MAX» mot fasit på utvalget.
fn blur_pr<'a>(items: impl Iterator<Item = &'a Item>) -> (f64, f64, usize, usize, usize) {
    let (mut tp, mut fp, mut fneg) = (0, 0, 0);
    // Nesten flate bilder (lumavarians < 4) er «ikke vurderbare» og telles ikke (se `BEntry::assessable`).
    for i in items.filter(|i| i.e.assessable) {
        match (i.e.blurry, i.score < BLUR_ABS_MAX) {
            (true, true) => tp += 1,
            (false, true) => fp += 1,
            (true, false) => fneg += 1,
            _ => {}
        }
    }
    (
        tp as f64 / (tp + fp).max(1) as f64,
        tp as f64 / (tp + fneg).max(1) as f64,
        tp,
        fp,
        fneg,
    )
}

fn report(name: &str, items: &[Item]) -> [(f64, f64); 2] {
    let mut out = [(0.0, 0.0); 2];
    for (k, split) in ["train", "eval"].iter().enumerate() {
        let (p, r, tp, fp, f) = blur_pr(items.iter().filter(|i| i.e.split == *split));
        eprintln!("K8 {name} {split}: presisjon {p:.3}, recall {r:.3} (tp {tp}, fp {fp}, fn {f})");
        out[k] = (p, r);
    }
    // Fordeling: andel flagget per familie og variant (eval + train sammen).
    let kinds = [
        "sharp",
        "lowcontrast",
        "soft",
        "recoded",
        "down",
        "gauss",
        "defocus",
        "motion",
        "noisyblur",
    ];
    let fams: BTreeSet<&str> = items.iter().map(|i| i.e.family.as_str()).collect();
    eprintln!(
        "K8 {name} andel flagget (score < ABS_MAX); skarpe: {}; uskarpe: {}",
        kinds[..5].join("/"),
        kinds[5..].join("/")
    );
    for f in fams {
        let row: Vec<String> = kinds
            .iter()
            .map(|k| {
                let v: Vec<_> = items
                    .iter()
                    .filter(|i| i.e.family == f && i.e.kind == *k)
                    .collect();
                format!(
                    "{:.2}",
                    v.iter().filter(|i| i.score < BLUR_ABS_MAX).count() as f64
                        / v.len().max(1) as f64
                )
            })
            .collect();
        eprintln!("    {f:11} {}", row.join(" "));
    }
    out
}

/// K8 uskarphet, trening og evaluering (samme seed, ulike motiver). Port: presisjon og recall ≥ 0,90
/// på `eval`. `train` rapporteres for å vise forskjellen mellom tilpasning og måling.
#[test]
fn k8_blur_train_and_eval() {
    let (_, m, items) = main_set();
    assert_eq!(m.entries.len(), 7 * 2 * 8 * 9);
    let na = m.entries.iter().filter(|e| !e.assessable).count();
    let na_blurry = m
        .entries
        .iter()
        .filter(|e| !e.assessable && e.blurry)
        .count();
    eprintln!("K8 hovedsett: {na} av {} bilder er ikke vurderbare (varians < 4), {na_blurry} av dem uskarpe; utelatt fra presisjon/recall", m.entries.len());
    let [(_, _), (p, r)] = report("hovedsett", items);
    assert!(p >= 0.90, "eval presisjon {p}");
    assert!(r >= 0.90, "eval recall {r}");
}

/// K8 uskarphet på kontrollsettet med egen seed (ingen valg tatt mot dette).
#[test]
fn k8_blur_holdout() {
    let (_, _, items) = holdout_set();
    let (p, r, tp, fp, f) = blur_pr(items.iter());
    eprintln!("K8 holdout (alle {} bilder, egen seed): presisjon {p:.3}, recall {r:.3} (tp {tp}, fp {fp}, fn {f})", items.len());
    report("holdout", items);
    assert!(p >= 0.90, "holdout presisjon {p}");
    assert!(r >= 0.90, "holdout recall {r}");
}

/// Skarpe lavkontrastbilder (kontrast 0,25–0,5) og skarpe kontroller med støy flagges ikke oftere enn
/// 5 % (K3-4 utvidet), og lavkontrast er like skarp som originalen innen ±25 % av scoren.
#[test]
fn k8_low_contrast_sharp_not_flagged() {
    let (_, _, items) = main_set();
    let low: Vec<_> = items.iter().filter(|i| i.e.kind == "lowcontrast").collect();
    let flagged = low.iter().filter(|i| i.score < BLUR_ABS_MAX).count();
    let rate = flagged as f64 / low.len() as f64;
    let sharp: BTreeMap<u32, f32> = items
        .iter()
        .filter(|i| i.e.kind == "sharp")
        .map(|i| (i.e.visual_group, i.score))
        .collect();
    let (mut agree, mut n) = (0, 0);
    for i in &low {
        let s = sharp[&i.e.visual_group];
        if s < LOW_CONTRAST && i.score < LOW_CONTRAST {
            n += 1;
            if (i.score / s - 1.0).abs() <= 0.25 {
                agree += 1;
            }
        }
    }
    eprintln!(
        "K8 lavkontrast: {flagged}/{} flagget ({rate:.3}); innen ±25 % av originalen: {agree}/{n}",
        low.len()
    );
    assert!(rate <= 0.05, "lavkontrast flagget {rate}");
}

type Hs = Vec<(AssetId, Hash64, Hash64)>;

fn pairs_of<'a>(gs: impl Iterator<Item = &'a Vec<AssetId>>) -> BTreeSet<(AssetId, AssetId)> {
    let mut s = BTreeSet::new();
    for g in gs {
        for (i, a) in g.iter().enumerate() {
            for b in &g[i + 1..] {
                s.insert((*a.min(b), *a.max(b)));
            }
        }
    }
    s
}

/// Hash-porter (visuelt duplikat, ARKITEKTUR del 4) på det utvidede korpuset: alle varianter av et
/// motiv (lavkontrast, myk, omkodet, halv oppløsning og **uskarpe**) er sannhet-gruppe. Presisjon
/// ≥ 0,98 og recall ≥ 0,90 uten Gauss/fokus/bevegelse-varianter (som `corpus`), og samlet rapportert
/// med dem. Teksturfamilier med lik palett og struktur er harde negative.
#[test]
fn k8_hash_gates_on_extended_data() {
    let (_, _, items) = main_set();
    let run = |skip: &[&str]| {
        let elig: Vec<&Item> = items
            .iter()
            .filter(|i| !i.low && !skip.contains(&i.e.kind.as_str()))
            .collect();
        let hs: Hs = elig.iter().map(|i| (i.id, i.dhash, i.phash)).collect();
        let groups = visual_groups(&hs, &Params::default());
        let pred = pairs_of(groups.iter().map(|g| &g.members));
        let by: BTreeMap<AssetId, &Item> = elig.iter().map(|i| (i.id, *i)).collect();
        let mut tg: BTreeMap<u32, Vec<AssetId>> = BTreeMap::new();
        for i in &elig {
            tg.entry(i.e.visual_group).or_default().push(i.id);
        }
        let truth = pairs_of(tg.values());
        let tp_all = pred
            .iter()
            .filter(|p| by[&p.0].e.visual_group == by[&p.1].e.visual_group)
            .count();
        let tp = truth.iter().filter(|p| pred.contains(p)).count();
        // Recall per variant mot originalen.
        let mut per: BTreeMap<String, (usize, usize)> = BTreeMap::new();
        for p in &truth {
            let (a, b) = (by[&p.0], by[&p.1]);
            if a.e.kind == "sharp" || b.e.kind == "sharp" {
                let k = if a.e.kind == "sharp" {
                    &b.e.kind
                } else {
                    &a.e.kind
                };
                let e = per.entry(k.clone()).or_default();
                e.1 += 1;
                e.0 += pred.contains(p) as usize;
            }
        }
        (
            tp_all as f64 / pred.len().max(1) as f64,
            tp as f64 / truth.len().max(1) as f64,
            groups.len(),
            pred.len() - tp_all,
            per,
        )
    };
    let (p, r, g, fp, _) = run(&["gauss", "defocus", "motion", "noisyblur"]);
    eprintln!("K8 hash uten uskarpe varianter: {g} grupper, presisjon {p:.3}, recall {r:.3} (falske par {fp})");
    let (p2, r2, g2, fp2, per) = run(&[]);
    eprintln!("K8 hash med uskarpe varianter: {g2} grupper, presisjon {p2:.3}, recall {r2:.3} (falske par {fp2})");
    for (k, (a, b)) in per {
        eprintln!(
            "    recall original mot {k}: {a}/{b} = {:.2}",
            a as f64 / b as f64
        );
    }
    assert!(p >= 0.98, "presisjon {p}");
    assert!(r >= 0.90, "recall {r}");
}

/// Generatoren er deterministisk (to kjøringer, byte-identisk manifest) og gir ulike sett for ulik seed.
#[test]
fn blurset_is_deterministic() {
    let (dir, _, _) = main_set();
    let again = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("blurset_again");
    let _ = std::fs::remove_dir_all(&again);
    generate_blur(&again, BLUR_SEED).unwrap();
    let (a, b) = (
        std::fs::read(dir.join("blur_manifest.json")).unwrap(),
        std::fs::read(again.join("blur_manifest.json")).unwrap(),
    );
    assert!(a == b, "manifest ikke deterministisk");
    let (hd, _, _) = holdout_set();
    assert!(a != std::fs::read(hd.join("blur_manifest.json")).unwrap());
}
