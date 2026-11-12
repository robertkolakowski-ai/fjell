//! Enhetstester for `api`.

use super::*;
use crate::model::{FakeClock, ProtectFlags, ReasonCode};

fn card(n: u64, kind: SuggestionKind) -> Card {
    Card {
        id: CardId(n),
        kind,
        asset: AssetId(n),
        group: Some(GroupId(1)),
        keeper: Some(AssetId(100)),
        reason: ReasonCode::ExactCopy,
        confidence: 1.0,
        bytes: 10,
        protect_flags: ProtectFlags::default(),
    }
}

fn api() -> MemApi {
    let cards = (1..=5)
        .map(|n| card(n, SuggestionKind::ExactDup))
        .chain([card(6, SuggestionKind::Blurry)]);
    MemApi::new(cards.collect(), Arc::new(FakeClock::new(1000)))
}

#[test]
fn paging_by_kind() {
    let a = api();
    let p1 = a.cards(SuggestionKind::ExactDup, 2, None).unwrap();
    assert_eq!(p1.cards.len(), 2);
    let p2 = a.cards(SuggestionKind::ExactDup, 10, p1.next).unwrap();
    assert_eq!(p2.cards.len(), 3);
    assert!(p2.next.is_none());
    assert_eq!(
        a.cards(SuggestionKind::Blurry, 10, None)
            .unwrap()
            .cards
            .len(),
        1
    );
}

#[test]
fn decide_is_one_batch_and_undo_restores() {
    let a = api();
    let b = a.decide_many(vec![
        (CardId(1), Decision::Trash),
        (CardId(2), Decision::Trash),
        (CardId(3), Decision::Keep),
    ]);
    assert_eq!(b, Ok(BatchId(1)));
    assert_eq!(
        a.trash_summary(),
        TrashSummary {
            count: 2,
            bytes: 20,
            expired: 0,
            freed_bytes: 0
        }
    );
    assert_eq!(
        a.cards(SuggestionKind::ExactDup, 10, None)
            .unwrap()
            .cards
            .len(),
        2
    );
    let r = a.undo().unwrap();
    assert_eq!((r.batch, r.restored), (BatchId(1), 2));
    assert_eq!(
        a.cards(SuggestionKind::ExactDup, 10, None)
            .unwrap()
            .cards
            .len(),
        5
    );
    assert_eq!(a.undo(), Err(ApiError::NothingToUndo));
}

#[test]
fn decide_is_all_or_nothing() {
    let a = api();
    let r = a.decide_many(vec![
        (CardId(1), Decision::Trash),
        (CardId(99), Decision::Trash),
    ]);
    assert_eq!(r, Err(ApiError::UnknownCard(CardId(99))));
    assert_eq!(a.state_of(AssetId(1)), Some(State::Active));
    a.decide_many(vec![(CardId(1), Decision::Keep)]).unwrap();
    assert!(a.decide_many(vec![(CardId(1), Decision::Trash)]).is_err());
}

#[test]
fn purge_needs_trash_and_partial_results_lose_nothing() {
    let a = api();
    assert_eq!(a.begin_purge(vec![AssetId(1)]), Err(ApiError::NotUndoable));
    a.decide_many(vec![
        (CardId(1), Decision::Trash),
        (CardId(2), Decision::Trash),
    ])
    .unwrap();
    let intent = a.begin_purge(vec![AssetId(1), AssetId(2)]).unwrap();
    assert_eq!(intent.bytes, 20);
    let mut forged = intent.clone();
    forged.ids.push(AssetId(3));
    assert_eq!(a.confirm_purge(&forged), Err(ApiError::PurgeStale));
    let ticket = a.confirm_purge(&intent).unwrap();
    assert_eq!(ticket.issued_at(), 1000);
    let rep = a
        .finish_purge(
            ticket,
            vec![
                (AssetId(1), DeleteOutcome::Deleted),
                (AssetId(2), DeleteOutcome::Cancelled),
            ],
        )
        .unwrap();
    assert_eq!(
        (rep.purged, rep.kept_trashed),
        (vec![AssetId(1)], vec![AssetId(2)])
    );
    assert_eq!(a.state_of(AssetId(1)), Some(State::Purged));
    assert_eq!(a.state_of(AssetId(2)), Some(State::Trashed));
    assert_eq!(a.trash_summary().freed_bytes, 10);
    assert_eq!(
        a.begin_purge(vec![AssetId(1)]),
        Err(ApiError::AlreadyPurged(AssetId(1)))
    );
}

#[test]
fn summary_keeper_scan_and_settings() {
    let a = api();
    let h = a.home_summary();
    assert_eq!(h.reclaimable_bytes, 60);
    assert_eq!(h.per_kind.len(), 4);
    assert_eq!(h.scan.phase, ScanPhase::Idle);
    a.start_scan(ScanConfig {
        mode: super::super::ScanMode::Quick,
        priority: 1,
        deadline: None,
    })
    .unwrap();
    assert_eq!(a.home_summary().scan.phase, ScanPhase::Analyzing);
    a.cancel();
    assert_eq!(a.home_summary().scan.phase, ScanPhase::Idle);
    a.set_keeper(GroupId(1), AssetId(2)).unwrap();
    assert_eq!(
        a.cards(SuggestionKind::ExactDup, 1, None).unwrap().cards[0].keeper,
        Some(AssetId(2))
    );
    assert_eq!(
        a.set_keeper(GroupId(1), AssetId(77)),
        Err(ApiError::UnknownAsset(AssetId(77)))
    );
    a.set_settings(Settings { trash_days: 7 });
    assert_eq!(a.settings().trash_days, 7);
}
