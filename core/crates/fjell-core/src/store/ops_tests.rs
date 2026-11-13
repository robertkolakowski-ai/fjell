//! Tester for `store::ops`: atomisitet, angre-markører, avstemming og skjemakrav (I4, I13, I14, I16).

use super::*;
use crate::model::{AssetId, SourceId, State};
use rusqlite::params;

fn store_with(n: i64) -> Store {
    let s = Store::open_in_memory().unwrap();
    for i in 1..=n {
        s.conn()
            .execute(
                "INSERT INTO asset(id, source_id, kind, created_at, mod_token, width, height, bytes, mime) \
                 VALUES (?1, ?2, 'photo', 1, 'm', 1, 1, 10, 'x')",
                params![i, format!("s{i}")],
            )
            .unwrap();
    }
    s
}

fn trash_spec(ids: &[u64]) -> BatchSpec {
    BatchSpec {
        ts: 100,
        cause: Cause::User,
        ops: ids
            .iter()
            .map(|&i| RowOp::Trash {
                id: AssetId(i),
                kind: None,
                expires_at: 999,
            })
            .collect(),
    }
}

#[test]
fn empty_batch_writes_nothing() {
    let mut s = store_with(2);
    assert_eq!(s.apply_batch(&trash_spec(&[])).unwrap(), None);
    assert_eq!(
        s.conn()
            .query_row("SELECT COUNT(*) FROM action_log", [], |r| r
                .get::<_, i64>(0))
            .unwrap(),
        0
    );
}

#[test]
fn wrong_state_rolls_back_whole_batch() {
    let mut s = store_with(3);
    s.apply_batch(&trash_spec(&[2])).unwrap().unwrap();
    // Id 2 er allerede trashed: hele batchen (også id 1) rulles tilbake.
    assert!(matches!(
        s.apply_batch(&trash_spec(&[1, 2])),
        Err(StoreError::Conflict(AssetId(2)))
    ));
    assert_eq!(
        s.asset_row(AssetId(1)).unwrap().unwrap().state,
        State::Active
    );
}

#[test]
fn undo_marks_batch_undone_even_when_every_row_is_skipped() {
    let mut s = store_with(2);
    s.apply_batch(&trash_spec(&[1])).unwrap().unwrap();
    s.reconcile_removed(&[SourceId("s1".into())], 200).unwrap();
    match s.undo_top(300, 1000).unwrap() {
        UndoResult::Done {
            applied, skipped, ..
        } => assert_eq!((applied, skipped), (0, vec![AssetId(1)])),
        other => panic!("{other:?}"),
    }
    assert_eq!(s.undo_top(301, 1000).unwrap(), UndoResult::Nothing);
}

#[test]
fn external_reconcile_batches_are_not_on_the_undo_stack() {
    let mut s = store_with(2);
    s.apply_batch(&trash_spec(&[1])).unwrap().unwrap();
    s.reconcile_removed(&[SourceId("s2".into())], 200).unwrap();
    assert!(matches!(
        s.undo_top(300, 1000).unwrap(),
        UndoResult::Done { applied: 1, .. }
    ));
}

/// I14: `action_log` kan ikke endres via noen av operasjonene (triggere gir feil).
#[test]
fn i14_action_log_rejects_update_and_delete() {
    let mut s = store_with(1);
    s.apply_batch(&trash_spec(&[1])).unwrap().unwrap();
    assert!(s
        .conn()
        .execute("UPDATE action_log SET ts = 0", [])
        .is_err());
    assert!(s.conn().execute("DELETE FROM action_log", []).is_err());
}

/// I13 (delvis): avstemming rører ikke andre trashed bilder. `upsert_assets` (M2) finnes ikke ennå;
/// testen for at den bevarer `state`/`trashed_at`/`expires_at` kommer med M2.
#[test]
fn i13_reconcile_only_touches_removed_sources() {
    let mut s = store_with(3);
    s.apply_batch(&trash_spec(&[1, 2])).unwrap().unwrap();
    let before = s.asset_row(AssetId(1)).unwrap().unwrap();
    s.reconcile_removed(&[SourceId("s2".into())], 200).unwrap();
    assert_eq!(s.asset_row(AssetId(1)).unwrap().unwrap(), before);
    assert_eq!(
        s.asset_row(AssetId(3)).unwrap().unwrap().state,
        State::Active
    );
}

/// I16: bare kjente kolonner, og BLOB bare for `content_hash`. Ingen bilde- eller fritekstkolonner.
#[test]
fn i16_schema_has_no_image_data_or_free_text() {
    let s = Store::open_in_memory().unwrap();
    let tables: Vec<String> = s
        .conn()
        .prepare("SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%'")
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    // (tabell.kolonne) som lagrer tekst: id/sti, kategorier og albumnavn, ikke brukertekst.
    let text_ok = [
        "asset.source_id",
        "asset.kind",
        "asset.mod_token",
        "asset.mime",
        "grp.kind",
        "grp.keeper_reason",
        "suggestion.kind",
        "suggestion.reason_code",
        "suggestion.status",
        "action_log.op",
        "action_log.prev_sugg_status",
        "action_log.cause",
        "setting.key",
        "setting.value",
        "album.name",
    ];
    for t in tables {
        let mut st = s
            .conn()
            .prepare(&format!("PRAGMA table_info({t})"))
            .unwrap();
        let cols: Vec<(String, String)> = st
            .query_map([], |r| {
                Ok((r.get(1)?, r.get::<_, String>(2)?.to_uppercase()))
            })
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        for (c, ty) in cols {
            let key = format!("{t}.{c}");
            match ty.as_str() {
                "BLOB" => assert_eq!(key, "fingerprint.content_hash"),
                "TEXT" => assert!(
                    text_ok.contains(&key.as_str()),
                    "uventet tekstkolonne {key}"
                ),
                _ => {}
            }
        }
    }
}

/// Avstemming logger hvert tilstandsskifte: aktiv gir `trash` + `purge`, trashed gir bare `purge`,
/// begge `cause=external`, og ingen rader slettes.
#[test]
fn reconcile_logs_every_transition_and_deletes_nothing() {
    let mut s = store_with(3);
    s.apply_batch(&trash_spec(&[2])).unwrap().unwrap();
    s.reconcile_removed(&[SourceId("s1".into()), SourceId("s2".into())], 200)
        .unwrap();
    let rows: Vec<(i64, String, i64, String)> = s
        .conn()
        .prepare(
            "SELECT asset_id, op, prev_state, cause FROM action_log WHERE ts = 200 ORDER BY id",
        )
        .unwrap()
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))
        .unwrap()
        .map(|r| r.unwrap())
        .collect();
    let e = |a: i64, op: &str, p: i64| (a, op.to_string(), p, "external".to_string());
    assert_eq!(
        rows,
        vec![e(1, "trash", 0), e(1, "purge", 1), e(2, "purge", 1)]
    );
    let n: i64 = s
        .conn()
        .query_row("SELECT COUNT(*) FROM asset", [], |r| r.get(0))
        .unwrap();
    assert_eq!(n, 3);
}

/// F1: `relocate_source` flytter bare pekeren; opptatt sti gir feil og ingen endring.
#[test]
fn relocate_source_moves_only_the_pointer_and_refuses_an_occupied_path() {
    let mut s = store_with(2);
    let to = SourceId("s1 (gjenopprettet)".into());
    assert!(s.relocate_source(AssetId(1), &to).unwrap());
    assert!(!s.relocate_source(AssetId(1), &to).unwrap(), "idempotent");
    assert!(!s.relocate_source(AssetId(9), &to).is_ok_and(|moved| moved));
    assert!(s
        .relocate_source(AssetId(1), &SourceId("s2".into()))
        .is_err());
    let ids = s.source_ids(&[AssetId(1), AssetId(2)]).unwrap();
    assert_eq!(ids[0].1, to);
    assert_eq!(ids[1].1, SourceId("s2".into()));
    assert_eq!(
        s.asset_rows(&[AssetId(1)]).unwrap()[0]
            .as_ref()
            .unwrap()
            .state,
        State::Active
    );
}
