//! Diagnose gjennom skallets kommandolag (kontraktfasen, docs/v1/ROBUSTHET-KONTRAKT.md). Grenen «diagnose» eier denne
//! filen og utvider den med ekte kjerne. Nå testes bare kontrakten mot mocken: rapporten har bare koder og tall,
//! og ingenting i den ligner en sti.
//! Kjørt på Linux.

use fjell_shell::Shell;
use serde_json::json;

#[test]
fn preview_is_codes_and_numbers_only() {
    let r = Shell::mock().diag_preview().unwrap();
    let j = serde_json::to_value(&r).unwrap();
    assert_eq!(j["format"], json!(1));
    assert_eq!(j["integrity"], json!("ok"));
    assert_eq!(j["indexing"], json!("idle"));
    assert_eq!(j["metrics"], json!([]));
    let text = j["text"].as_str().unwrap();
    assert!(!text.contains('/') && !text.contains('\\') && !text.contains(':'));
    assert!(text.lines().all(|l| l.split_once('=').is_some()));
}

#[test]
fn export_is_not_implemented_until_the_branch_lands() {
    let e = Shell::mock().diag_export().unwrap_err();
    assert_eq!(
        (e.code, e.detail.as_str()),
        ("core_error", "not_implemented")
    );
}
