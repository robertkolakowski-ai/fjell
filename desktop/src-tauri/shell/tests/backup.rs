//! Sikkerhetskopi av indeksen og integritet gjennom skallets kommandolag (kontraktfasen, docs/v1/ROBUSTHET-KONTRAKT.md).
//! Grenen «backup» eier denne filen og utvider den med ekte kjerne. Nå testes bare kontrakten mot mocken:
//! standardsvar, JSON-form og at gjenoppretting aldri går uten avkrysning.
//! Kjørt på Linux; ikke verifisert på Windows.

use fjell_shell::Shell;
use serde_json::json;

#[test]
fn mock_defaults_are_empty_and_integrity_is_ok() {
    let shell = Shell::mock();
    assert!(shell.backups().unwrap().is_empty());
    assert_eq!(shell.integrity().unwrap(), "ok");
    let e = shell.backup_create().unwrap_err();
    assert_eq!(
        (e.code, e.detail.as_str()),
        ("core_error", "not_implemented")
    );
}

#[test]
fn restore_without_acknowledgement_is_refused_before_anything_happens() {
    let shell = Shell::mock();
    let e = shell.backup_restore(1, false).unwrap_err();
    assert_eq!(
        (e.code, e.detail.as_str()),
        ("core_error", "backup_ack_required")
    );
}

#[test]
fn integrity_without_library_is_ok() {
    // Uten bibliotek finnes ingen indeks å sjekke; frontend skal ikke gå i trygg modus av den grunn.
    assert_eq!(Shell::mock().integrity().unwrap(), "ok");
}

#[test]
fn json_shape_is_camel_case_codes_only() {
    let b = fjell_shell::dto_robust::BackupDto {
        id: 2,
        kind: "pre_restore",
        created_at: 1_700_000_000,
        bytes: 4096,
        asset_count: 10,
        integrity: "ok",
    };
    assert_eq!(
        serde_json::to_value(&b).unwrap(),
        json!({ "id": 2, "kind": "pre_restore", "createdAt": 1_700_000_000, "bytes": 4096, "assetCount": 10, "integrity": "ok" })
    );
}
