//! Herding (M8): massesletting krever bekreftelse både ved trash og purge (I7, I12), `keeper_reason`
//! lagres, grupper holder seg riktige ved bakgrunnsoppdatering, og store indekser regner gruppene
//! utenfor handlingens vei. Del av `api` (bare tester).

use super::*;
use crate::model::{
    AssetMeta, DeleteOutcome, FakeClock, Kind, MemSource, ProtectFlags, ReasonCode, SourceId, State,
};
use crate::store::Store;
use std::sync::Arc;

/// `pairs` par med identiske bytes (eksakte duplikater): `pairs` kort å felle. Bare rask fase (ingen miniatyr).
fn pairs_fjell(pairs: usize) -> Fjell {
    pairs_fjell_in(pairs, Store::open_in_memory().unwrap())
}

fn pairs_fjell_in(pairs: usize, store: Store) -> Fjell {
    let src = MemSource::new();
    for p in 0..pairs {
        let bytes = format!("pair-{p}-{}", "x".repeat(p % 37)).into_bytes();
        for (k, name) in ["a", "b"].into_iter().enumerate() {
            src.put(
                AssetMeta {
                    source_id: SourceId(format!("p{p:04}{name}.jpg")),
                    kind: Kind::Photo,
                    created_at: 1_000 + (p * 2 + k) as i64,
                    mod_token: "m".into(),
                    width: 100,
                    height: 100,
                    bytes: bytes.len() as u64,
                    mime: "image/jpeg".into(),
                    platform_screenshot_hint: false,
                    has_camera_exif: Some(true),
                    is_local: true,
                    protect_flags: ProtectFlags::default(),
                    albums: vec![],
                    backup: None,
                },
                None,
                bytes.clone(),
            );
        }
    }
    let f = Fjell::new(store, Arc::new(src), Arc::new(FakeClock::new(5_000_000)));
    f.start_scan(ScanConfig {
        mode: ScanMode::Quick,
        priority: 1,
        deadline: None,
    })
    .unwrap();
    f.wait_scan().unwrap().unwrap();
    f
}

fn exact(f: &Fjell) -> Vec<Card> {
    let mut v = Vec::new();
    let mut cur = None;
    loop {
        let p = f.cards(SuggestionKind::ExactDup, 100, cur).unwrap();
        v.extend(p.cards);
        match p.next {
            Some(n) => cur = Some(n),
            None => return v,
        }
    }
}

fn state(f: &Fjell, a: AssetId) -> State {
    f.with_store(|s| s.asset_row(a).unwrap().unwrap().state)
}

const MASS: usize = 60; // over terskelen på 50

#[test]
fn mass_trash_requires_mass_ack_and_writes_nothing_without_it() {
    let f = pairs_fjell(MASS);
    let cards = exact(&f);
    assert_eq!(cards.len(), MASS);
    let d: Vec<_> = cards.iter().map(|c| (c.id, Decision::Trash)).collect();
    assert_eq!(
        f.decide_many(d.clone()),
        Err(ApiError::AckRequired(vec![ConsequenceCode::MassDelete]))
    );
    assert_eq!(
        f.trash_summary().count,
        0,
        "ingenting skrevet uten bekreftelse"
    );
    assert!(cards.iter().all(|c| state(&f, c.asset) == State::Active));
    // Feil kode (beskyttet) dekker ikke massesletting.
    assert!(matches!(
        f.decide_many_confirmed(d.clone(), vec![ConsequenceCode::ProtectedFavorite]),
        Err(ApiError::AckRequired(_))
    ));
    assert_eq!(f.trash_summary().count, 0);
    // Under terskelen trengs ingen bekreftelse.
    f.decide_many(d[..10].to_vec()).unwrap();
    assert_eq!(f.trash_summary().count, 10);
    f.undo().unwrap();
    // Bekreftet: én batch, ett angre.
    f.decide_many_confirmed(d, vec![ConsequenceCode::MassDelete])
        .unwrap();
    assert_eq!(f.trash_summary().count, MASS as u64);
    assert_eq!(f.undo().unwrap().restored, MASS as u64);
    assert_eq!(f.trash_summary().count, 0);
}

#[test]
fn mass_purge_requires_mass_ack_and_partial_results_stay_trashed() {
    let f = pairs_fjell(MASS);
    let cards = exact(&f);
    let ids: Vec<AssetId> = cards.iter().map(|c| c.asset).collect();
    f.decide_many_confirmed(
        cards.iter().map(|c| (c.id, Decision::Trash)).collect(),
        vec![ConsequenceCode::MassDelete],
    )
    .unwrap();
    let intent = f.begin_purge(ids.clone()).unwrap();
    // Uten bekreftelse: ingen billett, ingenting purget.
    assert_eq!(
        f.confirm_purge(&intent).unwrap_err(),
        ApiError::AckRequired(vec![ConsequenceCode::MassDelete])
    );
    assert!(ids.iter().all(|a| state(&f, *a) == State::Trashed));
    // Bekreftet: billett. Plattformen sletter bare halvparten; resten forblir i papirkurven (I15).
    let t = f
        .confirm_purge_confirmed(&intent, vec![ConsequenceCode::MassDelete])
        .unwrap();
    let results: Vec<_> = ids
        .iter()
        .enumerate()
        .map(|(i, a)| {
            (
                *a,
                if i % 2 == 0 {
                    DeleteOutcome::Deleted
                } else {
                    DeleteOutcome::Cancelled
                },
            )
        })
        .collect();
    let r = f.finish_purge(t, results).unwrap();
    assert_eq!((r.purged.len(), r.kept_trashed.len()), (MASS / 2, MASS / 2));
    let trashed = ids
        .iter()
        .filter(|a| state(&f, **a) == State::Trashed)
        .count();
    assert_eq!(trashed, MASS / 2);
}

#[test]
fn mass_ack_is_bound_to_the_set_it_was_given_for() {
    // En bekreftelse for én mengde dekker ikke en annen: etter at mengden endres må ny `begin_purge` til.
    let f = pairs_fjell(MASS);
    let cards = exact(&f);
    f.decide_many_confirmed(
        cards.iter().map(|c| (c.id, Decision::Trash)).collect(),
        vec![ConsequenceCode::MassDelete],
    )
    .unwrap();
    let ids: Vec<AssetId> = cards.iter().map(|c| c.asset).collect();
    let intent = f.begin_purge(ids.clone()).unwrap();
    // Én av dem gjenopprettes etter forhåndsvisningen: mengden er endret.
    f.restore_assets(&ids[..1]).unwrap();
    let r = f.confirm_purge_confirmed(&intent, vec![ConsequenceCode::MassDelete]);
    assert!(
        r.is_err(),
        "foreldet forhåndsvisning gir ingen billett: {r:?}"
    );
}

#[test]
fn keeper_reason_is_stored_and_cleared_by_manual_choice() {
    let f = pairs_fjell(3);
    let c = exact(&f)[0].clone();
    let g = c.group.unwrap();
    let reason = |f: &Fjell| {
        f.with_store(|s| {
            s.pending_cards(SuggestionKind::ExactDup, 100, None)
                .unwrap()
                .into_iter()
                .find(|r| r.group == Some(g))
                .map(|r| r.keeper_reason)
        })
    };
    // Eksakt gruppe: keeper er alltid «eldst».
    assert_eq!(reason(&f), Some(Some(ReasonCode::Older.key().to_string())));
    // Overlever ny utregning (gruppen gjenbrukes).
    f.sync_groups().unwrap();
    assert_eq!(reason(&f), Some(Some(ReasonCode::Older.key().to_string())));
    // Manuelt valg: begrunnelsen «eldst» gjelder ikke lenger.
    f.set_keeper(g, c.asset).unwrap();
    let after: Vec<_> = f.with_store(|s| {
        s.pending_cards(SuggestionKind::ExactDup, 100, None)
            .unwrap()
            .into_iter()
            .filter(|r| r.group == Some(g))
            .map(|r| r.keeper_reason)
            .collect()
    });
    assert!(after.iter().all(|r| r.is_none()), "{after:?}");
}

/// Bakgrunnsutregningen gir samme grupper som en synkron en: etter mange handlinger på rad og
/// `sync_groups` er indeksen lik en ny skanning fra bunnen (ingen foreldede forslag, ingen duplikater).
#[test]
fn background_refresh_converges_to_same_groups() {
    let f = pairs_fjell(30);
    for _ in 0..25 {
        let cards = exact(&f);
        f.decide_many(
            cards
                .iter()
                .take(3)
                .map(|c| (c.id, Decision::Trash))
                .collect(),
        )
        .unwrap();
        f.undo().unwrap();
    }
    f.sync_groups().unwrap();
    let key = |f: &Fjell| {
        let mut v: Vec<_> = exact(f)
            .iter()
            .map(|c| (c.asset, c.keeper, c.reason))
            .collect();
        v.sort_by_key(|x| x.0);
        v
    };
    let got = key(&f);
    let fresh = pairs_fjell(30);
    assert_eq!(got, key(&fresh));
    assert_eq!(f.with_store(|s| s.verify().unwrap()), Vec::<String>::new());
    let groups: i64 = f.with_store(|s| {
        s.conn()
            .query_row("SELECT COUNT(*) FROM grp", [], |r| r.get(0))
            .unwrap()
    });
    assert_eq!(groups, 30, "ingen dupliserte grupper");
}

/// Filbasert indeks over `SYNC_REFRESH_MAX_ROWS`: utregningen går i bakgrunnen (lesetilkobling, diff,
/// `change_counter`-sjekk). Mange raske handlinger konvergerer likevel til samme tilstand som en ny skanning,
/// uten dupliserte grupper og uten avvik i `verify`.
#[test]
fn async_refresh_on_file_index_converges_and_never_duplicates_groups() {
    let dir = std::env::temp_dir().join(format!("fjell-hard-async-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    const P: usize = 1_100; // 2 200 rader > 2 000
    let f = pairs_fjell_in(P, Store::open(&dir.join("i.db")).unwrap());
    assert!(f.with_store(|s| s.index_rows().unwrap().len()) > 2_000);
    for k in 0..40 {
        let cards = f.cards(SuggestionKind::ExactDup, 30, None).unwrap().cards;
        let n = 1 + k % 7;
        f.decide_many(
            cards
                .iter()
                .take(n)
                .map(|c| (c.id, Decision::Trash))
                .collect(),
        )
        .unwrap();
        if k % 2 == 0 {
            f.undo().unwrap();
        }
    }
    f.sync_groups().unwrap();
    assert_eq!(f.with_store(|s| s.verify().unwrap()), Vec::<String>::new());
    let (groups, dup): (i64, i64) = f.with_store(|s| {
        let c = s.conn();
        (
            c.query_row("SELECT COUNT(*) FROM grp", [], |r| r.get(0)).unwrap(),
            c.query_row(
                "SELECT COUNT(*) FROM (SELECT grp_id FROM grp_member GROUP BY grp_id HAVING COUNT(*) < 1)",
                [],
                |r| r.get(0),
            )
            .unwrap(),
        )
    });
    let trashed = f.trash_summary().count as i64;
    // Hver felte kopi er ett bilde i en gruppe på to; grupper med bare ett aktivt medlem forsvinner.
    assert_eq!(dup, 0);
    assert_eq!(groups + trashed, P as i64, "grupper + felte = par");
    // De gjenværende kortene stemmer med en frisk skanning av samme tilstand.
    let live = f
        .cards(SuggestionKind::ExactDup, 5_000, None)
        .unwrap()
        .cards
        .len() as i64;
    assert_eq!(live, groups);
    let _ = std::fs::remove_dir_all(&dir);
}
