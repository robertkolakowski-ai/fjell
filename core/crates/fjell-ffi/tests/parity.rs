//! Paritetstest: hver metode på `FjellApi` må enten være speilet i `fjell-ffi` eller stå i listen over
//! bevisst ikke-speilede metoder med begrunnelse. Feiler hvis kjernen får en ny metode uten at speilet
//! (eller unntakslisten) er oppdatert, og hvis listene peker på noe som ikke finnes.

use fjell_ffi::{MIRRORED_API, NOT_MIRRORED_API};
use std::collections::BTreeSet;

const API_SRC: &str = include_str!("../../fjell-core/src/api/mod.rs");
const FFI_LIB_SRC: &str = include_str!("../src/lib.rs");

fn ident_after(line: &str, prefix: &str) -> Option<String> {
    let rest = line.strip_prefix(prefix)?;
    let name: String = rest
        .chars()
        .take_while(|c| c.is_alphanumeric() || *c == '_')
        .collect();
    (!name.is_empty()).then_some(name)
}

/// Metodenavn i `pub trait FjellApi { ... }` (innrykk på fire mellomrom, kun signaturer).
fn trait_methods(src: &str) -> BTreeSet<String> {
    let start = src
        .find("pub trait FjellApi")
        .expect("fant ikke trait FjellApi");
    let body = &src[start..];
    let end = body.find("\n}\n").expect("fant ikke slutten på traiten");
    body[..end]
        .lines()
        .filter_map(|l| ident_after(l, "    fn "))
        .collect()
}

/// Offentlige metoder i omslaget (`lib.rs`).
fn ffi_methods(src: &str) -> BTreeSet<String> {
    src.lines()
        .filter_map(|l| ident_after(l.trim_start(), "pub fn "))
        .collect()
}

#[test]
fn parser_finds_known_methods() {
    let m = trait_methods(API_SRC);
    for n in ["start_scan", "protect_set", "thresholds", "dashboard_full"] {
        assert!(m.contains(n), "parseren mistet {n}");
    }
    assert!(m.len() >= 35, "uventet få metoder: {}", m.len());
}

#[test]
fn every_core_method_is_mirrored_or_explicitly_excluded() {
    let core = trait_methods(API_SRC);
    let mirrored: BTreeSet<String> = MIRRORED_API.iter().map(|s| s.to_string()).collect();
    let excluded: BTreeSet<String> = NOT_MIRRORED_API
        .iter()
        .map(|(n, _)| n.to_string())
        .collect();
    let missing: Vec<_> = core
        .iter()
        .filter(|n| !mirrored.contains(*n) && !excluded.contains(*n))
        .collect();
    assert!(
        missing.is_empty(),
        "FjellApi har metoder som verken er speilet i fjell-ffi eller bevisst utelatt: {missing:?}. \
         Legg dem til i `Fjell` (lib.rs) og `MIRRORED_API`, eller i `NOT_MIRRORED_API` med begrunnelse."
    );
}

#[test]
fn lists_do_not_contain_stale_or_overlapping_entries() {
    let core = trait_methods(API_SRC);
    for n in MIRRORED_API {
        assert!(
            core.contains(*n),
            "{n} står i MIRRORED_API men finnes ikke på FjellApi"
        );
    }
    for (n, why) in NOT_MIRRORED_API {
        assert!(
            core.contains(*n),
            "{n} står i NOT_MIRRORED_API men finnes ikke på FjellApi"
        );
        assert!(!why.trim().is_empty(), "{n} mangler begrunnelse");
        assert!(!MIRRORED_API.contains(n), "{n} er både speilet og utelatt");
    }
    let unique: BTreeSet<_> = MIRRORED_API.iter().collect();
    assert_eq!(
        unique.len(),
        MIRRORED_API.len(),
        "duplikater i MIRRORED_API"
    );
}

#[test]
fn every_mirrored_method_exists_on_the_ffi_object() {
    let ffi = ffi_methods(FFI_LIB_SRC);
    for n in MIRRORED_API {
        assert!(
            ffi.contains(*n),
            "{n} er oppgitt som speilet men finnes ikke i lib.rs"
        );
    }
}

#[test]
fn whole_v1_surface_is_mirrored() {
    for n in [
        "protect_set",
        "unprotect",
        "protections",
        "rules_list",
        "rule_save",
        "rule_preview",
        "rule_toggle",
        "rule_delete",
        "rule_restore",
        "people_list",
        "person_name",
        "person_merge",
        "person_unmerge",
        "person_protect",
        "face_index_enabled",
        "face_index_set",
        "face_index_wipe",
        "dashboard",
        "dashboard_full",
        "series_compare",
        "thresholds",
        "set_keeper",
        "settings",
        "set_settings",
    ] {
        assert!(MIRRORED_API.contains(&n), "V1-metoden {n} må være speilet");
    }
}
