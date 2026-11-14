//! Tester for V1-kommandoene i skallet mot ekte kjerne (`Fjell` med tom `MemSource`, ingen WebView).
//! Viser at DTO-ene har kontrakten `desktop/src/bridge/types.ts` forventer (camelCase, koder), at
//! beskyttelse krever bekreftelse, at ansiktsindeksen er av som standard og at feil er koder.

use crate::dto::DecisionDto;
use crate::dto_v1::{ConditionDto, RuleDraftDto, ScopeDto, SettingsDto};
use crate::Shell;
use fjell_core::api::{Fjell, FjellApi, ScanConfig, ScanMode};
use fjell_core::model::{AssetMeta, FakeClock, Kind, MemSource, ProtectFlags, SourceId};
use fjell_core::store::Store;
use serde_json::json;
use std::sync::Arc;

fn meta(name: &str, mime: &str, bytes: u64) -> AssetMeta {
    AssetMeta {
        source_id: SourceId(name.into()),
        kind: Kind::Photo,
        created_at: 1_000,
        mod_token: "t1".into(),
        width: 0,
        height: 0,
        bytes,
        mime: mime.into(),
        platform_screenshot_hint: false,
        has_camera_exif: Some(true),
        is_local: true,
        protect_flags: ProtectFlags::default(),
        albums: vec![],
        backup: None,
    }
}

/// Tre filer: to identiske (eksakt duplikat) og én PNG.
fn shell() -> Shell {
    let src = MemSource::new();
    src.put(meta("a.jpg", "image/jpeg", 10), None, vec![7; 10]);
    src.put(meta("b.jpg", "image/jpeg", 10), None, vec![7; 10]);
    src.put(meta("c.png", "image/png", 14), None, vec![9; 14]);
    let f = Fjell::new(
        Store::open_in_memory().unwrap(),
        Arc::new(src),
        Arc::new(FakeClock::new(5_000_000)),
    );
    f.start_scan(ScanConfig {
        mode: ScanMode::Full,
        priority: 1,
        deadline: None,
    })
    .unwrap();
    f.wait_scan().unwrap().unwrap();
    Shell::new(Arc::new(f), "core")
}

fn asset_scope(id: u64) -> ScopeDto {
    ScopeDto {
        scope: "asset".into(),
        target: id.to_string(),
    }
}

#[test]
fn protect_requires_confirmation_and_dto_has_contract_shape() {
    let s = shell();
    let page = s.cards("exact_dup", 10, None).unwrap();
    assert_eq!(page.cards.len(), 1);
    let card = &page.cards[0];
    let j = serde_json::to_value(card).unwrap();
    assert_eq!(j["detail"], json!({ "kind": "none" }));

    s.protect(&asset_scope(card.asset), true).unwrap();
    let list = s.protections().unwrap();
    assert_eq!(
        serde_json::to_value(&list).unwrap(),
        json!([{ "scope": { "scope": "asset", "target": card.asset.to_string() }, "createdAt": 5_000_000 }])
    );
    // Uten bekreftelse: ack_required med koden UI må få bekreftet. Med bekreftelse går det gjennom.
    let e = s.decide(&[card.id], DecisionDto::Trash).unwrap_err();
    assert_eq!(
        (e.code, e.detail.as_str()),
        ("ack_required", "protected_by_scope")
    );
    assert_eq!(s.state().trash_count, 0);
    let b = s
        .decide_confirmed(
            &[card.id],
            DecisionDto::Trash,
            &["protected_by_scope".into()],
        )
        .unwrap();
    assert_eq!(s.state().trash_count, 1);
    assert_eq!(s.undo().unwrap(), b);
    // Fjern beskyttelse (konfigurasjon) og ukjente koder.
    s.protect(&asset_scope(card.asset), false).unwrap();
    assert!(s.protections().unwrap().is_empty());
    assert_eq!(
        s.protect(
            &ScopeDto {
                scope: "bogus".into(),
                target: "1".into()
            },
            true
        )
        .unwrap_err()
        .code,
        "bad_request"
    );
    assert_eq!(
        s.decide_confirmed(&[card.id], DecisionDto::Trash, &["nope".into()])
            .unwrap_err()
            .code,
        "bad_request"
    );
}

#[test]
fn rules_roundtrip_with_undo_and_error_codes() {
    let s = shell();
    let conds = vec![ConditionDto::Mime("image/png".into())];
    let p = s.preview_rule(&conds).unwrap();
    assert_eq!((p.hits, p.bytes), (1, 14));
    assert!(s.rules().unwrap().is_empty());
    let id = s
        .save_rule(&RuleDraftDto {
            id: None,
            name: "PNG".into(),
            conditions: conds,
        })
        .unwrap();
    let j = serde_json::to_value(s.rules().unwrap()).unwrap();
    assert_eq!(
        j,
        json!([{ "id": id, "name": "PNG", "enabled": true, "hits": 1,
                 "conditions": [{ "kind": "mime", "value": "image/png" }] }])
    );
    assert_eq!(s.cards("rule", 10, None).unwrap().cards.len(), 1);
    s.toggle_rule(id, false).unwrap();
    assert!(s.cards("rule", 10, None).unwrap().cards.is_empty());
    s.toggle_rule(id, true).unwrap();
    s.delete_rule(id).unwrap();
    assert!(s.rules().unwrap().is_empty());
    s.restore_rule(id).unwrap();
    assert_eq!(s.rules().unwrap().len(), 1);
    // Feil er koder, ikke tekst.
    let e = s
        .save_rule(&RuleDraftDto {
            id: None,
            name: "tom".into(),
            conditions: vec![],
        })
        .unwrap_err();
    assert_eq!((e.code, e.detail.as_str()), ("core_error", "empty_rule"));
    // Vilkår uten verdi serialiseres bare med `kind`.
    assert_eq!(
        serde_json::to_value(ConditionDto::IsScreenshot).unwrap(),
        json!({ "kind": "is_screenshot" })
    );
    let back: ConditionDto =
        serde_json::from_value(json!({ "kind": "older_than_days", "value": 90 })).unwrap();
    assert_eq!(back, ConditionDto::OlderThanDays(90));
}

#[test]
fn face_index_is_off_by_default_and_wipe_is_explicit() {
    let s = shell();
    assert!(!s.face_index());
    assert!(!s.settings().face_index);
    assert!(s.people().unwrap().is_empty());
    s.set_face_index(true).unwrap();
    assert!(s.face_index());
    assert_eq!(s.wipe_face_index().unwrap(), 0);
    assert_eq!(
        s.name_person(999, Some("x".into())).unwrap_err().detail,
        "unknown_person"
    );
    assert_eq!(s.merge_people(1, 2).unwrap_err().detail, "unknown_person");
}

#[test]
fn dashboard_series_settings_and_thresholds() {
    let s = shell();
    let d = serde_json::to_value(s.dashboard().unwrap()).unwrap();
    assert_eq!(d["activeCount"], 3);
    assert_eq!(d["freedBytes"], 0);
    assert_eq!(d["protectedCount"], 0);
    // fremdrift, neste steg og «tømt» per måned er med (ingenting tømt ennå)
    assert_eq!(d["progress"]["decided"], 0);
    assert_eq!(d["progress"]["total"], 1);
    assert_eq!(d["nextStep"]["kind"], "review");
    assert_eq!(d["nextStep"]["suggestionKind"], "exact_dup");
    assert!(d["freedByMonth"].as_array().unwrap().is_empty());
    assert!(d["perKind"]
        .as_array()
        .unwrap()
        .iter()
        .any(|k| k["kind"] == "exact_dup"));
    assert_eq!(s.compare_series(42).unwrap_err().detail, "unknown_series");

    let mut st = s.settings();
    assert_eq!(st.sensitivity.len(), 6);
    assert!(st.sensitivity.values().all(|v| v == "normal"));
    st.sensitivity.insert("series".into(), "low".into());
    st.trash_days = 14;
    let saved = s.set_settings(&st).unwrap();
    assert_eq!(saved, st);
    assert!(s.thresholds().series_gap_secs < 10);
    let mut bad: SettingsDto = st.clone();
    bad.sensitivity.insert("series".into(), "extreme".into());
    assert_eq!(s.set_settings(&bad).unwrap_err().code, "bad_request");
    bad.sensitivity.clear();
    bad.sensitivity.insert("nope".into(), "low".into());
    assert_eq!(s.set_settings(&bad).unwrap_err().code, "bad_request");
}

#[test]
fn mock_backend_answers_v1_with_codes_not_panics() {
    let m = Shell::mock();
    assert!(m.rules().unwrap().is_empty());
    assert!(m.people().unwrap().is_empty());
    assert!(!m.face_index());
    assert_eq!(m.dashboard().unwrap().active_count, 0);
    assert_eq!(m.wipe_face_index().unwrap_err().code, "core_error");
}

#[test]
fn set_keeper_overrides_suggestion_and_rejects_unknowns_with_codes() {
    let s = shell();
    let card = s.cards("exact_dup", 10, None).unwrap().cards.remove(0);
    let (group, old_keeper, other) = (card.group.unwrap(), card.keeper.unwrap(), card.asset);
    assert_ne!(old_keeper, other);
    s.set_keeper(group, other).unwrap();
    let now = s.cards("exact_dup", 10, None).unwrap().cards.remove(0);
    assert_eq!(
        now.keeper,
        Some(other),
        "brukerens valg overstyrer forslaget"
    );
    assert_eq!(
        now.asset, old_keeper,
        "forslaget flyttes til den gamle keeperen"
    );
    // angre ved å velge tilbake; ukjent bilde og ukjent gruppe gir feilkode, ingen skriving
    s.set_keeper(group, old_keeper).unwrap();
    assert_eq!(
        s.set_keeper(group, 9_999).unwrap_err().code,
        "unknown_asset"
    );
    assert_eq!(
        s.set_keeper(9_999, other).unwrap_err().code,
        "unknown_asset"
    );
    let back = s.cards("exact_dup", 10, None).unwrap().cards.remove(0);
    assert_eq!(back.keeper, Some(old_keeper));
}
