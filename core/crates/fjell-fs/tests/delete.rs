//! «Tøm»: T28 (AckRequired, Unsupported) og helflyt mot ekte kjerne: skann -> trash -> stage ->
//! begin_purge -> confirm_purge -> `FsDeleter` -> finish_purge (T23-lignende: bare temp-kopi).

mod common;
use common::*;
use fjell_core::api::{ApiError, Decision, Fjell, FjellApi, ScanConfig, ScanMode};
use fjell_core::model::{AssetId, BatchId, Clock, DeleteOutcome, PlatformDelete, PurgeTicket};
use fjell_core::store::Store;
use fjell_fs::{
    DeleteError, DeleteMode, DeleteResult, FakeRecycleBin, FsDeleter, FsQuarantine, FsSource,
    PermanentAck, Quarantine,
};
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

struct Zero;
impl Clock for Zero {
    fn now(&self) -> i64 {
        1_700_000_000
    }
}

/// Tre filer: to identiske (duplikat) og én unik. Returnerer (rot, Fjell, ticket for duplikatet som er trashed).
fn world(tag: &str) -> (PathBuf, Fjell, PurgeTicket, Vec<AssetId>) {
    let root = tempdir(tag);
    let j = jpeg(64, 48, None);
    write(&root, "a.jpg", &j);
    write(&root, "dup/a2.jpg", &j);
    write(&root, "other.jpg", &jpeg(50, 50, None));
    let db = tempdir(&format!("{tag}-db")).join("i.db");
    let f = Fjell::new(
        Store::open(&db).unwrap(),
        Arc::new(FsSource::new(&root)),
        Arc::new(Zero),
    );
    f.start_scan(ScanConfig {
        mode: ScanMode::Full,
        priority: 1,
        deadline: None,
    })
    .unwrap();
    f.wait_scan().unwrap().unwrap();
    let cards = f
        .cards(fjell_core::model::SuggestionKind::ExactDup, 100, None)
        .unwrap()
        .cards;
    assert_eq!(cards.len(), 1, "ett duplikatkort");
    let ds: Vec<_> = cards.iter().map(|c| (c.id, Decision::Trash)).collect();
    match f.decide_many(ds.clone()) {
        Err(ApiError::AckRequired(codes)) => f.decide_many_confirmed(ds, codes).unwrap(),
        r => r.unwrap(),
    };
    let intent = f.begin_purge(vec![cards[0].asset]).unwrap();
    let ticket = match f.confirm_purge(&intent) {
        Err(ApiError::AckRequired(codes)) => f.confirm_purge_confirmed(&intent, codes).unwrap(),
        r => r.unwrap(),
    };
    let ids = ticket.ids().to_vec();
    (root, f, ticket, ids)
}

fn bin() -> Arc<FakeRecycleBin> {
    Arc::new(FakeRecycleBin::default())
}

#[test]
fn t28_permanent_without_ack_is_ack_required_and_nothing_called() {
    let (root, _f, _t, _ids) = world("t28a");
    let b = bin();
    let r = FsDeleter::new(&root, None, b.clone(), DeleteMode::Permanent, None);
    assert!(matches!(r, Err(DeleteError::AckRequired)));
    assert!(
        PermanentAck::new(false, 1).is_none(),
        "uten avkrysning ingen ack"
    );
    assert!(b.calls.lock().unwrap().is_empty());
    assert_eq!(snapshot(&root).len(), 3, "ingen fil slettet");
}

#[test]
fn t28_ack_count_must_match_ticket() {
    let (root, _f, t, ids) = world("t28b");
    let d = FsDeleter::new(
        &root,
        None,
        bin(),
        DeleteMode::Permanent,
        PermanentAck::new(true, 5),
    )
    .unwrap();
    let src = _f.sources_for(&ids);
    assert_eq!(d.delete_detailed(&t, &src), Err(DeleteError::AckMismatch));
    assert!(d
        .delete(&t, &src)
        .iter()
        .all(|(_, o)| *o == DeleteOutcome::Failed));
    assert_eq!(snapshot(&root).len(), 3);
}

#[test]
fn t28_unsupported_volume_is_not_deleted_and_not_silently_permanent() {
    let (root, f, t, ids) = world("t28c");
    let src = f.sources_for(&ids);
    let b = Arc::new(FakeRecycleBin {
        unsupported_under: vec![root.clone()],
        ..Default::default()
    });
    let d = FsDeleter::new(&root, None, b.clone(), DeleteMode::Recycle, None).unwrap();
    let det = d.delete_detailed(&t, &src).unwrap();
    assert!(det.iter().all(|(_, r)| *r == DeleteResult::Unsupported));
    assert!(d
        .delete(&t, &src)
        .iter()
        .all(|(_, o)| *o == DeleteOutcome::Failed));
    assert_eq!(snapshot(&root).len(), 3, "filen er der fortsatt");
    // Bare et eksplisitt valg om permanent (med ack) sletter.
    let ack = PermanentAck::new(true, ids.len());
    let p = FsDeleter::new(&root, None, b, DeleteMode::Permanent, ack).unwrap();
    assert!(p
        .delete(&t, &src)
        .iter()
        .all(|(_, o)| *o == DeleteOutcome::Deleted));
    assert_eq!(snapshot(&root).len(), 2);
}

#[test]
fn recycle_mode_moves_to_bin_and_core_marks_purged() {
    let (root, f, t, ids) = world("recycle");
    let src = f.sources_for(&ids);
    let bin_dir = tempdir("recycle-bin");
    let b = Arc::new(FakeRecycleBin {
        bin_dir: Some(bin_dir.clone()),
        ..Default::default()
    });
    let d = FsDeleter::new(&root, None, b.clone(), DeleteMode::Recycle, None).unwrap();
    let res = d.delete(&t, &src);
    assert!(res.iter().all(|(_, o)| *o == DeleteOutcome::Deleted));
    assert_eq!(b.calls.lock().unwrap().len(), ids.len());
    assert_eq!(fs::read_dir(&bin_dir).unwrap().count(), ids.len());
    let rep = f.finish_purge(t, res).unwrap();
    assert_eq!(rep.purged.len(), ids.len());
}

#[test]
fn staged_file_is_deleted_from_quarantine_and_manifest_cleared() {
    let (root, f, t, ids) = world("staged");
    let src = f.sources_for(&ids);
    let q = Arc::new(FsQuarantine::in_library(&root));
    let staged = q.stage(BatchId(1), &src);
    assert!(staged.iter().all(|(_, r)| r.is_ok()));
    let kept = snapshot(&root); // uten karantenemappen
    assert_eq!(kept.len(), 2, "duplikatet er flyttet bort fra biblioteket");
    // Skanning ekskluderer karantenen; originalsettet er uendret av `stage` utover flyttingen.
    let b = bin();
    let d = FsDeleter::new(&root, Some(q.clone()), b.clone(), DeleteMode::Recycle, None).unwrap();
    let res = d.delete(&t, &src);
    assert!(res.iter().all(|(_, o)| *o == DeleteOutcome::Deleted));
    assert!(b.calls.lock().unwrap()[0].starts_with(root.join(".fjell-karantene")));
    assert!(q.staged().is_empty());
    assert_eq!(snapshot(&root), kept, "andre filer urørt");
}

#[test]
fn offline_root_deletes_nothing() {
    let (root, f, t, ids) = world("offdel");
    let src = f.sources_for(&ids);
    let d = FsDeleter::new(&root, None, bin(), DeleteMode::Recycle, None).unwrap();
    let away = root.with_extension("away");
    fs::rename(&root, &away).unwrap();
    let det = d.delete_detailed(&t, &src).unwrap();
    assert!(det.iter().all(|(_, r)| *r == DeleteResult::Unavailable));
    fs::rename(&away, &root).unwrap();
    assert_eq!(snapshot(&root).len(), 3);
}

#[test]
fn ids_outside_ticket_are_refused() {
    let (root, f, t, ids) = world("outside");
    let mut src = f.sources_for(&ids);
    src.push((
        AssetId(9999),
        fjell_core::model::SourceId("other.jpg".into()),
    ));
    let b = bin();
    let d = FsDeleter::new(&root, None, b, DeleteMode::Recycle, None).unwrap();
    let res = d.delete_detailed(&t, &src).unwrap();
    assert_eq!(res.last().unwrap().1, DeleteResult::Failed);
    assert!(root.join("other.jpg").is_file());
}
