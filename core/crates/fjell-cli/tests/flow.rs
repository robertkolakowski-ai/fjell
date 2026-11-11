//! M4 fullflyt mot filsystemadapteren, gjennom `Fjell` (ekte kjerne): skann → grupper → forslag →
//! trash → angre → purge. K7-10: originalene er uendret til plattformen selv sletter (her en testdobbel
//! som sletter i en temp-mappe, aldri i brukerdata). Hurtigskanning: `FsSource` dekoder ikke bilder.

use fjell_cli::fsource::FsSource;
use fjell_core::api::{Decision, Fjell, FjellApi, ScanConfig, ScanMode};
use fjell_core::model::{
    AssetId, DeleteOutcome, FakeClock, PlatformDelete, SourceId, SuggestionKind,
};
use fjell_core::store::Store;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

fn tempdir(tag: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!("fjell-flow-{tag}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&p);
    fs::create_dir_all(&p).unwrap();
    p
}

fn sums(root: &Path) -> BTreeMap<String, String> {
    fn walk(root: &Path, dir: &Path, out: &mut BTreeMap<String, String>) {
        for e in fs::read_dir(dir).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() {
                walk(root, &p, out);
            } else {
                let rel = p.strip_prefix(root).unwrap().to_string_lossy().into_owned();
                out.insert(
                    rel,
                    blake3::hash(&fs::read(&p).unwrap()).to_hex().to_string(),
                );
            }
        }
    }
    let mut m = BTreeMap::new();
    walk(root, root, &mut m);
    m
}

/// Plattformsletting i temp-mappen (stand-in for `--fs-temp`).
struct TempDelete(PathBuf);
impl PlatformDelete for TempDelete {
    fn delete(
        &self,
        _t: &fjell_core::model::PurgeTicket,
        ids: &[(AssetId, SourceId)],
    ) -> Vec<(AssetId, DeleteOutcome)> {
        ids.iter()
            .map(|(a, s)| {
                let ok = fs::remove_file(self.0.join(&s.0)).is_ok();
                (
                    *a,
                    if ok {
                        DeleteOutcome::Deleted
                    } else {
                        DeleteOutcome::Failed
                    },
                )
            })
            .collect()
    }
}

#[test]
fn scan_trash_undo_purge_through_api_leaves_originals_intact() {
    let root = tempdir("a");
    fs::create_dir_all(root.join("sub")).unwrap();
    fs::write(root.join("a.jpg"), b"same-bytes").unwrap();
    fs::write(root.join("sub/b.jpg"), b"same-bytes").unwrap();
    fs::write(root.join("c.png"), b"other-content!").unwrap();
    let before = sums(&root);

    let f = Fjell::new(
        Store::open_in_memory().unwrap(),
        Arc::new(FsSource::new(&root)),
        Arc::new(FakeClock::new(1_000)),
    );
    // Full på fs: ingen miniatyrer (Unavailable) betyr hoppet over, ikke feil; eksakte grupper finnes likevel.
    f.start_scan(ScanConfig {
        mode: ScanMode::Full,
        priority: 1,
        deadline: None,
    })
    .unwrap();
    let r = f.wait_scan().unwrap().unwrap();
    assert!(r.failures.is_empty(), "{r:?}");
    assert_eq!(
        (r.new, r.hashed, r.analyzed, r.skipped_unavailable),
        (3, 2, 0, 3)
    );
    let cards = f.cards(SuggestionKind::ExactDup, 10, None).unwrap().cards;
    assert_eq!(cards.len(), 1);
    let dup = cards[0].clone();
    assert_eq!(sums(&root), before, "skann endrer ikke originaler");

    let batch = f.decide_many(vec![(dup.id, Decision::Trash)]).unwrap();
    assert_eq!(f.trash_summary().count, 1);
    assert_eq!(
        sums(&root),
        before,
        "trash er bare indeks, filen ligger der"
    );
    assert_eq!(f.undo().unwrap().batch, batch);
    assert_eq!(
        f.cards(SuggestionKind::ExactDup, 10, None)
            .unwrap()
            .cards
            .len(),
        1
    );
    assert_eq!(sums(&root), before, "angre endrer ikke originaler");

    // Trash på nytt, purge: bare det bekreftede bildet forsvinner, og bare via plattformen.
    let c = f.cards(SuggestionKind::ExactDup, 10, None).unwrap().cards[0].clone();
    f.decide_many(vec![(c.id, Decision::Trash)]).unwrap();
    let intent = f.begin_purge(vec![c.asset]).unwrap();
    let ticket = f.confirm_purge(&intent).unwrap();
    let ids = f.sources_for(ticket.ids());
    let results = TempDelete(root.clone()).delete(&ticket, &ids);
    let rep = f.finish_purge(ticket, results).unwrap();
    assert_eq!(rep.purged, vec![c.asset]);
    let after = sums(&root);
    assert_eq!(after.len(), 2);
    assert!(
        after.iter().all(|(k, v)| before[k] == *v),
        "øvrige originaler uendret"
    );
    assert!(f.trash_summary().freed_bytes > 0);

    // Ny skanning: det slettede er borte fra kilden og avstemmes uten feil.
    f.start_scan(ScanConfig {
        mode: ScanMode::Quick,
        priority: 1,
        deadline: None,
    })
    .unwrap();
    assert!(f.wait_scan().unwrap().unwrap().failures.is_empty());
    fs::remove_dir_all(&root).unwrap();
}
