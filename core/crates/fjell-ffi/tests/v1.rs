//! V1-flaten gjennom UniFFI-omslaget: beskyttelse, regler, personer, ansiktsindeks, dashboard, serie,
//! terskler og innstillinger. Omslaget skal bare delegere og konvertere; kjernens regler gjelder uendret.

use fjell_ffi::*;
use std::sync::Arc;

struct Empty;

impl AssetSourceCallback for Empty {
    fn list_changes(&self, _since: Option<String>) -> Result<ChangeSet, SourceFailure> {
        Ok(ChangeSet {
            changed: vec![],
            removed: vec![],
            next_token: "0".into(),
        })
    }
    fn thumbnail(&self, _id: String) -> Result<Thumb, SourceFailure> {
        Err(SourceFailure::Unavailable)
    }
    fn read_chunk(&self, _: String, _: u64, _: u32) -> Result<Vec<u8>, SourceFailure> {
        Err(SourceFailure::NotFound)
    }
}

/// Plattform med ett bilde (i album 3, under mappen `DCIM/Reise`), så beskyttelsesmål finnes i indeksen.
struct One;

impl AssetSourceCallback for One {
    fn list_changes(&self, since: Option<String>) -> Result<ChangeSet, SourceFailure> {
        let changed = if since.is_some() {
            vec![]
        } else {
            vec![AssetMeta {
                source_id: "DCIM/Reise/a.jpg".into(),
                kind: AssetKind::Photo,
                created_at: 1_000,
                mod_token: "1".into(),
                width: 100,
                height: 100,
                bytes: 5,
                mime: "image/jpeg".into(),
                platform_screenshot_hint: false,
                has_camera_exif: Some(true),
                is_local: true,
                protect_flags: 0,
                albums: vec![Album {
                    id: 3,
                    name: "Ferie".into(),
                    shared: false,
                }],
                backup: None,
            }]
        };
        Ok(ChangeSet {
            changed,
            removed: vec![],
            next_token: "1".into(),
        })
    }
    fn thumbnail(&self, _id: String) -> Result<Thumb, SourceFailure> {
        Err(SourceFailure::Unavailable)
    }
    fn read_chunk(&self, _: String, offset: u64, max: u32) -> Result<Vec<u8>, SourceFailure> {
        let all = b"bytes";
        let s = (offset as usize).min(all.len());
        let e = (s + max as usize).min(all.len());
        Ok(all[s..e].to_vec())
    }
}

fn open_one(tag: &str) -> (Arc<Fjell>, std::path::PathBuf) {
    let dir = std::env::temp_dir().join(format!("fjell-ffi-v1-{tag}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let db = dir.join("idx.sqlite").to_string_lossy().into_owned();
    let f = Fjell::open(db, Arc::new(One)).unwrap();
    f.start_scan(ScanConfig {
        mode: ScanMode::Quick,
        priority: 1,
        deadline: None,
    })
    .unwrap();
    f.wait_scan().unwrap();
    (f, dir)
}

fn open(tag: &str) -> (Arc<Fjell>, std::path::PathBuf) {
    let dir = std::env::temp_dir().join(format!("fjell-ffi-v1-{tag}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let db = dir.join("idx.sqlite").to_string_lossy().into_owned();
    (Fjell::open(db, Arc::new(Empty)).unwrap(), dir)
}

#[test]
fn protect_roundtrip_all_scopes() {
    let (f, dir) = open_one("protect");
    assert!(f.protections().unwrap().is_empty());
    let scopes = vec![
        ProtectScope::Asset { id: 1 },
        ProtectScope::Folder {
            path: "DCIM/Reise".into(),
        },
        ProtectScope::Album { id: 3 },
    ];
    // Ukjent mål avvises med kode (ingen person finnes uten ansiktsindeks).
    assert!(f.protect_set(ProtectScope::Person { id: 9 }).is_err());
    assert!(f.protect_set(ProtectScope::Asset { id: 999 }).is_err());
    for s in &scopes {
        f.protect_set(s.clone()).unwrap();
        f.protect_set(s.clone()).unwrap(); // idempotent
    }
    let got: Vec<ProtectScope> = f
        .protections()
        .unwrap()
        .into_iter()
        .map(|p| p.scope)
        .collect();
    assert_eq!(got.len(), 3);
    for s in &scopes {
        assert!(got.contains(s), "{s:?} mangler");
    }
    // Å fjerne beskyttelse er konfigurasjon og kan gjøres om igjen.
    f.unprotect(scopes[0].clone()).unwrap();
    assert_eq!(f.protections().unwrap().len(), 2);
    f.protect_set(scopes[0].clone()).unwrap();
    assert_eq!(f.protections().unwrap().len(), 3);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn rules_save_preview_toggle_delete_restore() {
    let (f, dir) = open("rules");
    let spec = RuleSpec {
        all_of: vec![
            RuleCondition::IsScreenshot,
            RuleCondition::OlderThanDays { days: 30 },
            RuleCondition::LargerThanBytes { bytes: 1 },
            RuleCondition::SmallerThanBytes { bytes: 1 << 30 },
            RuleCondition::InAlbum { id: 1 },
            RuleCondition::InFolder {
                path: "Skjerm".into(),
            },
            RuleCondition::Mime {
                mime: "image/png".into(),
            },
        ],
    };
    let pv = f.rule_preview(spec.clone()).unwrap();
    assert_eq!(pv.hits, 0);
    assert!(
        f.rules_list().unwrap().is_empty(),
        "forhåndsvisning lagrer ikke"
    );

    // Tom regel avvises (kjernens sikkerhetsregel).
    assert!(f
        .rule_save(RuleDraft {
            id: None,
            name: "tom".into(),
            spec: RuleSpec { all_of: vec![] },
        })
        .is_err());

    let id = f
        .rule_save(RuleDraft {
            id: None,
            name: "Gamle skjermbilder".into(),
            spec: spec.clone(),
        })
        .unwrap();
    let list = f.rules_list().unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].id, id);
    assert_eq!(list[0].spec, spec);
    assert!(list[0].enabled);

    f.rule_toggle(id, false).unwrap();
    assert!(!f.rules_list().unwrap()[0].enabled);
    f.rule_delete(id).unwrap();
    assert!(f.rules_list().unwrap().is_empty());
    f.rule_restore(id).unwrap();
    assert_eq!(
        f.rules_list().unwrap().len(),
        1,
        "sletting av regel kan angres"
    );
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn face_index_is_off_by_default_and_wipe_is_safe_when_empty() {
    let (f, dir) = open("face");
    assert!(!f.face_index_enabled());
    assert!(!f.settings().face_index);
    f.face_index_set(true).unwrap();
    assert!(f.face_index_enabled());
    assert!(f.settings().face_index);
    f.face_index_set(false).unwrap();
    assert!(!f.face_index_enabled());
    assert_eq!(f.face_index_wipe().unwrap(), 0);
    assert!(f.people_list().unwrap().is_empty());
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn people_calls_on_unknown_ids_return_errors_not_panics() {
    let (f, dir) = open("people");
    assert!(f.person_name(99, Some("Ola".into())).is_err());
    assert!(f.person_merge(1, 2).is_err());
    assert!(f.person_unmerge(2).is_err());
    // person_protect beskytter omfanget Person; ukjent person avvises.
    assert!(f.person_protect(5, true).is_err());
    assert!(f.protections().unwrap().is_empty());
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn dashboard_on_empty_index() {
    let (f, dir) = open("dash");
    let d = f.dashboard().unwrap();
    assert_eq!(d.active_count, 0);
    assert_eq!(d.freed_bytes, 0);
    let full = f.dashboard_full().unwrap();
    assert_eq!(full.data, d);
    assert_eq!(full.insights.next_step, NextStep::Nothing);
    assert_eq!(full.insights.progress.permille, 0);
    assert!(full.insights.freed_by_month.is_empty());
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn series_compare_unknown_series_is_error() {
    let (f, dir) = open("series");
    assert!(f.series_compare(12345).is_err());
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn settings_roundtrip_keeps_sensitivity_and_trash_days() {
    let (f, dir) = open("settings");
    let s = f.settings();
    assert_eq!(s.trash_days, 30);
    assert_eq!(s.sensitivity.len(), 6, "én per forslagstype");
    assert!(s.sensitivity.iter().all(|k| k.level == Sensitivity::Normal));

    let before = f.thresholds();
    let mut s2 = s.clone();
    s2.trash_days = 14;
    for k in s2.sensitivity.iter_mut() {
        if k.kind == SuggestionKind::VisualDup {
            k.level = Sensitivity::High;
        }
    }
    f.set_settings(s2);
    let got = f.settings();
    assert_eq!(got.trash_days, 14);
    let lvl = |k: SuggestionKind| got.sensitivity.iter().find(|x| x.kind == k).unwrap().level;
    assert_eq!(lvl(SuggestionKind::VisualDup), Sensitivity::High);
    assert_eq!(lvl(SuggestionKind::Blurry), Sensitivity::Normal);
    let after = f.thresholds();
    assert!(
        after.visual_max_hamming >= before.visual_max_hamming,
        "høy sensitivitet gir ikke strengere terskel"
    );
    assert!(after.series_gap_secs > 0);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn set_settings_with_partial_sensitivity_keeps_the_rest() {
    let (f, dir) = open("partial");
    let mut s = f.settings();
    s.sensitivity = vec![KindSensitivity {
        kind: SuggestionKind::Blurry,
        level: Sensitivity::Low,
    }];
    f.set_settings(s);
    let got = f.settings();
    assert_eq!(got.sensitivity.len(), 6);
    let lvl = |k: SuggestionKind| got.sensitivity.iter().find(|x| x.kind == k).unwrap().level;
    assert_eq!(lvl(SuggestionKind::Blurry), Sensitivity::Low);
    assert_eq!(lvl(SuggestionKind::Screenshot), Sensitivity::Normal);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn set_keeper_unknown_asset_maps_to_code_and_ack_error_is_mirrored() {
    let (f, dir) = open("keeper");
    match f.set_keeper(1, 2) {
        Err(FjellError::UnknownAsset { id }) => assert_eq!(id, 2),
        other => panic!("{other:?}"),
    }
    // Bekreftelse av konsekvenser går som koder, aldri tekst.
    let e: FjellError = fjell_core::api::ApiError::AckRequired(vec![
        fjell_core::model::ConsequenceCode::ProtectedByScope,
    ])
    .into();
    assert_eq!(
        e,
        FjellError::AckRequired {
            codes: vec![ConsequenceCode::ProtectedByScope]
        }
    );
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn reason_detail_conversions_are_lossless() {
    use fjell_core::model as m;
    let cases = [
        (m::ReasonDetail::None, ReasonDetail::None),
        (
            m::ReasonDetail::Percent(42),
            ReasonDetail::Percent { value: 42 },
        ),
        (m::ReasonDetail::Count(3), ReasonDetail::Count { value: 3 }),
        (
            m::ReasonDetail::Position { index: 2, of: 5 },
            ReasonDetail::Position { index: 2, of: 5 },
        ),
        (
            m::ReasonDetail::Rule(m::RuleId(8)),
            ReasonDetail::Rule { id: 8 },
        ),
        (
            m::ReasonDetail::Person(m::PersonId(4)),
            ReasonDetail::Person { id: 4 },
        ),
    ];
    for (core, ffi) in cases {
        assert_eq!(ReasonDetail::from(core), ffi);
        assert_eq!(m::ReasonDetail::from(ffi), core);
    }
}

#[test]
fn next_step_conversion_covers_all_variants() {
    use fjell_core::dashboard as d;
    use fjell_core::model as m;
    assert_eq!(NextStep::from(d::NextStep::Nothing), NextStep::Nothing);
    assert_eq!(
        NextStep::from(d::NextStep::EmptyTrash { count: 2, bytes: 9 }),
        NextStep::EmptyTrash { count: 2, bytes: 9 }
    );
    assert_eq!(
        NextStep::from(d::NextStep::ReviewKind {
            kind: m::SuggestionKind::Series,
            count: 1,
            bytes: 2
        }),
        NextStep::ReviewKind {
            kind: SuggestionKind::Series,
            count: 1,
            bytes: 2
        }
    );
}
