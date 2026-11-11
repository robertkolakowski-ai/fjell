//! Golden-porter for K1 (eksakt hash) og K2 (dHash, pHash, visuell klynging) mot korpuset.
//! Dekker K1-3, K2a-2, K2a-4, K2b-3, K2b-4, K2b-7 og K2b-1 på korpus-hasher. Tallene skrives ut
//! (`cargo test -- --nocapture`). Miniatyrene er generatorens referansenedskalering (Gray8-PNG).

use fjell_core::cluster::{exact_groups, similar_pairs, visual_groups, Params};
use fjell_core::hash::{content_hash, low_info, perceptual};
use fjell_core::model::{AssetId, Hash64, Thumb, ThumbFmt};
use fjell_testdata::corpus::{generate, DEFAULT_SEED};
use fjell_testdata::manifest::{Entry, Manifest};
use std::collections::{BTreeMap, BTreeSet};
use std::{fs, path::PathBuf, sync::OnceLock};

struct Item {
    id: AssetId,
    e: Entry,
    content: [u8; 32],
    dhash: Hash64,
    phash: Hash64,
    low: bool,
}

fn corpus() -> &'static (PathBuf, Manifest, Vec<Item>) {
    static C: OnceLock<(PathBuf, Manifest, Vec<Item>)> = OnceLock::new();
    C.get_or_init(|| {
        let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("golden_algos");
        let _ = fs::remove_dir_all(&dir);
        let m = generate(&dir, DEFAULT_SEED).unwrap();
        let items = m
            .entries
            .iter()
            .enumerate()
            .map(|(i, e)| {
                let mut f = fs::File::open(dir.join(&e.file)).unwrap();
                let content = content_hash(&mut f).unwrap();
                let img = image::open(dir.join(&e.thumb)).unwrap().to_luma8();
                let t =
                    Thumb::new(img.width(), img.height(), ThumbFmt::Gray8, img.into_raw()).unwrap();
                let (dhash, phash) = perceptual(&t);
                Item {
                    id: AssetId(i as u64 + 1),
                    e: e.clone(),
                    content,
                    dhash,
                    phash,
                    low: low_info(&t),
                }
            })
            .collect();
        (dir, m, items)
    })
}

fn pairs_of<'a>(ids: impl Iterator<Item = &'a Vec<AssetId>>) -> BTreeSet<(AssetId, AssetId)> {
    let mut s = BTreeSet::new();
    for g in ids {
        for (i, a) in g.iter().enumerate() {
            for b in &g[i + 1..] {
                s.insert((*a.min(b), *a.max(b)));
            }
        }
    }
    s
}

#[test]
fn k1_3_exact_copies_form_exact_groups() {
    let (_, _, items) = corpus();
    let hashes: Vec<_> = items.iter().map(|i| (i.id, i.content)).collect();
    let got = exact_groups(&hashes);
    let mut truth: BTreeMap<u32, Vec<AssetId>> = BTreeMap::new();
    for i in items {
        if let Some(g) = i.e.exact_group {
            truth.entry(g).or_default().push(i.id);
        }
    }
    let (p, t) = (
        pairs_of(got.iter().map(|g| &g.members)),
        pairs_of(truth.values().filter(|v| v.len() > 1)),
    );
    let tp = p.intersection(&t).count() as f64;
    let (prec, rec) = (tp / p.len() as f64, tp / t.len() as f64);
    eprintln!(
        "K1-3: {} grupper, presisjon {prec:.3}, recall {rec:.3}",
        got.len()
    );
    assert_eq!((prec, rec), (1.0, 1.0));
}

fn near(a: &Item, b: &Item) -> bool {
    a.dhash.hamming(b.dhash) <= 6 && a.phash.hamming(b.phash) <= 6
}

#[test]
fn k2a_2_resized_and_recoded_stay_close() {
    let (_, _, items) = corpus();
    let originals: BTreeMap<u32, &Item> = items
        .iter()
        .filter(|i| i.e.group == "original")
        .filter_map(|i| i.e.visual_group.map(|g| (g, i)))
        .collect();
    let derived: Vec<_> = items
        .iter()
        .filter(|i| i.e.group == "downscaled" || i.e.group == "recoded")
        .collect();
    let ok = derived
        .iter()
        .filter(|d| near(d, originals[&d.e.visual_group.unwrap()]))
        .count();
    let rate = ok as f64 / derived.len() as f64;
    eprintln!(
        "K2a-2: {ok}/{} par innen Hamming 6 på begge ({rate:.3})",
        derived.len()
    );
    assert!(rate >= 0.95);
}

#[test]
fn k2a_4_negative_pairs_are_separated() {
    let (_, _, items) = corpus();
    let by = |g: &str| items.iter().filter(|i| i.e.group == g).collect::<Vec<_>>();
    let mut neg: Vec<(&Item, &Item)> = by("negpair").chunks(2).map(|c| (c[0], c[1])).collect();
    let orig = by("original");
    for (i, a) in orig.iter().enumerate() {
        for b in &orig[i + 1..] {
            neg.push((a, b));
        }
    }
    let sep = neg.iter().filter(|(a, b)| !near(a, b)).count();
    let rate = sep as f64 / neg.len() as f64;
    eprintln!("K2a-4: {sep}/{} negative par skilt ({rate:.3})", neg.len());
    assert!(rate >= 0.98);
}

/// Én representant (lavest id) per eksakt gruppe og uten lavinformasjon, som pipeline gjør.
/// Alle korpusbilder tas med (også lavkontrast-skarpe s30–s39, som i manifestet hører til
/// visuell gruppe 100+i); kun lavinformasjon og eksakte kopier utelates, som pipeline gjør.
fn eligible<'a>(items: &'a [Item], skip: &[&str]) -> Vec<&'a Item> {
    let mut seen = BTreeSet::new();
    items
        .iter()
        .filter(|i| !skip.contains(&i.e.group.as_str()) && !i.low && seen.insert(i.content))
        .collect()
}

fn score(elig: &[&Item], only_pairs_with: Option<&str>) -> (f64, f64, usize) {
    let hs: Vec<_> = elig.iter().map(|i| (i.id, i.dhash, i.phash)).collect();
    let groups = visual_groups(&hs, &Params::default());
    let pred = pairs_of(groups.iter().map(|g| &g.members));
    let by_id: BTreeMap<AssetId, &Item> = elig.iter().map(|i| (i.id, *i)).collect();
    let mut truth_g: BTreeMap<u32, Vec<AssetId>> = BTreeMap::new();
    for i in elig {
        if let Some(g) = i.e.visual_group {
            truth_g.entry(g).or_default().push(i.id);
        }
    }
    let mut truth = pairs_of(truth_g.values());
    if let Some(g) = only_pairs_with {
        truth.retain(|(a, b)| by_id[a].e.group == g || by_id[b].e.group == g);
    }
    let in_truth = |p: &(AssetId, AssetId)| {
        by_id[&p.0].e.visual_group.is_some()
            && by_id[&p.0].e.visual_group == by_id[&p.1].e.visual_group
    };
    if std::env::var("FJELL_DEBUG").is_ok() {
        for p in pred.iter().filter(|p| {
            !(by_id[&p.0].e.visual_group.is_some()
                && by_id[&p.0].e.visual_group == by_id[&p.1].e.visual_group)
        }) {
            let (a, b) = (by_id[&p.0], by_id[&p.1]);
            eprintln!(
                "FALSK PAR {} / {} d={} p={}",
                a.e.file,
                b.e.file,
                a.dhash.hamming(b.dhash),
                a.phash.hamming(b.phash)
            );
        }
    }
    let tp_all = pred.iter().filter(|p| in_truth(p)).count() as f64;
    let tp = truth.iter().filter(|p| pred.contains(p)).count() as f64;
    (
        tp_all / pred.len().max(1) as f64,
        tp / truth.len().max(1) as f64,
        groups.len(),
    )
}

#[test]
fn k2b_3_visual_duplicates_meet_gate() {
    let (_, _, items) = corpus();
    let (prec, rec, n) = score(&eligible(items, &["cropped"]), None);
    eprintln!("K2b-3: {n} grupper, presisjon {prec:.3}, recall {rec:.3}");
    assert!(prec >= 0.98, "presisjon {prec}");
    assert!(rec >= 0.90, "recall {rec}");
}

#[test]
fn k2b_4_cropped_recall_is_reported() {
    let (_, _, items) = corpus();
    let (_, rec, _) = score(&eligible(items, &[]), Some("cropped"));
    eprintln!("K2b-4: recall for beskåret 5 % = {rec:.3} (informativ, ikke port)");
}

#[test]
fn k2b_7_degenerate_never_group() {
    let (_, _, items) = corpus();
    // Som pipeline: lavinformasjon utelates. Resten av de degenererte (smale støybilder) skal
    // heller ikke havne i en gruppe selv når de sendes inn.
    let all: Vec<&Item> = items
        .iter()
        .filter(|i| i.e.group == "degenerate" || !i.low)
        .collect();
    let hs: Vec<_> = all.iter().map(|i| (i.id, i.dhash, i.phash)).collect();
    let groups = visual_groups(&hs, &Params::default());
    let degen: BTreeSet<AssetId> = items
        .iter()
        .filter(|i| i.e.group == "degenerate")
        .map(|i| i.id)
        .collect();
    let bad: Vec<_> = groups
        .iter()
        .filter(|g| g.members.iter().any(|m| degen.contains(m)))
        .collect();
    eprintln!(
        "K2b-7: {} av 10 degenererte er lavinformasjon",
        items
            .iter()
            .filter(|i| i.e.group == "degenerate" && i.low)
            .count()
    );
    assert!(bad.is_empty(), "degenererte i gruppe: {bad:?}");
}

#[test]
fn k2b_1_mih_equals_brute_force_on_corpus() {
    let (_, _, items) = corpus();
    let hs: Vec<_> = items.iter().map(|i| (i.id, i.dhash, i.phash)).collect();
    let mut brute = Vec::new();
    for (i, a) in items.iter().enumerate() {
        for b in &items[i + 1..] {
            if near(a, b) {
                brute.push((a.id, b.id));
            }
        }
    }
    brute.sort();
    assert!(!brute.is_empty());
    assert_eq!(similar_pairs(&hs, &Params::default()), brute);
}
