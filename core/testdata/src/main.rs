//! `fjell-gen-golden [ut-mappe] [seed]`: genererer golden-korpuset.
//! Standard ut-mappe: `generated/` ved siden av denne crate-en.

use std::path::PathBuf;

fn main() {
    let mut a = std::env::args().skip(1);
    let out = a
        .next()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/generated")));
    let seed = a
        .next()
        .map(|s| s.parse().expect("seed må være heltall"))
        .unwrap_or(fjell_testdata::corpus::DEFAULT_SEED);
    let m = fjell_testdata::corpus::generate(&out, seed).expect("generering feilet");
    println!("{} bilder skrevet til {}", m.entries.len(), out.display());
    if std::env::var("FJELL_BLURSET").is_ok() {
        let bo = out.join("blurset");
        let b = fjell_testdata::blurset::generate_blur(&bo, fjell_testdata::blurset::BLUR_SEED)
            .expect("blurset feilet");
        println!("{} uskarphetsbilder skrevet til {}", b.entries.len(), bo.display());
    }
}
