//! Karantene: T25 (stage/unstage, sjekksum, kollisjon), T26 (avbrutt mellom commit/stage/manifest),
//! T27 (frakoblet rot), pluss samme-volum-regel og at innhold aldri endres.

mod common;
use common::*;
use fjell_core::model::{AssetId, BatchId, SourceId};
use fjell_fs::manifest::{Entry, Manifest};
use fjell_fs::{FsQuarantine, Quarantine, QuarantineError, Reconciled};
use std::fs;

fn sid(s: &str) -> SourceId {
    SourceId(s.into())
}
fn items(v: &[(u64, &str)]) -> Vec<(AssetId, SourceId)> {
    v.iter().map(|(i, s)| (AssetId(*i), sid(s))).collect()
}

fn setup(tag: &str) -> (std::path::PathBuf, FsQuarantine) {
    let root = tempdir(tag);
    write(&root, "a.jpg", &jpeg(30, 20, Some(6)));
    write(&root, "sub/b.jpg", &jpeg(30, 20, None));
    write(&root, "sub/c.txt", b"x");
    let q = FsQuarantine::in_library(&root);
    (root, q)
}

#[test]
fn t25_stage_then_unstage_restores_identical_file() {
    let (root, q) = setup("t25");
    let before = snapshot(&root);
    let r = q.stage(BatchId(7), &items(&[(1, "a.jpg"), (2, "sub/b.jpg")]));
    assert!(r.iter().all(|(_, x)| x.is_ok()), "{r:?}");
    assert!(!root.join("a.jpg").exists() && !root.join("sub/b.jpg").exists());
    let bdir = root.join(".fjell-karantene/7");
    assert!(bdir.join("manifest.json").is_file() && bdir.join("README.txt").is_file());
    let m = Manifest::load(&bdir).unwrap();
    assert_eq!(m.entries.len(), 2);
    assert_eq!(m.entries[1].original, "sub/b.jpg");
    assert!(m.entries[0].bytes > 0);
    assert_eq!(q.staged().len(), 2);
    assert!(q.locate(AssetId(1)).unwrap().is_file());

    let u = q.unstage(&[AssetId(1), AssetId(2)]);
    assert_eq!(u[0].1, Ok(sid("a.jpg")));
    assert_eq!(u[1].1, Ok(sid("sub/b.jpg")));
    assert_eq!(snapshot(&root), before, "sjekksum og sti lik");
    assert!(!root.join(".fjell-karantene/7").exists(), "manifest ryddet");
    assert!(q.staged().is_empty());
}

#[test]
fn t25_collision_gives_gjenopprettet_name_and_never_overwrites() {
    let (root, q) = setup("t25c");
    q.stage(BatchId(1), &items(&[(1, "a.jpg")]));
    write(&root, "a.jpg", b"new file in the way");
    write(&root, "a (gjenopprettet).jpg", b"also taken");
    let u = q.unstage(&[AssetId(1)]);
    assert_eq!(u[0].1, Ok(sid("a (gjenopprettet 2).jpg")));
    assert_eq!(
        fs::read(root.join("a.jpg")).unwrap(),
        b"new file in the way"
    );
    assert_eq!(
        fs::read(root.join("a (gjenopprettet).jpg")).unwrap(),
        b"also taken"
    );
    assert!(root.join("a (gjenopprettet 2).jpg").is_file());
}

#[test]
fn stage_errors_leave_files_alone() {
    let (root, q) = setup("errs");
    let r = q.stage(
        BatchId(1),
        &items(&[(1, "missing.jpg"), (2, "../evil.jpg"), (3, "sub")]),
    );
    assert!(r.iter().all(|(_, x)| x.is_err()));
    assert!(
        !root.join(".fjell-karantene").exists(),
        "ingen tom karantene"
    );
    // stage er idempotent for allerede staged id; unstage av ukjent gir NotFound.
    q.stage(BatchId(2), &items(&[(5, "a.jpg")]));
    assert!(q.stage(BatchId(3), &items(&[(5, "a.jpg")]))[0].1.is_ok());
    assert_eq!(
        q.unstage(&[AssetId(99)])[0].1,
        Err(QuarantineError::NotFound)
    );
}

#[test]
fn t26_killed_between_commit_and_stage_keeps_file_and_reports_not_moved() {
    let (root, q) = setup("t26a");
    // Manifest (hensikt) finnes, men ingen flytting skjedde.
    let m = Manifest {
        batch: 3,
        entries: vec![Entry {
            id: 1,
            original: "a.jpg".into(),
            stored: "1.jpg".into(),
            bytes: 1,
        }],
    };
    m.save(&root.join(".fjell-karantene/3")).unwrap();
    let before = snapshot(&root);
    let r = q.reconcile(&items(&[(1, "a.jpg")]));
    assert_eq!(r, vec![(AssetId(1), Reconciled::NotMoved)]);
    assert_eq!(snapshot(&root), before, "ingen fil mistet");
    assert!(q.staged().is_empty(), "foreldet manifestoppføring ryddet");
    // Og uten noe manifest i det hele tatt: samme svar.
    let r = q.reconcile(&items(&[(1, "a.jpg")]));
    assert_eq!(r, vec![(AssetId(1), Reconciled::NotMoved)]);
}

#[test]
fn t26_killed_between_stage_and_index_restores_file() {
    let (root, q) = setup("t26b");
    let before = snapshot(&root);
    q.stage(BatchId(4), &items(&[(1, "a.jpg"), (2, "sub/b.jpg")]));
    // Indeksen kjenner bare id 2 som trashed (id 1 ble aldri committet): 1 flyttes tilbake.
    let r = q.reconcile(&items(&[(2, "sub/b.jpg")]));
    assert!(r.contains(&(AssetId(1), Reconciled::Restored(sid("a.jpg")))));
    assert!(r.contains(&(AssetId(2), Reconciled::StillStaged)));
    assert!(root.join("a.jpg").is_file() && !root.join("sub/b.jpg").exists());
    q.unstage(&[AssetId(2)]);
    assert_eq!(snapshot(&root), before);
}

#[test]
fn t26_manifest_lost_or_file_gone_never_loses_files() {
    let (root, q) = setup("t26c");
    q.stage(BatchId(5), &items(&[(1, "a.jpg"), (2, "sub/b.jpg")]));
    // Manifest forsvant: filene blir i karantene (ingen sletting), avstemming rører dem ikke.
    fs::remove_file(root.join(".fjell-karantene/5/manifest.json")).unwrap();
    let r = q.reconcile(&items(&[(1, "a.jpg"), (2, "sub/b.jpg")]));
    assert!(
        r.iter().all(|(_, x)| *x == Reconciled::Orphaned),
        "ikke purged: {r:?}"
    );
    assert!(root.join(".fjell-karantene/5/1.jpg").is_file());
    assert!(root.join(".fjell-karantene/5/2.jpg").is_file());
}

#[test]
fn t26_trashed_but_gone_everywhere_is_missing() {
    let (root, q) = setup("t26d");
    q.stage(BatchId(6), &items(&[(1, "a.jpg")]));
    fs::remove_file(root.join(".fjell-karantene/6/1.jpg")).unwrap(); // slettet utenfra
    let r = q.reconcile(&items(&[(1, "a.jpg")]));
    assert_eq!(r, vec![(AssetId(1), Reconciled::Missing)]);
}

#[test]
fn t27_offline_root_changes_nothing_and_is_never_missing() {
    let (root, q) = setup("t27");
    q.stage(BatchId(1), &items(&[(1, "a.jpg")]));
    let gone = root.with_extension("away");
    fs::rename(&root, &gone).unwrap(); // «disk trukket ut»
    let t = items(&[(1, "a.jpg")]);
    assert_eq!(q.reconcile(&t), vec![(AssetId(1), Reconciled::Unavailable)]);
    assert_eq!(
        q.stage(BatchId(2), &items(&[(9, "sub/b.jpg")]))[0].1,
        Err(QuarantineError::Unavailable)
    );
    assert_eq!(
        q.unstage(&[AssetId(1)])[0].1,
        Err(QuarantineError::Unavailable)
    );
    fs::rename(&gone, &root).unwrap(); // disken er tilbake
    assert_eq!(q.reconcile(&t), vec![(AssetId(1), Reconciled::StillStaged)]);
    assert_eq!(q.unstage(&[AssetId(1)])[0].1, Ok(sid("a.jpg")));
}

#[test]
fn unstage_recreates_missing_parent_dir() {
    let (root, q) = setup("parent");
    q.stage(BatchId(1), &items(&[(1, "sub/b.jpg"), (2, "sub/c.txt")]));
    fs::remove_dir(root.join("sub")).unwrap();
    assert_eq!(q.unstage(&[AssetId(1)])[0].1, Ok(sid("sub/b.jpg")));
    assert!(root.join("sub/b.jpg").is_file());
}
