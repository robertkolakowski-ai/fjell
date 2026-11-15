//! Indeksering (status, pause, innstillinger) gjennom skallets kommandolag (kontraktfasen, docs/v1/ROBUSTHET-KONTRAKT.md).
//! Grenen «indeksering» eier denne filen og utvider den med ekte kjerne. Nå testes bare kontrakten mot mocken.
//! Kjørt på Linux; strømstatus og trådprioritet på Windows er ikke verifisert.

use fjell_shell::dto_robust::IndexingSettingsDto;
use fjell_shell::Shell;
use serde_json::json;

#[test]
fn mock_status_is_idle_and_settings_are_the_defaults() {
    let shell = Shell::mock();
    let s = shell.indexing_status().unwrap();
    assert_eq!(
        serde_json::to_value(&s).unwrap(),
        json!({ "state": "idle", "phase": "idle", "done": 0, "total": 0, "ratePerSec": 0, "etaSecs": null })
    );
    assert_eq!(
        serde_json::to_value(shell.indexing_settings().unwrap()).unwrap(),
        json!({ "threads": 0, "priority": "low", "pauseOnBattery": true })
    );
}

#[test]
fn unknown_priority_is_refused_with_a_code() {
    let shell = Shell::mock();
    let bad = IndexingSettingsDto {
        threads: 2,
        priority: "turbo".into(),
        pause_on_battery: true,
    };
    let e = shell.set_indexing_settings(&bad).unwrap_err();
    assert_eq!(
        (e.code, e.detail.as_str()),
        ("core_error", "indexing_invalid_settings")
    );
}

#[test]
fn control_calls_are_not_implemented_until_the_branch_lands() {
    let shell = Shell::mock();
    for e in [
        shell.indexing_pause().unwrap_err(),
        shell.indexing_resume().unwrap_err(),
        shell.indexing_cancel().unwrap_err(),
    ] {
        assert_eq!(
            (e.code, e.detail.as_str()),
            ("core_error", "not_implemented")
        );
    }
}
