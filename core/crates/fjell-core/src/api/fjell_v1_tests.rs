//! Ende-til-ende for V1 gjennom `Fjell` (integrasjonsfasen): skann syntetisk korpus → serie med beste valg →
//! beskyttet bilde krever egen bekreftelse → regel gir forslag → dashboard stemmer → ansiktsindeks er av som
//! standard og wipe fjerner embeddings → originalenes sjekksum er uendret.
//!
//! Grensesnitt: bare tester. Bruker korpuset fra `pipeline::tests` pluss tre bilder i én serie.

use super::fjell_v1::V1_ERROR_CODES;
use super::*;
use crate::model::ConsequenceCode;
use crate::model::{
    AssetSource, Embedding, FaceBox, FaceObservation, FakeClock, ProtectScope, ReasonCode,
    ReasonDetail, RuleCondition, SuggestionKind,
};
use crate::people::MemFaceAnalyzer;
use crate::pipeline::tests::{scene, Counting, Fx, H, W};
use crate::store::Store;
use std::collections::BTreeMap;
use std::io::Read;
use std::sync::Arc;

/// Kopi av `scene` med lysere rektangel. Mot grunnbildet er pHash-avstanden 8 og 10 (>6: ikke visuelt duplikat,
/// <=10: samme serie); de to variantene er 12 fra hverandre og henger sammen via grunnbildet.
fn shifted(px: &[u8], x0: usize, y0: usize, off: i32) -> Vec<u8> {
    let mut v = px.to_vec();
    for y in y0..y0 + 22 {
        for x in x0..x0 + 22 {
            let i = y * W as usize + x;
            v[i] = (v[i] as i32 + off).clamp(0, 255) as u8;
        }
    }
    assert!(y0 + 22 <= H as usize && x0 + 22 <= W as usize);
    v
}

fn corpus_with_series() -> Fx {
    let mut fx = Fx::new();
    fx.corpus();
    let base = scene(700);
    fx.uniq("s1.jpg", base.clone());
    fx.uniq("s2.jpg", shifted(&base, 8, 8, 80));
    fx.uniq("s3.jpg", shifted(&base, 20, 8, 80));
    fx
}

fn sums(src: &Counting) -> BTreeMap<String, [u8; 32]> {
    let cs = src.list_changes(None).unwrap();
    cs.changed
        .iter()
        .map(|m| {
            let mut r = src.read_bytes(&m.source_id).unwrap();
            let mut b = Vec::new();
            r.read_to_end(&mut b).unwrap();
            (m.source_id.0.clone(), *blake3::hash(&b).as_bytes())
        })
        .collect()
}

fn all(f: &Fjell, kind: SuggestionKind) -> Vec<Card> {
    let mut out = Vec::new();
    let mut cur = None;
    loop {
        let p = f.cards(kind, 3, cur).unwrap();
        out.extend(p.cards);
        match p.next {
            Some(n) => cur = Some(n),
            None => return out,
        }
    }
}

fn face(v: f32) -> FaceObservation {
    FaceObservation {
        bbox: FaceBox {
            x: 100,
            y: 100,
            w: 300,
            h: 300,
        },
        quality: 0.9,
        embedding: Embedding(vec![v, 1.0 - v, 0.0, 0.0]),
    }
}

fn scan(f: &Fjell) {
    f.start_scan(ScanConfig {
        mode: ScanMode::Full,
        priority: 1,
        deadline: None,
    })
    .unwrap();
    let r = f.wait_scan().unwrap().unwrap();
    assert!(r.failures.is_empty(), "{r:?}");
}

#[test]
fn v1_end_to_end() {
    let fx = corpus_with_series();
    let before = sums(&fx.src);
    let faces = Arc::new(
        MemFaceAnalyzer::new()
            .with_faces(scene(100), vec![face(1.0)])
            .with_faces(scene(101), vec![face(0.99)])
            .with_faces(scene(102), vec![face(0.0)]),
    );
    let f = Fjell::with_analyzers(
        Store::open_in_memory().unwrap(),
        fx.src.clone(),
        Arc::new(FakeClock::new(5_000_000)),
        Arc::new(crate::model::NullEyeAnalyzer),
        faces.clone(),
    );
    scan(&f);

    // 1. Serie: én serie med tre bilder, behold-kandidaten har ingen kort, de to andre har kort med posisjon.
    let series = all(&f, SuggestionKind::Series);
    assert_eq!(series.len(), 2, "{series:?}");
    let sid = SeriesId(series[0].group.unwrap().0);
    assert!(series.iter().all(|c| c.group.unwrap().0 == sid.0));
    let cmp = f.series_compare(sid).unwrap();
    assert_eq!(cmp.members.len(), 3);
    assert_eq!(cmp.members[0].asset, cmp.keeper);
    assert!(cmp.members[0].is_keeper);
    assert_eq!(cmp.members[0].reason, ReasonCode::KeeperBestOverall);
    assert!(cmp.members.windows(2).all(|w| w[0].score >= w[1].score));
    assert!(series.iter().all(|c| c.asset != cmp.keeper));
    assert!(series.iter().all(|c| c.keeper == Some(cmp.keeper)));
    for c in &series {
        assert_eq!(c.reason, ReasonCode::SeriesMember);
        assert!(matches!(
            c.detail,
            ReasonDetail::Position { of: 3, index } if (2..=3).contains(&index)
        ));
    }
    // Samme asset aldri i to kort, også med serier og regler.
    let mut seen = std::collections::BTreeSet::new();
    for k in SuggestionKind::ALL {
        for c in all(&f, k) {
            assert!(seen.insert(c.asset), "asset i flere kort: {c:?}");
        }
    }

    // 2. Beskyttelse: beskyttet bilde krever egen bekreftelse og forblir aktivt uten den.
    let victim = series[0].clone();
    f.protect_set(ProtectScope::Asset(victim.asset)).unwrap();
    f.protect_set(ProtectScope::Asset(victim.asset)).unwrap(); // idempotent
    assert_eq!(f.protections().unwrap().len(), 1);
    let r = f.decide_many(vec![(victim.id, Decision::Trash)]);
    let Err(ApiError::AckRequired(codes)) = r else {
        panic!("forventet AckRequired: {r:?}");
    };
    assert!(
        codes.contains(&ConsequenceCode::ProtectedByScope),
        "{codes:?}"
    );
    let state = |a| f.with_store(|s| s.asset_row(a).unwrap().unwrap().state);
    assert_eq!(state(victim.asset), crate::model::State::Active);
    // Fjernet beskyttelse kan settes igjen (angre), og fjerning rører ingen bilder.
    f.unprotect(ProtectScope::Asset(victim.asset)).unwrap();
    assert!(f.protections().unwrap().is_empty());
    f.protect_set(ProtectScope::Asset(victim.asset)).unwrap();
    assert!(matches!(
        f.protect_set(ProtectScope::Folder(String::new())),
        Err(ApiError::Internal(c)) if c == "empty_folder"
    ));

    // 3. Regel: PNG-bilder foreslås; regelen sletter ikke; beskyttelse gir ingen treff; sletting kan angres.
    let spec = RuleSpec {
        all_of: vec![RuleCondition::Mime("image/png".into())],
    };
    assert_eq!(f.rule_preview(spec.clone()).unwrap().hits, 1);
    assert!(f.rules_list().unwrap().is_empty(), "preview lagrer ikke");
    let rid = f
        .rule_save(RuleDraft {
            id: None,
            name: "PNG".into(),
            spec: spec.clone(),
        })
        .unwrap();
    let rule_cards = all(&f, SuggestionKind::Rule);
    assert_eq!(rule_cards.len(), 1);
    assert_eq!(rule_cards[0].reason, ReasonCode::Rule);
    assert_eq!(rule_cards[0].detail, ReasonDetail::Rule(rid));
    assert_eq!(f.rules_list().unwrap()[0].hits, 1);
    assert!(matches!(
        f.rule_save(RuleDraft {
            id: None,
            name: "tom".into(),
            spec: RuleSpec::default()
        }),
        Err(ApiError::Internal(c)) if c == "empty_rule"
    ));
    let png = rule_cards[0].asset;
    f.protect_set(ProtectScope::Asset(png)).unwrap();
    assert_eq!(
        f.rules_list().unwrap()[0].hits,
        0,
        "beskyttet gir ingen treff"
    );
    f.unprotect(ProtectScope::Asset(png)).unwrap();
    assert_eq!(f.rules_list().unwrap()[0].hits, 1);
    f.rule_delete(rid).unwrap();
    assert!(f.rules_list().unwrap().is_empty());
    f.rule_restore(rid).unwrap();
    assert_eq!(f.rules_list().unwrap().len(), 1);
    f.rule_toggle(rid, false).unwrap();
    assert!(all(&f, SuggestionKind::Rule).is_empty());
    f.rule_toggle(rid, true).unwrap();

    // 4. Dashboard stemmer med indeksen.
    let d = f.dashboard().unwrap();
    assert_eq!(d.active_count, 28 + 3);
    assert_eq!(d.trashed_count, 0);
    assert_eq!(d.protected_count, 1);
    assert_eq!(d.series_count, 1);
    assert_eq!(d.rules_active, 1);
    assert_eq!(d.people_count, 0);
    assert_eq!(d.freed_bytes, 0, "ingen påstand om frigjort før purge");
    let series_stat = d
        .per_kind
        .iter()
        .find(|k| k.kind == SuggestionKind::Series)
        .unwrap();
    // Beskyttet bilde teller ikke som «kan ryddes»: serien har to forslag, det ene (victim) er beskyttet.
    // Hvert bilde teller under nøyaktig én type, og uskarpt går foran serie i dashboardet. Etter K8-kalibreringen
    // av uskarphet (main) kan det andre seriebildet også være uskarpt, så forventningen regnes fra indeksen.
    let (series_only, reclaimable): (u64, u64) = f.with_store(|s| {
        let one = |sql: &str| -> u64 {
            s.conn()
                .query_row(sql, [victim.asset.0 as i64], |r| r.get::<_, i64>(0))
                .unwrap() as u64
        };
        (
            one("SELECT count(DISTINCT s.asset_id) FROM suggestion s \
                 WHERE s.kind = 'series' AND s.status = 'pending' AND s.asset_id <> ?1 \
                   AND NOT EXISTS (SELECT 1 FROM suggestion o WHERE o.asset_id = s.asset_id \
                       AND o.status = 'pending' \
                       AND o.kind IN ('exact_dup', 'visual_dup', 'screenshot', 'blurry'))"),
            one("SELECT count(DISTINCT asset_id) FROM suggestion \
                 WHERE status = 'pending' AND asset_id <> ?1"),
        )
    });
    assert_eq!(series_stat.count, series_only);
    assert!(
        series_stat.count <= 1,
        "det beskyttede seriebildet telles aldri"
    );
    assert_eq!(
        d.per_kind.iter().map(|k| k.count).sum::<u64>(),
        reclaimable,
        "beskyttet bilde telles ikke under noen type"
    );
    assert_eq!(
        all(&f, SuggestionKind::Series).len(),
        2,
        "kortene (serie går foran uskarpt) viser fortsatt begge"
    );
    let h = f.home_summary();
    assert_eq!(h.per_kind.len(), SuggestionKind::ALL.len());

    // 5. Ansiktsindeks er av som standard: analysatoren er aldri kalt, ingen personer.
    assert!(!f.face_index_enabled());
    assert!(!f.settings().face_index);
    assert_eq!(faces.calls(), 0);
    assert!(f.people_list().unwrap().is_empty());
    assert_eq!(f.with_store(|s| s.face_count().unwrap()), 0);
    // Slås den på, lages ansikter og personer ved neste skanning.
    f.face_index_set(true).unwrap();
    assert!(f.settings().face_index);
    scan(&f);
    assert!(faces.calls() > 0);
    let people = f.people_list().unwrap();
    assert_eq!(people.len(), 2, "{people:?}");
    assert_eq!(f.with_store(|s| s.face_count().unwrap()), 3);
    let (p1, p2) = (people[0].id, people[1].id);
    f.person_name(p1, Some("Ola".into())).unwrap();
    assert_eq!(f.people_list().unwrap()[0].name.as_deref(), Some("Ola"));
    assert_eq!(
        f.person_name(p1, Some("x".repeat(500))),
        Err(ApiError::Internal("name_too_long".into()))
    );
    assert_eq!(f.person_merge(p1, p2).unwrap(), p1);
    assert_eq!(f.people_list().unwrap().len(), 1);
    f.person_unmerge(p2).unwrap();
    assert_eq!(f.people_list().unwrap().len(), 2);
    f.person_protect(p1, true).unwrap();
    assert!(f
        .protections()
        .unwrap()
        .iter()
        .any(|e| e.scope == ProtectScope::Person(p1)));
    f.person_protect(p1, false).unwrap();
    // Wipe fjerner alle embeddings og personer, aldri bilder.
    assert_eq!(f.face_index_wipe().unwrap(), 3);
    assert_eq!(f.with_store(|s| s.face_count().unwrap()), 0);
    assert!(f.people_list().unwrap().is_empty());
    assert_eq!(f.dashboard().unwrap().active_count, 31);

    // 6. Originalene er uendret (innhold), og ingen bilde er slettet.
    assert_eq!(sums(&fx.src), before);
}

#[test]
fn settings_roundtrip_and_thresholds() {
    let fx = Fx::new();
    let f = Fjell::new(
        Store::open_in_memory().unwrap(),
        fx.src.clone(),
        Arc::new(FakeClock::new(1)),
    );
    let d = f.settings();
    assert_eq!(d, Settings::default());
    assert_eq!(f.thresholds(), Thresholds::default());
    let mut s = d;
    s.trash_days = 14;
    s.sensitivity.set(SuggestionKind::Series, Sensitivity::Low);
    s.sensitivity.set(SuggestionKind::Blurry, Sensitivity::High);
    f.set_settings(s);
    assert_eq!(f.settings(), s);
    let t = f.thresholds();
    assert!(t.series_gap_secs < Thresholds::default().series_gap_secs);
    assert!(t.blur_max_score > Thresholds::default().blur_max_score);
    assert_eq!(
        t.visual_max_hamming,
        Thresholds::default().visual_max_hamming
    );
    // Ansiktsindeks følger innstillingen.
    s.face_index = true;
    f.set_settings(s);
    assert!(f.face_index_enabled());
}

#[test]
fn null_analyzers_never_claim_results() {
    // Uten modell (Null) lagres hverken øyeresultater eller ansikter, selv med ansiktsindeks på.
    let fx = corpus_with_series();
    let f = Fjell::new(
        Store::open_in_memory().unwrap(),
        fx.src.clone(),
        Arc::new(FakeClock::new(5_000_000)),
    );
    f.face_index_set(true).unwrap();
    scan(&f);
    assert_eq!(f.with_store(|s| s.eye_result_count().unwrap()), 0);
    assert_eq!(f.with_store(|s| s.face_count().unwrap()), 0);
    assert!(f.people_list().unwrap().is_empty());
}

#[test]
fn protection_added_after_trash_still_gates_purge() {
    let fx = corpus_with_series();
    let f = Fjell::new(
        Store::open_in_memory().unwrap(),
        fx.src.clone(),
        Arc::new(FakeClock::new(5_000_000)),
    );
    scan(&f);
    let c = all(&f, SuggestionKind::ExactDup).remove(0);
    f.decide_many(vec![(c.id, Decision::Trash)]).unwrap();
    f.protect_set(ProtectScope::Asset(c.asset)).unwrap();
    let intent = f.begin_purge(vec![c.asset]).unwrap();
    assert_eq!(
        f.confirm_purge(&intent),
        Err(ApiError::AckRequired(vec![
            ConsequenceCode::ProtectedByScope
        ]))
    );
    // Med bekreftelse går den videre (selve slettingen skjer i plattformen, aldri her).
    assert!(f
        .confirm_purge_confirmed(&intent, vec![ConsequenceCode::ProtectedByScope])
        .is_ok());
}

/// Hver kode kjernen sender har tekst i nb og en (konsekvenser, V1-feilkoder, forslagstyper, neste steg).
#[test]
fn every_code_the_core_sends_has_i18n_key_in_nb_and_en() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../i18n");
    let mut keys: Vec<String> = ConsequenceCode::ALL
        .iter()
        .map(|c| format!("consequence.{}", c.key()))
        .collect();
    keys.extend(V1_ERROR_CODES.iter().map(|c| format!("error.{c}")));
    keys.extend(
        SuggestionKind::ALL
            .iter()
            .map(|k| format!("suggestion.kind.{}", k.as_str())),
    );
    keys.extend(["review", "empty_trash", "nothing"].map(|k| format!("dashboard.next.{k}")));
    for loc in ["nb", "en"] {
        let text = std::fs::read_to_string(root.join(format!("{loc}.json"))).unwrap();
        for k in &keys {
            assert!(text.contains(&format!("\"{k}\"")), "{loc}.json mangler {k}");
        }
    }
}

/// Keeper-reglenes beskyttede mapper (`ProtectFolder`, docs/mvp/REGLER.md) vinner over V1-forslagene: bilder i
/// mappen får verken serie- eller regelforslag, forhåndsvisningen teller dem ikke, og alt kommer tilbake når
/// keeper-regelen fjernes (konfigurasjon, ingen bilder røres). Kjøres både mot minnedatabase (utregning under
/// låsen) og indeksfil (utregning på egen lesetilkobling).
#[test]
fn keeper_rule_protected_folder_blocks_series_and_rule_suggestions() {
    use crate::rules::KeeperRule;
    let dir = std::env::temp_dir().join(format!("fjell-v1-guard-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    for store in [
        Store::open_in_memory().unwrap(),
        Store::open(&dir.join("index.db")).unwrap(),
    ] {
        let mut fx = Fx::new();
        fx.corpus();
        let base = scene(700);
        fx.uniq("arkiv/s1.jpg", base.clone());
        fx.uniq("arkiv/s2.jpg", shifted(&base, 8, 8, 80));
        fx.uniq("arkiv/s3.jpg", shifted(&base, 20, 8, 80));
        let f = Fjell::new(store, fx.src.clone(), Arc::new(FakeClock::new(5_000_000)));
        scan(&f);
        f.sync_groups().unwrap();
        let spec = RuleSpec {
            all_of: vec![RuleCondition::LargerThanBytes(0)],
        };
        let rid = f
            .rule_save(RuleDraft {
                id: None,
                name: "alt".into(),
                spec: spec.clone(),
            })
            .unwrap();
        let count = |sql: &str| -> i64 {
            f.with_store(|s| s.conn().query_row(sql, [], |r| r.get(0)).unwrap())
        };
        const IN_FOLDER: &str =
            "SELECT count(*) FROM suggestion s JOIN asset a ON a.id = s.asset_id \
                                 WHERE a.source_id LIKE 'arkiv/%' AND s.status = 'pending'";
        const SERIES: &str = "SELECT count(*) FROM grp WHERE kind = 'series'";
        let total = f.rule_preview(spec.clone()).unwrap().hits;
        assert_eq!(total, 28 + 3);
        assert_eq!(f.rules_list().unwrap()[0].hits, total);
        assert_eq!(count(SERIES), 1);
        // To serieforslag og tre regelforslag i mappen.
        assert!(count(IN_FOLDER) >= 5, "{}", count(IN_FOLDER));

        f.set_keeper_rules(vec![KeeperRule::ProtectFolder {
            prefix: "arkiv".into(),
        }])
        .unwrap();
        f.sync_groups().unwrap();
        assert_eq!(count(IN_FOLDER), 0, "beskyttet mappe får aldri forslag");
        assert_eq!(count(SERIES), 0);
        assert_eq!(f.rule_preview(spec.clone()).unwrap().hits, total - 3);
        assert_eq!(f.rules_list().unwrap()[0].hits, total - 3);
        // Sperren gjelder også når forslagsregelen endres direkte (uten ny gruppeutregning).
        f.rule_toggle(rid, false).unwrap();
        f.rule_toggle(rid, true).unwrap();
        assert_eq!(count(IN_FOLDER), 0);
        assert_eq!(f.rules_list().unwrap()[0].hits, total - 3);

        // Fjernes keeper-regelen, er forslagene tilbake. Ingen bilder er rørt underveis.
        f.set_keeper_rules(Vec::new()).unwrap();
        f.sync_groups().unwrap();
        assert!(count(IN_FOLDER) >= 5);
        assert_eq!(count(SERIES), 1);
        assert_eq!(f.rules_list().unwrap()[0].hits, total);
        assert_eq!(count("SELECT count(*) FROM asset WHERE state <> 0"), 0);
        assert_eq!(count("SELECT count(*) FROM action_log"), 0);
    }
    let _ = std::fs::remove_dir_all(&dir);
}
