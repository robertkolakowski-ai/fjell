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
    let text = j["text"].as_str().unwrap();
    assert!(!text.contains('/') && !text.contains('\\') && !text.contains(':'));
    assert!(text.is_ascii());
    assert!(text.lines().all(|l| l.split_once('=').is_some()));
    for key in [
        "app_version=",
        "core_version=",
        "os=",
        "os_version=",
        "scale_percent=",
        "log_entries=",
    ] {
        assert!(text.contains(key), "mangler {key}");
    }
    // Ingen mappenavn som standard.
    assert!(!text.contains("folder."));
}

#[test]
fn export_needs_the_app_data_folder() {
    // Mock-skallet har ingen datamappe: eksporten feiler med kode i stedet for å skrive et annet sted.
    let e = Shell::mock().diag_export().unwrap_err();
    assert_eq!((e.code, e.detail.as_str()), ("core_error", "diag_failed"));
}

#[test]
fn export_saves_exactly_the_preview_in_the_diagnostics_folder() {
    let dir = std::env::temp_dir().join(format!("fjell-diag-it-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let sh = Shell::with_data_dir(&dir, std::sync::Arc::new(fjell_shell::NoRecycleBin));
    let preview = sh.diag_preview().unwrap();
    let exported = sh.diag_export().unwrap();
    assert_eq!(preview.text, exported.text);
    let files: Vec<_> = std::fs::read_dir(dir.join("diagnose"))
        .unwrap()
        .flatten()
        .collect();
    assert_eq!(files.len(), 1);
    assert_eq!(
        std::fs::read_to_string(files[0].path()).unwrap(),
        preview.text
    );
    let _ = std::fs::remove_dir_all(dir);
}
