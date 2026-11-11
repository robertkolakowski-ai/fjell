//! Scenariene T1-T10 fra PLAN-TRYGGHET del 6.

use super::*;
use crate::model::{BackupSummary, ProtectFlags};

const FAV: u8 = ProtectFlags::FAVORITE;
const SHARED: u8 = ProtectFlags::SHARED_ALBUM;
const EDITED: u8 = ProtectFlags::EDITED;

/// T1: swipe slett, så angre. Én batch på hver vei.
#[test]
fn t1_swipe_trash_then_undo() {
    let mut fx = Fx::new();
    let ids = fx.assets(3);
    let b = fx.trash(&ids[..1]).unwrap();
    assert_eq!(fx.state(ids[0]), State::Trashed);
    let u = fx.a.undo().unwrap();
    assert_eq!((u.undone, u.restored), (b, 1));
    assert_ne!(u.inverse, b);
    assert_eq!(fx.state(ids[0]), State::Active);
    let row = fx.a.store().asset_row(ids[0]).unwrap().unwrap();
    assert_eq!((row.trashed_at, row.expires_at), (None, None));
    assert_eq!(
        fx.sql_i64("SELECT COUNT(DISTINCT batch_id) FROM action_log"),
        2
    );
}

/// T2: gruppeforslag med 5 bilder, behold 1; angre gir alle 4 tilbake som én batch.
#[test]
fn t2_group_suggestion_and_undo() {
    let mut fx = Fx::new();
    let ids = fx.assets(5);
    fx.group(1, &ids);
    for id in &ids[1..] {
        fx.suggest(*id, "visual_dup", Some(1));
    }
    let plan = fx.a.plan_pending().unwrap();
    assert_eq!(plan.items.len(), 4);
    assert_eq!(plan.summary.by_kind.visual_dup, 4);
    assert!(plan
        .items
        .iter()
        .all(|i| i.reason == Some(crate::model::ReasonCode::VisualDuplicate)));
    fx.a.commit(&plan, &Acks::default()).unwrap().unwrap();
    assert_eq!(
        fx.sql_i64("SELECT COUNT(*) FROM suggestion WHERE status='accepted'"),
        4
    );
    let u = fx.a.undo().unwrap();
    assert_eq!(u.restored, 4);
    assert!(fx.states(&ids).iter().all(|s| *s == State::Active));
    assert_eq!(
        fx.sql_i64("SELECT COUNT(*) FROM suggestion WHERE status='pending'"),
        4
    );
    assert_eq!(
        fx.sql_i64("SELECT COUNT(DISTINCT batch_id) FROM action_log WHERE cause='undo'"),
        1
    );
}

/// T3: masseforslag 124 bilder (1,8 GB), 3 i albumet «Bryllup».
#[test]
fn t3_mass_suggestion_summary_and_mass_ack() {
    let mut fx = Fx::new();
    let per = 1_800_000_000u64 / 124;
    let ids: Vec<_> = (1..=124).map(|i| fx.asset(i, per, 0, None)).collect();
    fx.album(7, "Bryllup", false, &ids[..3]);
    let plan = fx.a.plan(&PlanRequest::manual(&ids)).unwrap();
    let s = &plan.summary;
    assert_eq!((s.count, s.bytes), (124, per * 124));
    assert_eq!(s.albums.len(), 1);
    assert_eq!(
        (s.albums[0].name.as_str(), s.albums[0].count),
        ("Bryllup", 3)
    );
    assert_eq!(s.backup, BackupSummary::Unknown);
    assert!(!s.frees_storage_now);
    for c in [
        ConsequenceCode::MassDelete,
        ConsequenceCode::BackupUnknown,
        ConsequenceCode::InAlbum,
        ConsequenceCode::FreesNoStorageYet,
    ] {
        assert!(s.codes.contains(&c), "mangler {c:?}");
    }
    assert_ack_required(
        fx.a.commit(&plan, &Acks::default()),
        &[ConsequenceCode::MassDelete],
    );
    assert!(fx.states(&ids).iter().all(|s| *s == State::Active));
    assert!(fx.a.commit(&plan, &acks_covering(&plan)).unwrap().is_some());
    assert_eq!(fx.sql_i64("SELECT COUNT(*) FROM asset WHERE state=1"), 124);
}

/// T4: favoritt i sletteforslag krever ack; standardutvalget («godta alle») utelater den.
#[test]
fn t4_favorite_requires_ack_and_is_not_default() {
    let mut fx = Fx::new();
    let a = fx.asset(1, 3, 0, None);
    let fav = fx.asset(2, 3, FAV, None);
    let c = fx.asset(3, 3, 0, None);
    fx.group(1, &[a, c, fav]);
    for id in [c, fav] {
        fx.suggest(id, "visual_dup", Some(1));
    }
    let manual = fx.a.plan(&PlanRequest::manual(&[fav])).unwrap();
    assert_ack_required(
        fx.a.commit(&manual, &Acks::default()),
        &[ConsequenceCode::ProtectedFavorite],
    );
    assert_eq!(fx.state(fav), State::Active);
    let pending = fx.a.plan_pending().unwrap();
    assert_eq!(
        pending.ids(),
        vec![c],
        "beskyttet bilde skal aldri være i standardutvalget"
    );
}

/// T5: delt album og redigert i samme batch krever hver sin ack.
#[test]
fn t5_one_ack_does_not_cover_both_categories() {
    let mut fx = Fx::new();
    let s = fx.asset(1, 3, SHARED, None);
    let e = fx.asset(2, 3, EDITED, None);
    let plan = fx.a.plan(&PlanRequest::manual(&[s, e])).unwrap();
    assert_eq!(plan.summary.protected.shared_album, 1);
    assert_eq!(plan.summary.protected.edited, 1);
    let all = acks_covering(&plan);
    let only_shared = Acks {
        protected: all
            .protected
            .iter()
            .filter(|a| a.category() == SHARED)
            .cloned()
            .collect(),
        mass: None,
    };
    assert_ack_required(
        fx.a.commit(&plan, &only_shared),
        &[ConsequenceCode::ProtectedEdited],
    );
    // En ack som lister feil id-er dekker heller ikke.
    let wrong = Acks {
        protected: vec![
            crate::model::ProtectedAck::new(SHARED, vec![e], plan.digest),
            crate::model::ProtectedAck::new(EDITED, vec![s], plan.digest),
        ],
        mass: None,
    };
    assert_ack_required(
        fx.a.commit(&plan, &wrong),
        &[
            ConsequenceCode::ProtectedSharedAlbum,
            ConsequenceCode::ProtectedEdited,
        ],
    );
    assert_eq!(fx.states(&[s, e]), vec![State::Active; 2]);
    assert!(fx.a.commit(&plan, &all).unwrap().is_some());
}

/// T6: dry-run skriver ingenting, og påfølgende commit gir samme mengde.
#[test]
fn t6_dry_run_has_no_side_effects_and_commit_matches() {
    let mut fx = Fx::new();
    let ids = fx.assets(6);
    for id in &ids {
        fx.suggest(*id, "blurry", None);
    }
    let (changes, sum) = (fx.total_changes(), dump_sum(&fx.a));
    let plan = fx.a.plan_pending().unwrap();
    assert_eq!((fx.total_changes(), dump_sum(&fx.a)), (changes, sum));
    assert_eq!(plan.summary.count, 6);
    fx.a.commit(&plan, &Acks::default()).unwrap().unwrap();
    let trashed: Vec<_> = ids
        .iter()
        .copied()
        .filter(|i| fx.state(*i) == State::Trashed)
        .collect();
    assert_eq!(trashed, plan.ids());
    assert_eq!(plan.summary.bytes, 18);
}

/// T7: planen blir gammel (endret mellom plan og commit, eller for gammel).
#[test]
fn t7_stale_plan_is_rejected_and_writes_nothing() {
    let mut fx = Fx::new();
    let ids = fx.assets(3);
    let plan = fx.a.plan(&PlanRequest::manual(&ids)).unwrap();
    fx.trash(&ids[..1]);
    let sum = dump_sum(&fx.a);
    assert!(matches!(
        fx.a.commit(&plan, &acks_covering(&plan)),
        Err(ActionError::PlanStale)
    ));
    assert_eq!(dump_sum(&fx.a), sum);
    // Beskyttelse endret etter planen.
    let plan = fx.a.plan(&PlanRequest::manual(&ids[1..])).unwrap();
    fx.a.store()
        .conn()
        .execute("UPDATE asset SET protect_flags = 1 WHERE id = 2", [])
        .unwrap();
    assert!(matches!(
        fx.a.commit(&plan, &Acks::default()),
        Err(ActionError::PlanStale)
    ));
    // For gammel (> 5 min).
    let plan = fx.a.plan(&PlanRequest::manual(&ids[2..])).unwrap();
    fx.clock.advance(301);
    assert!(matches!(
        fx.a.commit(&plan, &Acks::default()),
        Err(ActionError::PlanStale)
    ));
    assert_eq!(fx.state(ids[2]), State::Active);
}

/// T8: 31 dager frem. Elementer er `trashed`, varsles én gang, restore virker, ingen sletting.
#[test]
fn t8_thirty_one_days_notice_once_and_restore_works() {
    let mut fx = Fx::new();
    let ids = fx.assets(4);
    fx.trash(&ids);
    assert_eq!(fx.a.expire_due().unwrap(), None);
    fx.clock.advance(31 * DAY);
    let n = fx.a.expire_due().unwrap().unwrap();
    assert_eq!((n.count, n.bytes), (4, 12));
    assert_eq!(fx.a.expire_due().unwrap(), None);
    assert_eq!(fx.states(&ids), vec![State::Trashed; 4]);
    let sum = fx.a.trash_summary().unwrap();
    assert_eq!(
        (sum.trashed_count, sum.expired_count, sum.freed_bytes),
        (4, 4, 0)
    );
    fx.a.restore(&ids[..2]).unwrap().unwrap();
    assert_eq!(fx.states(&ids[..2]), vec![State::Active; 2]);
}

/// T9: bruker avbryter systemdialogen. Alle forblir `trashed`, ingen `purge` i loggen.
#[test]
fn t9_user_cancels_platform_dialog() {
    let mut fx = Fx::new();
    let ids = fx.assets(3);
    fx.trash(&ids);
    let offer = fx.a.begin_purge(&ids).unwrap();
    assert!(offer.codes.contains(&ConsequenceCode::PlatformDialogNext));
    let t = fx.a.confirm_purge(&offer, None).unwrap();
    let res = FakePlatform::default()
        .delete_checked(&fx.a, &t, DeleteOutcome::Cancelled)
        .unwrap();
    let out = fx.a.finish_purge(t, &res).unwrap();
    assert!(out.purged.is_empty());
    assert_eq!(out.kept_trashed.len(), 3);
    assert_eq!(fx.states(&ids), vec![State::Trashed; 3]);
    assert_eq!(
        fx.sql_i64("SELECT COUNT(*) FROM action_log WHERE op='purge'"),
        0
    );
}

/// T10: delvis (7 av 10 slettet). Rapporten viser 7/3 og «frigjort» teller bare de 7.
#[test]
fn t10_partial_platform_result() {
    let mut fx = Fx::new();
    let ids = fx.assets(10);
    fx.trash(&ids);
    let offer = fx.a.begin_purge(&ids).unwrap();
    let t = fx.a.confirm_purge(&offer, None).unwrap();
    let results: Vec<_> = ids
        .iter()
        .enumerate()
        .map(|(i, &id)| {
            (
                id,
                if i < 7 {
                    DeleteOutcome::Deleted
                } else {
                    DeleteOutcome::Failed
                },
            )
        })
        .collect();
    let out = fx.a.finish_purge(t, &results).unwrap();
    assert_eq!(
        (out.purged.len(), out.kept_trashed.len(), out.freed_bytes),
        (7, 3, 21)
    );
    assert_eq!(fx.sql_i64("SELECT COUNT(*) FROM asset WHERE state=2"), 7);
    assert_eq!(fx.sql_i64("SELECT COUNT(*) FROM asset WHERE state=1"), 3);
    let s = fx.a.trash_summary().unwrap();
    assert_eq!(
        (s.trashed_count, s.trashed_bytes, s.freed_bytes),
        (3, 9, 21)
    );
}
