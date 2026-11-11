//! Golden-port K5-6: ved en original og dens nedskalerte kopi beholdes alltid den store.
//! Blur-score regnes fra generatorens miniatyr; metadata (størrelse, bytes, tid) fra manifestet.

use fjell_core::model::{AssetId, ProtectFlags, Thumb, ThumbFmt};
use fjell_core::quality::blur_score;
use fjell_core::suggest::{group_suggestions, rank_group, GroupKind, Member};
use fjell_testdata::corpus::{generate, DEFAULT_SEED};
use fjell_testdata::manifest::Entry;
use std::{fs, path::PathBuf};

fn member(dir: &std::path::Path, id: u64, e: &Entry) -> Member {
    let img = image::open(dir.join(&e.thumb)).unwrap().to_luma8();
    let t = Thumb::new(img.width(), img.height(), ThumbFmt::Gray8, img.into_raw()).unwrap();
    Member {
        id: AssetId(id),
        width: e.width,
        height: e.height,
        bytes: e.bytes,
        blur_score: blur_score(&t),
        created_at: e.created_at,
        protect_flags: ProtectFlags(0),
    }
}

#[test]
fn k5_6_original_kept_over_downscaled() {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("golden_suggest");
    let _ = fs::remove_dir_all(&dir);
    let m = generate(&dir, DEFAULT_SEED).unwrap();
    let originals: Vec<(u64, &Entry)> = m
        .entries
        .iter()
        .enumerate()
        .filter(|(_, e)| e.group == "original")
        .map(|(i, e)| (i as u64 + 1, e))
        .collect();
    let (mut pairs, mut kept) = (0, 0);
    for (i, e) in m.entries.iter().enumerate().filter(|(_, e)| e.group == "downscaled") {
        let (oid, o) = originals
            .iter()
            .find(|(_, o)| o.visual_group == e.visual_group)
            .expect("original finnes");
        let (a, b) = (member(&dir, *oid, o), member(&dir, i as u64 + 1, e));
        // Begge rekkefølger gir samme keeper.
        for g in [[a, b], [b, a]] {
            pairs += 1;
            let r = rank_group(&g).unwrap();
            let res = group_suggestions(GroupKind::Visual { max_hamming: 4 }, &g).unwrap();
            assert_eq!(r.keeper, res.keeper);
            if r.keeper == a.id {
                kept += 1;
            }
        }
    }
    eprintln!("K5-6: store beholdt i {kept}/{pairs} nedskalerte par");
    assert!(pairs > 0);
    assert_eq!(kept, pairs);
}
