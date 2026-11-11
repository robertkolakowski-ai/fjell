//! Scenariene T11-T20 fra PLAN-TRYGGHET del 6 (kjernenivå; T21-T22 er UI, T23 og
//! fjell-cli-delene av T11/T12/T15 hører til integrasjonsfasen).

use super::*;
use crate::model::{Acks, ConsequenceCode, SourceId};

/// T11: app drept etter plattformsletting, før DB. Avstemming gir `purged` med `cause=external`.
#[test]
fn t11_crash_after_platform_delete_reconciles_as_external() {
    let mut fx = Fx::new();
    let ids = fx.assets(3);
    fx.trash(&ids);
    let offer = fx.a.begin_purge(&ids).unwrap();
    let t = fx.a.confirm_purge(&offer, None).unwrap();
    let plat = FakePlatform::default();
    plat.delete_checked(&fx.a, &t, DeleteOutcome::Deleted)
        .unwrap();
    // Krasj: `finish_purge` kjøres aldri. Neste skanning melder kildene som borte.
    assert_eq!(fx.states(&ids), vec![State::Trashed; 3]);
    let gone = plat.gone.lock().unwrap().clone();
    let purged =
        fx.a.store_mut()
            .reconcile_removed(&gone, 2_000_000)
            .unwrap();
    assert_eq!(purged.len(), 3);
    assert_eq!(fx.states(&ids), vec![State::Purged; 3]);
    assert_eq!(
        fx.sql_i64("SELECT COUNT(*) FROM action_log WHERE op='purge' AND cause='external'"),
        3
    );
    // Ikke regnet som «frigjort av Fjell».
    assert_eq!(fx.a.trash_summary().unwrap().freed_bytes, 0);
    // Reconcile er idempotent.
    assert!(fx
        .a
        .store_mut()
        .reconcile_removed(&gone, 2_000_001)
        .unwrap()
        .is_empty());
}

/// T12: bilde slettet utenfor Fjell mens `trashed`. Angre hopper over det og rapporterer.
#[test]
fn t12_external_delete_while_trashed_then_undo_skips() {
    let mut fx = Fx::new();
    let ids = fx.assets(3);
    fx.trash(&ids);
    fx.a.store_mut()
        .reconcile_removed(&[SourceId("s2".into())], 1_000_100)
        .unwrap();
    assert_eq!(fx.state(ids[1]), State::Purged);
    let u = fx.a.undo().unwrap();
    assert_eq!((u.restored, u.skipped.clone()), (2, vec![ids[1]]));
    assert_eq!(fx.state(ids[1]), State::Purged);
    // Aktivt bilde som forsvinner går via trashed -> purged, uten å bryte triggeren.
    let extra = fx.asset(9, 3, 0, None);
    fx.a.store_mut()
        .reconcile_removed(&[SourceId("s9".into())], 1_000_200)
        .unwrap();
    assert_eq!(fx.state(extra), State::Purged);
}

/// T13: angre av batch som inneholder purge gir `NotUndoable`; ingenting endres.
#[test]
fn t13_undo_of_purge_batch_is_not_undoable() {
    let mut fx = Fx::new();
    let ids = fx.assets(2);
    fx.trash(&ids);
    fx.purge_all(&ids[..1], &FakePlatform::default());
    let (logs, sum) = (fx.log_count(), dump_sum(&fx.a));
    assert!(matches!(fx.a.undo(), Err(ActionError::NotUndoable)));
    assert!(matches!(fx.a.undo(), Err(ActionError::NotUndoable)));
    assert_eq!((fx.log_count(), dump_sum(&fx.a)), (logs, sum));
    assert_eq!(fx.state(ids[1]), State::Trashed);
}

/// T14: angre når tilstanden er endret (restore gjort manuelt) hopper over id-en.
#[test]
fn t14_undo_skips_ids_with_deviating_state() {
    let mut fx = Fx::new();
    let ids = fx.assets(3);
    fx.trash(&ids);
    fx.a.restore(&ids[1..2]).unwrap().unwrap();
    // Siste batch er restoren; angre den (re-trasher), så angre trash-batchen.
    assert_eq!(fx.a.undo().unwrap().restored, 1);
    fx.a.restore(&ids[1..2]).unwrap().unwrap();
    fx.a.undo().unwrap();
    // Nå manuelt restore uten batch-stabel: simuler ved direkte restore av id 2 utenom angre.
    fx.a.store()
        .conn()
        .execute(
            "UPDATE asset SET state = 0, trashed_at = NULL, expires_at = NULL WHERE id = 2",
            [],
        )
        .unwrap();
    let u = fx.a.undo().unwrap();
    assert_eq!((u.restored, u.skipped), (2, vec![ids[1]]));
    assert_eq!(fx.states(&ids), vec![State::Active; 3]);
    assert!(matches!(fx.a.undo(), Err(ActionError::NothingToUndo)));
}

/// T15 (kjernenivå): avstemming av andre kilder rører ikke trashed bilder, og trashed bilder
/// dukker ikke opp som forslag/kort.
#[test]
fn t15_reconcile_leaves_trashed_untouched_and_hidden_from_suggestions() {
    let mut fx = Fx::new();
    let ids = fx.assets(3);
    for id in &ids {
        fx.suggest(*id, "blurry", None);
    }
    fx.trash(&ids[..2]);
    let before = fx.a.store().asset_row(ids[0]).unwrap().unwrap();
    fx.a.store_mut()
        .reconcile_removed(
            &[SourceId("s3".into()), SourceId("ukjent".into())],
            1_000_500,
        )
        .unwrap();
    assert_eq!(fx.a.store().asset_row(ids[0]).unwrap().unwrap(), before);
    assert_eq!(
        fx.states(&ids),
        vec![State::Trashed, State::Trashed, State::Purged]
    );
    assert!(fx.a.store().pending_suggestions().unwrap().is_empty());
    assert!(fx.a.plan_pending().unwrap().items.is_empty());
}

/// T16: billett gjenbrukt, utløpt eller mengde endret gir `PurgeStale` og ingen `delete`.
#[test]
fn t16_ticket_reuse_expiry_and_changed_set() {
    let mut fx = Fx::new();
    let ids = fx.assets(3);
    fx.trash(&ids);
    let plat = FakePlatform::default();
    // Gjenbruk.
    let offer = fx.a.begin_purge(&ids[..1]).unwrap();
    let t = fx.a.confirm_purge(&offer, None).unwrap();
    let res = plat
        .delete_checked(&fx.a, &t, DeleteOutcome::Deleted)
        .unwrap();
    fx.a.finish_purge(t.clone(), &res).unwrap();
    assert_eq!(plat.call_count(), 1);
    assert!(matches!(
        plat.delete_checked(&fx.a, &t, DeleteOutcome::Deleted),
        Err(ActionError::PurgeStale)
    ));
    assert!(matches!(
        fx.a.finish_purge(t, &res),
        Err(ActionError::PurgeStale)
    ));
    // Utløpt (> 10 min).
    let offer = fx.a.begin_purge(&ids[1..2]).unwrap();
    let t = fx.a.confirm_purge(&offer, None).unwrap();
    fx.clock.advance(601);
    assert!(matches!(
        plat.delete_checked(&fx.a, &t, DeleteOutcome::Deleted),
        Err(ActionError::PurgeStale)
    ));
    assert!(matches!(
        fx.a.finish_purge(t, &[]),
        Err(ActionError::PurgeStale)
    ));
    // Mengde endret etter bekreftelse (bildet gjenopprettet).
    let offer = fx.a.begin_purge(&ids[2..]).unwrap();
    let t = fx.a.confirm_purge(&offer, None).unwrap();
    fx.a.restore(&ids[2..]).unwrap();
    assert!(matches!(
        plat.delete_checked(&fx.a, &t, DeleteOutcome::Deleted),
        Err(ActionError::PurgeStale)
    ));
    assert!(matches!(
        fx.a.confirm_purge(&offer, None),
        Err(ActionError::PurgeStale)
    ));
    assert_eq!(plat.call_count(), 1, "ingen delete på ugyldig billett");
    assert_eq!(
        fx.states(&ids),
        vec![State::Purged, State::Trashed, State::Active]
    );
}

/// T17: injisert feil midt i batch gir ingen delvis batch (også gjentatt per feilposisjon).
#[test]
fn t17_injected_failure_mid_batch_leaves_nothing() {
    for fail_at in 1..=5u64 {
        let mut fx = Fx::new();
        let ids = fx.assets(5);
        let plan = fx.a.plan(&PlanRequest::manual(&ids)).unwrap();
        fx.a.store()
            .conn()
            .execute_batch(&format!(
            "CREATE TEMP TRIGGER inject BEFORE INSERT ON action_log WHEN NEW.asset_id = {fail_at} \
             BEGIN SELECT RAISE(ABORT, 'injisert'); END;"
        ))
            .unwrap();
        let sum = dump_sum(&fx.a);
        assert!(matches!(
            fx.a.commit(&plan, &Acks::default()),
            Err(ActionError::Store(_))
        ));
        assert_eq!(
            dump_sum(&fx.a),
            sum,
            "feil på rad {fail_at} etterlot delvis batch"
        );
        fx.a.store()
            .conn()
            .execute_batch("DROP TRIGGER inject")
            .unwrap();
        assert!(fx.a.commit(&plan, &Acks::default()).unwrap().is_some());
    }
}

/// T18: 50 000 id-er i én batch (over SQLites variabelgrense), fortsatt én transaksjon.
#[test]
fn t18_fifty_thousand_ids_in_one_batch() {
    let mut fx = Fx::new();
    fx.a.store().conn().execute_batch("BEGIN").unwrap();
    for i in 1..=50_000i64 {
        fx.a.store().conn().execute(
            "INSERT INTO asset(id, source_id, kind, created_at, mod_token, width, height, bytes, mime) \
             VALUES (?1, ?2, 'photo', 1, 'm', 1, 1, 1, 'x')", rusqlite::params![i, format!("s{i}")]).unwrap();
    }
    fx.a.store().conn().execute_batch("COMMIT").unwrap();
    let ids: Vec<_> = (1..=50_000).map(AssetId).collect();
    let plan = fx.a.plan(&PlanRequest::manual(&ids)).unwrap();
    assert_eq!(plan.items.len(), 50_000);
    let b = fx.a.commit(&plan, &acks_covering(&plan)).unwrap().unwrap();
    assert_eq!(
        fx.sql_i64("SELECT COUNT(DISTINCT batch_id) FROM action_log"),
        1
    );
    assert_eq!(
        fx.sql_i64(&format!(
            "SELECT COUNT(*) FROM action_log WHERE batch_id = {}",
            b.0
        )),
        50_000
    );
    assert_eq!(
        fx.sql_i64("SELECT COUNT(*) FROM asset WHERE state = 1"),
        50_000
    );
    let u = fx.a.undo().unwrap();
    assert_eq!(u.restored, 50_000);
    assert_eq!(
        fx.sql_i64("SELECT COUNT(*) FROM asset WHERE state = 0"),
        50_000
    );
}

/// T19: tom mengde, duplikater og ukjent id.
#[test]
fn t19_empty_duplicates_and_unknown() {
    let mut fx = Fx::new();
    let ids = fx.assets(3);
    let empty = fx.a.plan(&PlanRequest::default()).unwrap();
    assert_eq!(fx.a.commit(&empty, &Acks::default()).unwrap(), None);
    assert_eq!(fx.log_count(), 0);
    let dup =
        fx.a.plan(&PlanRequest::manual(&[ids[0], ids[0], ids[1], ids[0]]))
            .unwrap();
    assert_eq!(dup.ids(), vec![ids[0], ids[1]]);
    fx.a.commit(&dup, &Acks::default()).unwrap().unwrap();
    assert_eq!(fx.log_count(), 2);
    assert!(matches!(
        fx.a.plan(&PlanRequest::manual(&[AssetId(99)])),
        Err(ActionError::UnknownAsset(AssetId(99)))
    ));
    assert!(matches!(
        fx.a.restore(&[AssetId(99)]),
        Err(ActionError::UnknownAsset(_))
    ));
    assert!(matches!(
        fx.a.begin_purge(&[]),
        Err(ActionError::EmptySelection)
    ));
    assert!(matches!(
        fx.a.begin_purge(&[ids[2]]),
        Err(ActionError::NotTrashed(_))
    ));
    fx.purge_all(&ids[..2], &FakePlatform::default());
    assert!(matches!(
        fx.a.plan(&PlanRequest::manual(&[ids[0]])),
        Err(ActionError::AlreadyPurged(_))
    ));
    assert!(matches!(
        fx.a.begin_purge(&[ids[0]]),
        Err(ActionError::AlreadyPurged(_))
    ));
    assert!(matches!(
        fx.a.restore(&[ids[0]]),
        Err(ActionError::AlreadyPurged(_))
    ));
}

/// T20 (kontrakt i kjernen): alle koder har unike snake_case-nøkler. At nøklene finnes i
/// `i18n/nb.json` og `en.json` sjekkes av i18n-planen (nøklene `consequence.*` og `error.*`).
#[test]
fn t20_code_keys_are_unique_snake_case() {
    let mut keys: Vec<&str> = ConsequenceCode::ALL.iter().map(|c| c.key()).collect();
    assert!(ConsequenceCode::ALL.contains(&ConsequenceCode::PlatformDialogNext));
    let errs = [
        ActionError::AckRequired(vec![]),
        ActionError::PlanStale,
        ActionError::PurgeStale,
        ActionError::NothingToUndo,
        ActionError::NotUndoable,
        ActionError::AlreadyPurged(AssetId(1)),
        ActionError::UnknownAsset(AssetId(1)),
        ActionError::NotTrashed(AssetId(1)),
        ActionError::WouldEmptyGroup(crate::model::GroupId(1)),
        ActionError::EmptySelection,
    ];
    keys.extend(errs.iter().map(|e| e.key()));
    assert!(keys
        .iter()
        .all(|k| k.chars().all(|c| c.is_ascii_lowercase() || c == '_')));
    let n = keys.len();
    keys.sort();
    keys.dedup();
    assert_eq!(keys.len(), n);
}
