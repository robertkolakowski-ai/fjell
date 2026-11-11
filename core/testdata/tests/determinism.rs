//! To kjøringer med samme seed gir byte-identisk manifest (og dermed identiske filer,
//! siden manifestet har innholdshash). Sjekker også størrelsene og fasit-invariantene.

use fjell_testdata::corpus::{generate, DEFAULT_SEED};
use std::{fs, path::PathBuf};

fn tmp(name: &str) -> PathBuf {
    let p = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = fs::remove_dir_all(&p);
    p
}

#[test]
fn two_runs_give_identical_manifest() {
    let (a, b) = (tmp("golden_a"), tmp("golden_b"));
    generate(&a, DEFAULT_SEED).unwrap();
    generate(&b, DEFAULT_SEED).unwrap();
    let (ma, mb) = (
        fs::read(a.join("manifest.json")).unwrap(),
        fs::read(b.join("manifest.json")).unwrap(),
    );
    assert!(ma == mb, "manifest er ikke deterministisk");
    let other = tmp("golden_c");
    generate(&other, DEFAULT_SEED ^ 1).unwrap();
    assert!(
        ma != fs::read(other.join("manifest.json")).unwrap(),
        "annen seed må gi annet korpus"
    );
}

#[test]
fn corpus_matches_plan() {
    let m = generate(&tmp("golden_plan"), DEFAULT_SEED).unwrap();
    let n = |g: &str| m.entries.iter().filter(|e| e.group == g).count();
    for (g, c) in [
        ("original", 60),
        ("exact", 20),
        ("downscaled", 30),
        ("recoded", 30),
        ("cropped", 20),
        ("blurry", 40),
        ("sharp", 40),
        ("screenshot", 30),
        ("screenshot_neg", 20),
        ("negpair", 30),
        ("degenerate", 10),
    ] {
        assert_eq!(n(g), c, "gruppe {g}");
    }
    // Eksakte kopier er bytevis like originalen og har ulik fil og tid.
    for e in m.entries.iter().filter(|e| e.group == "exact") {
        let o = m
            .entries
            .iter()
            .find(|o| o.group == "original" && o.exact_group == e.exact_group)
            .unwrap();
        assert_eq!((&o.fnv64, o.bytes), (&e.fnv64, e.bytes));
        assert!(o.file != e.file && o.created_at != e.created_at);
    }
    // Uskarphetsfasit: sigma >= 1 % er uskarp, 0,5 % og skarpe kontroller er det ikke.
    assert!(m
        .entries
        .iter()
        .filter(|e| e.group == "sharp")
        .all(|e| e.blurry == Some(false)));
    assert!(m
        .entries
        .iter()
        .filter(|e| e.group == "negpair" || e.group == "degenerate")
        .all(|e| e.visual_group.is_none() && e.exact_group.is_none()));
    assert!(m
        .entries
        .iter()
        .filter(|e| e.group == "screenshot")
        .all(|e| e.screenshot == Some(true)));
    assert!(m.entries.iter().all(|e| e.thumb.starts_with("thumbs/")));
}
