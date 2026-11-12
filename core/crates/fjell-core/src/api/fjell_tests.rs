//! Fullflyt gjennom `Fjell`: skann → grupper → forslag → trash → angre → purge (M4 DoD).
//!
//! Grensesnitt: bare tester. Bruker syntetisk korpus fra `pipeline::tests`.

use super::*;
use crate::model::{DeleteOutcome, FakeClock, ReasonCode, State};
use crate::pipeline::tests::{Counting, Fx};
use crate::store::Store;
use std::sync::Arc;

fn fjell() -> (Fjell, Arc<Counting>) {
    let mut fx = Fx::new();
    fx.corpus();
    let src = fx.src.clone();
    let f = Fjell::new(
        Store::open_in_memory().unwrap(),
        src.clone(),
        Arc::new(FakeClock::new(5_000_000)),
    );
    (f, src)
}

fn scanned() -> (Fjell, Arc<Counting>) {
    let (f, src) = fjell();
    f.start_scan(ScanConfig {
        mode: ScanMode::Full,
        priority: 1,
        deadline: None,
    })
    .unwrap();
    let r = f.wait_scan().unwrap().unwrap();
    assert!(r.failures.is_empty() && r.analyzed == 28, "{r:?}");
    (f, src)
}

fn all(f: &Fjell, kind: SuggestionKind) -> Vec<Card> {
    let mut out = Vec::new();
    let mut cur = None;
    loop {
        let p = f.cards(kind, 2, cur).unwrap();
        out.extend(p.cards);
        match p.next {
            Some(n) => cur = Some(n),
            None => return out,
        }
    }
}

fn state(f: &Fjell, a: AssetId) -> State {
    f.with_store(|s| s.asset_row(a).unwrap().unwrap().state)
}

#[test]
fn full_flow_scan_groups_suggestions_trash_undo_purge() {
    let (f, src) = scanned();
    let h = f.home_summary();
    assert_eq!(h.scan.phase, ScanPhase::Done);
    assert_eq!((h.scan.done, h.scan.total, h.scan.found_exact), (36, 36, 4));
    let n = |k| h.per_kind.iter().find(|x| x.kind == k).unwrap().count;
    assert_eq!(
        (n(SuggestionKind::ExactDup), n(SuggestionKind::VisualDup)),
        (4, 3)
    );
    assert!(n(SuggestionKind::Blurry) >= 1 && n(SuggestionKind::Screenshot) == 1);
    assert!(h.reclaimable_bytes > 0);

    // Kort: sidering, keeper og begrunnelse; samme asset aldri i to kort.
    let exact = all(&f, SuggestionKind::ExactDup);
    assert_eq!(exact.len(), 4);
    assert!(exact
        .iter()
        .all(|c| c.keeper.is_some() && c.reason == ReasonCode::ExactCopy && c.confidence == 1.0));
    let mut seen = std::collections::BTreeSet::new();
    for k in SuggestionKind::ALL {
        for c in all(&f, k) {
            assert!(seen.insert(c.asset), "asset i flere kort: {c:?}");
        }
    }

    // Trash alle eksakte kort: én batch, tilstand trashed, kortene er borte.
    let originals_before = src.counts();
    let batch = f
        .decide_many(exact.iter().map(|c| (c.id, Decision::Trash)).collect())
        .unwrap();
    assert!(exact.iter().all(|c| state(&f, c.asset) == State::Trashed));
    assert_eq!(f.trash_summary().count, 4);
    assert!(all(&f, SuggestionKind::ExactDup).is_empty());
    assert!(exact
        .iter()
        .all(|c| state(&f, c.keeper.unwrap()) == State::Active));
    // Samme beslutning to ganger: kortet er avgjort.
    assert!(matches!(
        f.decide_many(vec![(exact[0].id, Decision::Trash)]),
        Err(ApiError::UnknownCard(_))
    ));

    // Angre: alt tilbake, kortene er der igjen.
    let u = f.undo().unwrap();
    assert_eq!((u.batch, u.restored), (batch, 4));
    assert!(exact.iter().all(|c| state(&f, c.asset) == State::Active));
    assert_eq!(all(&f, SuggestionKind::ExactDup).len(), 4);
    assert_eq!(f.trash_summary().count, 0);

    // Purge uten å ha vært i papirkurven: nektes.
    assert!(matches!(
        f.begin_purge(vec![exact[0].asset]),
        Err(ApiError::NotTrashed(_))
    ));

    // Trash på nytt og tøm: begin → confirm → plattform → finish.
    let cards = all(&f, SuggestionKind::ExactDup);
    f.decide_many(cards.iter().map(|c| (c.id, Decision::Trash)).collect())
        .unwrap();
    let ids: Vec<AssetId> = cards.iter().map(|c| c.asset).collect();
    let intent = f.begin_purge(ids.clone()).unwrap();
    assert_eq!(intent.ids.len(), 4);
    let ticket = f.confirm_purge(&intent).unwrap();
    // Plattformen sletter bare to; resten forblir trashed (I15).
    let results = ids
        .iter()
        .enumerate()
        .map(|(i, id)| {
            (
                *id,
                if i < 2 {
                    DeleteOutcome::Deleted
                } else {
                    DeleteOutcome::Cancelled
                },
            )
        })
        .collect();
    let rep = f.finish_purge(ticket.clone(), results).unwrap();
    assert_eq!((rep.purged.len(), rep.kept_trashed.len()), (2, 2));
    assert!(rep.purged.iter().all(|a| state(&f, *a) == State::Purged));
    assert!(rep
        .kept_trashed
        .iter()
        .all(|a| state(&f, *a) == State::Trashed));
    let t = f.trash_summary();
    assert_eq!(t.count, 2);
    assert!(t.freed_bytes > 0);
    // Billetten er brukt opp; purge kan ikke angres.
    assert!(matches!(
        f.finish_purge(ticket, vec![]),
        Err(ApiError::PurgeStale)
    ));
    assert!(matches!(f.undo(), Err(ApiError::NotUndoable)));
    // Kilden er aldri skrevet til: ingen nye lesinger av api-et selv etter skanningen.
    assert_eq!(src.counts(), originals_before);
}

#[test]
fn protected_images_need_confirmation_and_nothing_is_written_before() {
    let (f, _) = scanned();
    let c = all(&f, SuggestionKind::ExactDup)[0].clone();
    f.with_store(|s| {
        s.conn()
            .execute(
                "UPDATE asset SET protect_flags = 1 WHERE id = ?1",
                [c.asset.0 as i64],
            )
            .unwrap();
    });
    let r = f.decide_many(vec![(c.id, Decision::Trash)]);
    assert_eq!(
        r,
        Err(ApiError::AckRequired(vec![
            ConsequenceCode::ProtectedFavorite
        ]))
    );
    assert_eq!(state(&f, c.asset), State::Active);
    assert_eq!(f.trash_summary().count, 0);
    // Feil kode bekrefter ikke.
    let wrong = f.decide_many_confirmed(
        vec![(c.id, Decision::Trash)],
        vec![ConsequenceCode::MassDelete],
    );
    assert!(matches!(wrong, Err(ApiError::AckRequired(_))));
    f.decide_many_confirmed(
        vec![(c.id, Decision::Trash)],
        vec![ConsequenceCode::ProtectedFavorite],
    )
    .unwrap();
    assert_eq!(state(&f, c.asset), State::Trashed);
}

#[test]
fn trash_and_keep_in_one_batch_one_undo() {
    let (f, _) = scanned();
    let ex = all(&f, SuggestionKind::ExactDup);
    let vis = all(&f, SuggestionKind::VisualDup);
    let decisions = vec![(ex[0].id, Decision::Trash), (vis[0].id, Decision::Keep)];
    f.decide_many(decisions).unwrap();
    assert_eq!(state(&f, ex[0].asset), State::Trashed);
    assert_eq!(
        all(&f, SuggestionKind::VisualDup).len(),
        2,
        "avvist kort er borte"
    );
    f.undo().unwrap();
    assert_eq!(state(&f, ex[0].asset), State::Active);
    assert_eq!(
        all(&f, SuggestionKind::VisualDup).len(),
        3,
        "angre gjenoppretter avvisningen"
    );
    // Alt eller ingenting: ett ukjent kort stopper hele runden.
    let r = f.decide_many(vec![
        (ex[1].id, Decision::Trash),
        (CardId(999_999), Decision::Trash),
    ]);
    assert!(matches!(r, Err(ApiError::UnknownCard(_))));
    assert_eq!(state(&f, ex[1].asset), State::Active);
}

#[test]
fn set_keeper_swaps_the_suggestion_and_survives_rescan() {
    let (f, _) = scanned();
    let vis = all(&f, SuggestionKind::VisualDup);
    let c = vis[0].clone();
    let (g, old) = (c.group.unwrap(), c.keeper.unwrap());
    f.set_keeper(g, c.asset).unwrap();
    let now = all(&f, SuggestionKind::VisualDup);
    let n = now.iter().find(|x| x.group == Some(g)).unwrap();
    assert_eq!((n.asset, n.keeper), (old, Some(c.asset)));
    // Ny skanning (ingen endringer) og full oppfriskning beholder valget.
    f.start_scan(ScanConfig {
        mode: ScanMode::Full,
        priority: 1,
        deadline: None,
    })
    .unwrap();
    f.wait_scan().unwrap().unwrap();
    let again = all(&f, SuggestionKind::VisualDup);
    assert_eq!(
        again.iter().find(|x| x.group == Some(g)).unwrap().keeper,
        Some(c.asset)
    );
    assert!(matches!(
        f.set_keeper(g, AssetId(99_999)),
        Err(ApiError::UnknownAsset(_))
    ));
}

#[test]
fn trashed_images_stay_trashed_through_a_rescan_and_settings_roundtrip() {
    let (f, src) = scanned();
    let c = all(&f, SuggestionKind::ExactDup)[0].clone();
    f.decide_many(vec![(c.id, Decision::Trash)]).unwrap();
    // Endre metadata for det slettede bildet (ny token): state er uendret (I13).
    let name = f.with_store(|s| {
        s.conn()
            .query_row(
                "SELECT source_id FROM asset WHERE id=?1",
                [c.asset.0 as i64],
                |r| r.get::<_, String>(0),
            )
            .unwrap()
    });
    let mut m = crate::pipeline::tests::meta(&name, 5_000, "ny");
    m.mod_token = "ny".into();
    src.inner.update(m);
    f.start_scan(ScanConfig {
        mode: ScanMode::Quick,
        priority: 0,
        deadline: None,
    })
    .unwrap();
    f.wait_scan().unwrap().unwrap();
    assert_eq!(state(&f, c.asset), State::Trashed);
    f.set_settings(Settings { trash_days: 7 });
    assert_eq!(f.settings().trash_days, 7);
}

#[test]
fn restore_assets_brings_trashed_back_and_is_one_undoable_batch() {
    let (f, _src) = scanned();
    let c = all(&f, SuggestionKind::ExactDup)[0].clone();
    f.decide_many(vec![(c.id, Decision::Trash)]).unwrap();
    assert_eq!(state(&f, c.asset), State::Trashed);
    let b = f.restore_assets(&[c.asset]).unwrap();
    assert!(b.is_some());
    assert_eq!(state(&f, c.asset), State::Active);
    // Allerede aktiv: ingen ny batch.
    assert_eq!(f.restore_assets(&[c.asset]).unwrap(), None);
    // Gjenoppretting kan angres (tilbake i «Nylig slettet»).
    f.undo().unwrap();
    assert_eq!(state(&f, c.asset), State::Trashed);
}
