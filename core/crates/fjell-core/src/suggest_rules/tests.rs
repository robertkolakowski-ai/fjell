//! Tester for `suggest_rules` (forslagsregler): koding, vilkår, beskyttelse, evaluering, angre (av/på, sletting, gjenoppretting),
//! egenskapstester («ingen regel gir søppel uten brukerens valg») og ytelse (10 regler, 50 000 bilder).

use super::spec::{self, SpecError};
use super::*;
use crate::model::{AssetId, ReasonCode, RuleCondition as C, RuleId, RuleSpec};
use crate::store::Store;
use proptest::prelude::*;
use rusqlite::{params, Connection};
use std::time::Instant;

const NOW: i64 = 1_800_000_000;

// Testene under er skrevet uten sperre (`Guard = None`). Disse innpakningene skygger for `super::*` slik at
// testene står uendret; sperren (keeper-reglenes beskyttede mapper) har egne tester nederst.
fn save(
    store: &mut Store,
    id: Option<RuleId>,
    name: &str,
    spec: &RuleSpec,
    now: i64,
) -> Result<RuleId, RuleError> {
    super::save(store, id, name, spec, now, None)
}
fn toggle(store: &mut Store, id: RuleId, enabled: bool, now: i64) -> Result<EvalReport, RuleError> {
    super::toggle(store, id, enabled, now, None)
}
fn delete(store: &mut Store, id: RuleId, now: i64) -> Result<EvalReport, RuleError> {
    super::delete(store, id, now, None)
}
fn restore(store: &mut Store, id: RuleId, now: i64) -> Result<EvalReport, RuleError> {
    super::restore(store, id, now, None)
}
fn preview(store: &Store, spec: &RuleSpec, now: i64) -> Result<Preview, RuleError> {
    super::preview(store, spec, now, None)
}
fn match_ids(store: &Store, spec: &RuleSpec, now: i64) -> Result<Vec<AssetId>, RuleError> {
    super::match_ids(store, spec, now, None)
}
fn evaluate(store: &mut Store, now: i64) -> Result<EvalReport, RuleError> {
    super::evaluate(store, now, None)
}
const DAY: i64 = 86_400;

fn spec_of(all_of: Vec<C>) -> RuleSpec {
    RuleSpec { all_of }
}

/// Bilde i indeksen med sine standardverdier (aktivt, ubeskyttet, 1 000 bytes, JPEG).
struct A {
    id: i64,
    source: String,
    kind: &'static str,
    created: i64,
    bytes: i64,
    mime: &'static str,
    hint: i64,
    state: i64,
    flags: i64,
}

fn a(id: i64) -> A {
    A {
        id,
        source: format!("/DCIM/a/{id}"),
        kind: "photo",
        created: NOW - 10 * DAY,
        bytes: 1_000,
        mime: "image/jpeg",
        hint: 0,
        state: 0,
        flags: 0,
    }
}

fn put(c: &Connection, x: &A) {
    c.execute(
        "INSERT INTO asset(id, source_id, kind, created_at, mod_token, width, height, bytes, mime, \
         platform_screenshot_hint, state, protect_flags) VALUES (?1,?2,?3,?4,'m',100,100,?5,?6,?7,?8,?9)",
        params![x.id, x.source, x.kind, x.created, x.bytes, x.mime, x.hint, x.state, x.flags],
    )
    .unwrap();
}

fn store_with(assets: Vec<A>) -> Store {
    let s = Store::open_in_memory().unwrap();
    for x in &assets {
        put(s.conn(), x);
    }
    s
}

fn ids(s: &Store, spec: &RuleSpec) -> Vec<u64> {
    match_ids(s, spec, NOW)
        .unwrap()
        .into_iter()
        .map(|a| a.0)
        .collect()
}

fn count(s: &Store, sql: &str) -> i64 {
    s.conn().query_row(sql, [], |r| r.get(0)).unwrap()
}

fn protect(s: &Store, scope: &str, target: &str) {
    s.conn()
        .execute(
            "INSERT INTO protection(scope, target, created_at) VALUES (?1, ?2, 0)",
            params![scope, target],
        )
        .unwrap();
}

// --- koding ---------------------------------------------------------------------------------------

#[test]
fn spec_roundtrip_all_conditions_and_special_characters() {
    let s = spec_of(vec![
        C::IsScreenshot,
        C::IsBlurry,
        C::IsExactDuplicate,
        C::OlderThanDays(30),
        C::LargerThanBytes(u64::MAX),
        C::SmallerThanBytes(0),
        C::InAlbum(7),
        C::InFolder("/Bilder/a;b=c%d/ø æ å\n".into()),
        C::Mime("image/png".into()),
    ]);
    let text = spec::encode(&s);
    assert!(text.starts_with("v1;screenshot;blurry;exact_dup;older=30;"));
    assert!(!text.contains('\n'));
    assert_eq!(spec::decode(&text).unwrap(), s);
}

#[test]
fn spec_format_is_stable() {
    let s = spec_of(vec![
        C::IsScreenshot,
        C::OlderThanDays(30),
        C::InFolder("/a".into()),
    ]);
    assert_eq!(spec::encode(&s), "v1;screenshot;older=30;folder=/a");
    assert_eq!(spec::encode(&RuleSpec::default()), "v1");
    assert_eq!(spec::decode("v1").unwrap(), RuleSpec::default());
}

#[test]
fn spec_decode_rejects_unknown_and_broken_input() {
    assert_eq!(spec::decode(""), Err(SpecError::UnknownVersion));
    assert_eq!(
        spec::decode("v2;screenshot"),
        Err(SpecError::UnknownVersion)
    );
    assert_eq!(spec::decode("v1;nope"), Err(SpecError::UnknownCondition));
    assert_eq!(
        spec::decode("v1;screenshot=1"),
        Err(SpecError::UnknownCondition)
    );
    assert_eq!(spec::decode("v1;older"), Err(SpecError::BadNumber));
    assert_eq!(spec::decode("v1;older=-1"), Err(SpecError::BadNumber));
    assert_eq!(spec::decode("v1;folder=%G1"), Err(SpecError::BadEscape));
    assert_eq!(spec::decode("v1;folder=%FF"), Err(SpecError::BadEscape));
    assert_eq!(spec::decode("v1;folder=%"), Err(SpecError::BadEscape));
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(128))]
    #[test]
    fn spec_roundtrips_for_any_text(folder in ".*", mime in ".*", n in any::<u64>(), d in any::<u32>()) {
        let s = spec_of(vec![C::InFolder(folder), C::Mime(mime), C::LargerThanBytes(n), C::OlderThanDays(d)]);
        prop_assert_eq!(spec::decode(&spec::encode(&s)).unwrap(), s);
    }
}

// --- vilkår ---------------------------------------------------------------------------------------

#[test]
fn empty_rule_matches_nothing() {
    let s = store_with(vec![a(1), a(2)]);
    assert!(ids(&s, &RuleSpec::default()).is_empty());
    assert_eq!(
        preview(&s, &RuleSpec::default(), NOW).unwrap(),
        Preview::default()
    );
}

#[test]
fn size_age_mime_and_folder_conditions() {
    let mut big = a(1);
    big.bytes = 5_000_000;
    let mut old = a(2);
    old.created = NOW - 400 * DAY;
    let mut png = a(3);
    png.mime = "IMAGE/PNG";
    let mut other = a(4);
    other.source = "/Bilder/x/4".into();
    let mut tiny = a(5);
    tiny.bytes = 10;
    let s = store_with(vec![big, old, png, other, tiny]);
    assert_eq!(
        ids(&s, &spec_of(vec![C::LargerThanBytes(1_000_000)])),
        vec![1]
    );
    assert_eq!(ids(&s, &spec_of(vec![C::SmallerThanBytes(100)])), vec![5]);
    assert_eq!(ids(&s, &spec_of(vec![C::OlderThanDays(365)])), vec![2]);
    assert_eq!(
        ids(&s, &spec_of(vec![C::Mime("image/png".into())])),
        vec![3]
    );
    assert_eq!(
        ids(&s, &spec_of(vec![C::InFolder("/Bilder/".into())])),
        vec![4]
    );
    // OG: alle vilkår må gjelde.
    assert!(ids(
        &s,
        &spec_of(vec![C::OlderThanDays(365), C::LargerThanBytes(1_000_000)])
    )
    .is_empty());
    // Alderen er strengt «eldre enn»: nøyaktig på grensen treffer ikke.
    let mut edge = a(6);
    edge.created = NOW - 30 * DAY;
    put(s.conn(), &edge);
    assert!(!ids(&s, &spec_of(vec![C::OlderThanDays(30)])).contains(&6));
}

#[test]
fn screenshot_blurry_duplicate_and_album_conditions() {
    let mut shot = a(1);
    shot.kind = "screenshot_hint";
    let mut hint = a(2);
    hint.hint = 1;
    let s = store_with(vec![shot, hint, a(3), a(4), a(5), a(6)]);
    let c = s.conn();
    // Uskarp via fingerprint (3) og via eksisterende forslag (4); skarp (5).
    for (id, score) in [(3, 0.001), (5, 0.5)] {
        c.execute(
            "INSERT INTO fingerprint(asset_id, blur_score) VALUES (?1, ?2)",
            params![id, score],
        )
        .unwrap();
    }
    c.execute(
        "INSERT INTO suggestion(asset_id, kind, reason_code, confidence) VALUES (4,'blurry','blurry',0.5)",
        [],
    )
    .unwrap();
    // Eksakt duplikat: 5 er keeper, 6 er duplikat.
    c.execute(
        "INSERT INTO grp(id, kind, keeper_id, created_at) VALUES (1,'exact',5,0)",
        [],
    )
    .unwrap();
    for (m, r) in [(5, 0), (6, 1)] {
        c.execute(
            "INSERT INTO grp_member(grp_id, asset_id, rank) VALUES (1, ?1, ?2)",
            params![m, r],
        )
        .unwrap();
    }
    c.execute("INSERT INTO album(id, name) VALUES (9, 'x')", [])
        .unwrap();
    c.execute(
        "INSERT INTO asset_album(asset_id, album_id) VALUES (3, 9)",
        [],
    )
    .unwrap();
    assert_eq!(ids(&s, &spec_of(vec![C::IsScreenshot])), vec![1, 2]);
    assert_eq!(ids(&s, &spec_of(vec![C::IsBlurry])), vec![3, 4]);
    assert_eq!(ids(&s, &spec_of(vec![C::IsExactDuplicate])), vec![6]);
    assert_eq!(ids(&s, &spec_of(vec![C::InAlbum(9)])), vec![3]);
    assert_eq!(ids(&s, &spec_of(vec![C::InAlbum(9), C::IsBlurry])), vec![3]);
}

#[test]
fn only_active_assets_match() {
    let mut trashed = a(1);
    trashed.state = 1;
    let mut purged = a(2);
    purged.state = 2;
    let s = store_with(vec![trashed, purged, a(3)]);
    assert_eq!(
        ids(&s, &spec_of(vec![C::Mime("image/jpeg".into())])),
        vec![3]
    );
}

// --- beskyttelse ------------------------------------------------------------------------------------

#[test]
fn protection_always_beats_rules() {
    let mut fav = a(1);
    fav.flags = 1;
    let mut asset_prot = a(2);
    asset_prot.source = "/p/2".into();
    let mut folder_prot = a(3);
    folder_prot.source = "/Familie/3".into();
    let mut album_prot = a(4);
    album_prot.source = "/p/4".into();
    let mut face = a(5);
    face.source = "/p/5".into();
    let mut merged = a(6);
    merged.source = "/p/6".into();
    let mut free = a(7);
    free.source = "/p/7".into();
    let s = store_with(vec![
        fav,
        asset_prot,
        folder_prot,
        album_prot,
        face,
        merged,
        free,
    ]);
    let c = s.conn();
    protect(&s, "asset", "2");
    protect(&s, "folder", "/Familie/");
    c.execute("INSERT INTO album(id, name) VALUES (3, 'a')", [])
        .unwrap();
    c.execute(
        "INSERT INTO asset_album(asset_id, album_id) VALUES (4, 3)",
        [],
    )
    .unwrap();
    protect(&s, "album", "3");
    // Person 10 er beskyttet; person 11 er slått sammen inn i 10; bilde 5 har 10, bilde 6 har 11.
    c.execute("INSERT INTO person(id, created_at) VALUES (10, 0)", [])
        .unwrap();
    c.execute(
        "INSERT INTO person(id, created_at, merged_into) VALUES (11, 0, 10)",
        [],
    )
    .unwrap();
    for (asset, person) in [(5, 10), (6, 11)] {
        c.execute(
            "INSERT INTO face_embedding(asset_id, person_id, box_x, box_y, box_w, box_h, quality, embedding) \
             VALUES (?1, ?2, 0, 0, 1, 1, 1.0, x'00')",
            params![asset, person],
        )
        .unwrap();
    }
    protect(&s, "person", "10");
    let all = spec_of(vec![C::Mime("image/jpeg".into())]);
    assert_eq!(ids(&s, &all), vec![7]);
    let p = preview(&s, &all, NOW).unwrap();
    assert_eq!((p.hits, p.bytes, p.sample), (1, 1_000, vec![AssetId(7)]));
}

// --- evaluering og forslag ------------------------------------------------------------------------

fn jpeg() -> RuleSpec {
    spec_of(vec![C::Mime("image/jpeg".into())])
}

#[test]
fn evaluate_creates_pending_rule_suggestions_only() {
    let mut s = store_with(vec![a(1), a(2), a(3)]);
    let id = save(&mut s, None, "  JPEG  ", &jpeg(), NOW).unwrap();
    assert_eq!(count(&s, "SELECT count(*) FROM rule_hit"), 3);
    let (kind, status, reason): (String, String, String) = s
        .conn()
        .query_row(
            "SELECT kind, status, reason_code FROM suggestion WHERE asset_id = 1",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap();
    assert_eq!((kind.as_str(), status.as_str()), ("rule", "pending"));
    assert_eq!(reason, ReasonCode::Rule.key());
    assert_eq!(
        reason_detail(&s, AssetId(1)).unwrap(),
        ReasonDetail::Rule(id)
    );
    assert_eq!(reason_detail(&s, AssetId(99)).unwrap(), ReasonDetail::None);
    let l = list(&s).unwrap();
    assert_eq!(
        (l[0].name.as_str(), l[0].hits, l[0].enabled),
        ("JPEG", 3, true)
    );
    // Ingenting er flyttet eller loggført.
    assert_eq!(count(&s, "SELECT count(*) FROM asset WHERE state <> 0"), 0);
    assert_eq!(count(&s, "SELECT count(*) FROM action_log"), 0);
}

#[test]
fn evaluate_respects_user_decisions_and_removes_stale_pending() {
    let mut s = store_with(vec![a(1), a(2), a(3)]);
    let id = save(&mut s, None, "r", &jpeg(), NOW).unwrap();
    let c = s.conn();
    c.execute(
        "UPDATE suggestion SET status='rejected' WHERE asset_id = 1",
        [],
    )
    .unwrap();
    c.execute(
        "UPDATE suggestion SET status='accepted' WHERE asset_id = 2",
        [],
    )
    .unwrap();
    // Bilde 3 slutter å treffe regelen.
    c.execute("UPDATE asset SET mime = 'image/png' WHERE id = 3", [])
        .unwrap();
    let rep = evaluate(&mut s, NOW).unwrap();
    assert_eq!(rep.suggestions_removed, 1);
    let st = |asset: i64| -> Option<String> {
        s.conn()
            .query_row(
                "SELECT status FROM suggestion WHERE asset_id = ?1",
                [asset],
                |r| r.get(0),
            )
            .ok()
    };
    assert_eq!(st(1).as_deref(), Some("rejected")); // kommer ikke tilbake som pending
    assert_eq!(st(2).as_deref(), Some("accepted"));
    assert_eq!(st(3), None);
    assert_eq!(
        reason_detail(&s, AssetId(2)).unwrap(),
        ReasonDetail::Rule(id)
    );
}

#[test]
fn evaluate_restores_suggestions_removed_elsewhere() {
    // Grupperingen (`apply_grouping`) rydder ventende forslag av alle typer; neste evaluering gir dem tilbake.
    let mut s = store_with(vec![a(1), a(2)]);
    save(&mut s, None, "r", &jpeg(), NOW).unwrap();
    s.conn().execute("DELETE FROM suggestion", []).unwrap();
    let rep = evaluate(&mut s, NOW).unwrap();
    assert_eq!(rep.suggestions_added, 2);
    assert_eq!(
        count(
            &s,
            "SELECT count(*) FROM suggestion WHERE status = 'pending'"
        ),
        2
    );
    // Uendret indeks gir ingen endringer.
    let rep = evaluate(&mut s, NOW).unwrap();
    assert_eq!((rep.suggestions_added, rep.suggestions_removed), (0, 0));
}

#[test]
fn disable_delete_and_restore_are_reversible() {
    let mut s = store_with(vec![a(1), a(2)]);
    let id = save(&mut s, None, "r", &jpeg(), NOW).unwrap();
    toggle(&mut s, id, false, NOW).unwrap();
    assert_eq!(count(&s, "SELECT count(*) FROM rule_hit"), 0);
    assert_eq!(count(&s, "SELECT count(*) FROM suggestion"), 0);
    assert_eq!(list(&s).unwrap()[0].hits, 0);
    toggle(&mut s, id, true, NOW).unwrap();
    assert_eq!(count(&s, "SELECT count(*) FROM suggestion"), 2);

    delete(&mut s, id, NOW + 5).unwrap();
    assert!(list(&s).unwrap().is_empty());
    let gone = list_deleted(&s).unwrap();
    assert_eq!((gone.len(), gone[0].deleted_at), (1, Some(NOW + 5)));
    assert_eq!(count(&s, "SELECT count(*) FROM rule"), 1); // raden består
    assert_eq!(count(&s, "SELECT count(*) FROM rule_hit"), 0);
    assert_eq!(count(&s, "SELECT count(*) FROM suggestion"), 0);
    // Slettet regel kan ikke endres eller slås på, bare hentes tilbake.
    assert!(matches!(
        save(&mut s, Some(id), "x", &jpeg(), NOW),
        Err(RuleError::NotFound)
    ));
    assert!(matches!(
        toggle(&mut s, id, true, NOW),
        Err(RuleError::NotFound)
    ));

    restore(&mut s, id, NOW).unwrap();
    assert_eq!(list(&s).unwrap()[0].hits, 2);
    assert_eq!(count(&s, "SELECT count(*) FROM suggestion"), 2);
    // Gjenopprettet «av»-regel forblir av.
    toggle(&mut s, id, false, NOW).unwrap();
    delete(&mut s, id, NOW).unwrap();
    restore(&mut s, id, NOW).unwrap();
    assert!(!list(&s).unwrap()[0].enabled);
    assert_eq!(count(&s, "SELECT count(*) FROM rule_hit"), 0);
}

#[test]
fn rules_cannot_be_hard_deleted() {
    let mut s = store_with(vec![a(1)]);
    save(&mut s, None, "r", &jpeg(), NOW).unwrap();
    assert!(s.conn().execute("DELETE FROM rule", []).is_err());
}

#[test]
fn edit_replaces_hits_and_unknown_ids_are_not_found() {
    let mut s = store_with(vec![a(1), a(2)]);
    s.conn()
        .execute("UPDATE asset SET mime = 'image/png' WHERE id = 2", [])
        .unwrap();
    let id = save(&mut s, None, "r", &jpeg(), NOW).unwrap();
    assert_eq!(count(&s, "SELECT count(*) FROM suggestion"), 1);
    save(
        &mut s,
        Some(id),
        "r2",
        &spec_of(vec![C::Mime("image/png".into())]),
        NOW,
    )
    .unwrap();
    assert_eq!(ids_hit(&s), vec![2]);
    assert_eq!(count(&s, "SELECT count(*) FROM suggestion"), 1); // gammelt ventende forslag fjernet
    assert!(matches!(
        toggle(&mut s, RuleId(999), true, NOW),
        Err(RuleError::NotFound)
    ));
    assert!(matches!(
        delete(&mut s, RuleId(999), NOW),
        Err(RuleError::NotFound)
    ));
    assert!(matches!(
        restore(&mut s, RuleId(999), NOW),
        Err(RuleError::NotFound)
    ));
}

fn ids_hit(s: &Store) -> Vec<i64> {
    let mut st = s
        .conn()
        .prepare("SELECT asset_id FROM rule_hit ORDER BY 1")
        .unwrap();
    let v = st
        .query_map([], |r| r.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    v
}

#[test]
fn save_validates_input() {
    let mut s = store_with(vec![a(1)]);
    let e = |r: Result<RuleId, RuleError>| format!("{:?}", r.unwrap_err());
    assert_eq!(
        e(save(&mut s, None, "r", &RuleSpec::default(), NOW)),
        "EmptySpec"
    );
    assert_eq!(e(save(&mut s, None, "  ", &jpeg(), NOW)), "EmptyName");
    assert_eq!(
        e(save(&mut s, None, &"x".repeat(81), &jpeg(), NOW)),
        "NameTooLong"
    );
    assert_eq!(
        e(save(
            &mut s,
            None,
            "r",
            &spec_of(vec![C::InFolder(" ".into())]),
            NOW
        )),
        "InvalidCondition"
    );
    assert_eq!(
        e(save(
            &mut s,
            None,
            "r",
            &spec_of(vec![C::Mime(String::new())]),
            NOW
        )),
        "InvalidCondition"
    );
    assert_eq!(count(&s, "SELECT count(*) FROM rule"), 0);
    assert!(save(&mut s, None, &"x".repeat(80), &jpeg(), NOW).is_ok());
}

#[test]
fn preview_changes_nothing() {
    let s = store_with((1..=40).map(a).collect());
    let before = count(&s, "SELECT count(*) FROM rule")
        + count(&s, "SELECT count(*) FROM rule_hit")
        + count(&s, "SELECT count(*) FROM suggestion");
    let p = preview(&s, &jpeg(), NOW).unwrap();
    assert_eq!((p.hits, p.bytes), (40, 40_000));
    assert_eq!(p.sample.len(), 24);
    assert_eq!(p.sample[0], AssetId(1));
    let after = count(&s, "SELECT count(*) FROM rule")
        + count(&s, "SELECT count(*) FROM rule_hit")
        + count(&s, "SELECT count(*) FROM suggestion");
    assert_eq!(before, after);
}

#[test]
fn corrupt_stored_spec_is_skipped_not_guessed() {
    let mut s = store_with(vec![a(1)]);
    s.conn()
        .execute(
            "INSERT INTO rule(name, spec, enabled, created_at) VALUES ('ny', 'v9;alt', 1, 0)",
            [],
        )
        .unwrap();
    let rep = evaluate(&mut s, NOW).unwrap();
    assert_eq!(
        (rep.rules_evaluated, rep.rules_skipped, rep.hits),
        (0, 1, 0)
    );
    let l = list(&s).unwrap();
    assert!(l[0].corrupt && l[0].spec.is_empty());
}

// --- egenskapstester: ingen regel gir søppel uten brukerens valg ----------------------------------------

#[derive(Clone, Debug)]
struct GenAsset {
    folder: u8,
    bytes: u32,
    mime: u8,
    age_days: u16,
    hint: bool,
    flags: u8,
    state: u8,
}

fn gen_asset() -> impl Strategy<Value = GenAsset> {
    (
        0u8..3,
        0u32..5_000,
        0u8..2,
        0u16..800,
        any::<bool>(),
        prop_oneof![3 => Just(0u8), 1 => 1u8..16],
        prop_oneof![5 => Just(0u8), 1 => Just(1u8), 1 => Just(2u8)],
    )
        .prop_map(
            |(folder, bytes, mime, age_days, hint, flags, state)| GenAsset {
                folder,
                bytes,
                mime,
                age_days,
                hint,
                flags,
                state,
            },
        )
}

fn gen_cond() -> impl Strategy<Value = C> {
    prop_oneof![
        Just(C::IsScreenshot),
        (0u32..900).prop_map(C::OlderThanDays),
        (0u64..5_000).prop_map(C::LargerThanBytes),
        (0u64..5_000).prop_map(C::SmallerThanBytes),
        (0u8..3).prop_map(|f| C::InFolder(format!("/f{f}/"))),
        (0u8..2).prop_map(|m| C::Mime(MIMES[m as usize].into())),
    ]
}

const MIMES: [&str; 2] = ["image/jpeg", "image/png"];

#[derive(Clone, Debug)]
struct GenRule {
    all_of: Vec<C>,
    enabled: bool,
    deleted: bool,
}

fn gen_rule() -> impl Strategy<Value = GenRule> {
    (
        prop::collection::vec(gen_cond(), 0..4),
        prop_oneof![3 => Just(true), 1 => Just(false)],
        prop_oneof![3 => Just(false), 1 => Just(true)],
    )
        .prop_map(|(all_of, enabled, deleted)| GenRule {
            all_of,
            enabled,
            deleted,
        })
}

/// Uavhengig fasit for vilkårene testen genererer (ingen delspørringer).
fn oracle(c: &C, x: &GenAsset) -> bool {
    match c {
        C::IsScreenshot => x.hint,
        C::OlderThanDays(d) => (x.age_days as i64) * DAY > (*d as i64) * DAY,
        C::LargerThanBytes(n) => x.bytes as u64 > *n,
        C::SmallerThanBytes(n) => (x.bytes as u64) < *n,
        C::InFolder(p) => format!("/f{}/{}", x.folder, 0).starts_with(p.as_str()),
        C::Mime(m) => MIMES[x.mime as usize] == m,
        _ => unreachable!("genereres ikke"),
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(48))]
    #[test]
    fn no_rule_ever_trashes_or_hits_protected_assets(
        assets in prop::collection::vec(gen_asset(), 1..40),
        rules in prop::collection::vec(gen_rule(), 0..6),
        protect_folder in prop::option::of(0u8..3),
        protect_assets in prop::collection::vec(1usize..40, 0..4),
        rerun in any::<bool>(),
    ) {
        let mut s = Store::open_in_memory().unwrap();
        for (i, g) in assets.iter().enumerate() {
            let mut x = a(i as i64 + 1);
            x.source = format!("/f{}/{}", g.folder, 0) + &format!("/{}", i + 1);
            x.bytes = g.bytes as i64;
            x.mime = MIMES[g.mime as usize];
            x.created = NOW - g.age_days as i64 * DAY;
            x.hint = g.hint as i64;
            x.flags = g.flags as i64;
            x.state = g.state as i64;
            put(s.conn(), &x);
        }
        if let Some(f) = protect_folder {
            protect(&s, "folder", &format!("/f{f}/"));
        }
        for p in &protect_assets {
            if *p <= assets.len() {
                s.conn().execute(
                    "INSERT OR IGNORE INTO protection(scope, target, created_at) VALUES ('asset', ?1, 0)",
                    [p.to_string()],
                ).unwrap();
            }
        }
        let mut live: Vec<(RuleId, &GenRule)> = Vec::new();
        for r in &rules {
            let id = s.rule_insert("r", &spec::encode(&RuleSpec { all_of: r.all_of.clone() }), 0).unwrap();
            if !r.enabled { s.rule_set_enabled(id, false).unwrap(); }
            if r.deleted { s.rule_set_deleted(id, Some(1)).unwrap(); }
            if r.enabled && !r.deleted { live.push((id, r)); }
        }
        let snapshot = |s: &Store| -> Vec<(i64, i64, i64)> {
            let mut st = s.conn().prepare("SELECT id, state, protect_flags FROM asset ORDER BY id").unwrap();
            let v = st.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))).unwrap().collect::<Result<_, _>>().unwrap();
            v
        };
        let before = snapshot(&s);
        evaluate(&mut s, NOW).unwrap();
        if rerun { evaluate(&mut s, NOW).unwrap(); }

        // 1. Ingen tilstandsendring, ingen handling i loggen.
        prop_assert_eq!(snapshot(&s), before);
        prop_assert_eq!(count(&s, "SELECT count(*) FROM action_log"), 0);
        // 2. Forslagene er bare ventende, av typen regel, aldri forhåndsvalgt.
        prop_assert_eq!(count(&s, "SELECT count(*) FROM suggestion WHERE status <> 'pending' OR kind <> 'rule'"), 0);
        // 3. Treff finnes bare for aktive regler.
        for h in s.rule_hit_counts().unwrap() {
            prop_assert!(live.iter().any(|(id, _)| *id == h.0), "treff for av/slettet regel");
        }
        // 4. Ingen treff eller forslag på beskyttede eller ikke-aktive bilder.
        let prot_asset = |id: i64| count(&s, &format!("SELECT count(*) FROM protection WHERE scope='asset' AND target='{id}'")) > 0;
        let prot_folder = |g: &GenAsset| protect_folder == Some(g.folder);
        let bad = |id: i64| {
            let g = &assets[id as usize - 1];
            g.flags != 0 || g.state != 0 || prot_asset(id) || prot_folder(g)
        };
        for id in ids_hit(&s) { prop_assert!(!bad(id), "treff på beskyttet bilde {}", id); }
        let mut st = s.conn().prepare("SELECT asset_id FROM suggestion").unwrap();
        let sugg: Vec<i64> = st.query_map([], |r| r.get(0)).unwrap().collect::<Result<_, _>>().unwrap();
        for id in sugg { prop_assert!(!bad(id), "forslag på beskyttet bilde {}", id); }
        // 5. Treffene er nøyaktig det fasiten sier (tom regel treffer ingenting).
        for (id, r) in &live {
            let mut got: Vec<i64> = {
                let mut st = s.conn().prepare("SELECT asset_id FROM rule_hit WHERE rule_id = ?1 ORDER BY 1").unwrap();
                let v = st.query_map([id.0 as i64], |r| r.get(0)).unwrap().collect::<Result<_, _>>().unwrap();
                v
            };
            got.sort();
            let want: Vec<i64> = if r.all_of.is_empty() {
                vec![]
            } else {
                assets.iter().enumerate()
                    .filter(|(i, g)| !bad(*i as i64 + 1) && r.all_of.iter().all(|c| oracle(c, g)))
                    .map(|(i, _)| i as i64 + 1)
                    .collect()
            };
            prop_assert_eq!(got, want);
        }
    }
}

// --- ytelse ---------------------------------------------------------------------------------------

/// Budsjettet (10 regler, 50 000 bilder, p95 < 50 ms) gjelder `--release`; i debug brukes en romslig faktor
/// slik at testen fortsatt fanger størrelsesorden-regresjoner (som `actions/tests/perf.rs`).
fn factor() -> f64 {
    if cfg!(debug_assertions) {
        25.0
    } else {
        1.0
    }
}

fn p95(mut v: Vec<f64>) -> f64 {
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    v[(v.len() as f64 * 0.95).ceil() as usize - 1]
}

fn big_store(n: i64) -> (Store, std::path::PathBuf) {
    let dir = std::env::temp_dir().join(format!("fjell-rules-perf-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let s = Store::open(&dir.join("index.db")).unwrap();
    let c = s.conn();
    c.execute_batch("BEGIN").unwrap();
    for i in 1..=n {
        let mut x = a(i);
        x.source = format!("/DCIM/{}/{i}", i % 25);
        x.bytes = 50_000 + (i * 7_919) % 9_000_000;
        x.mime = ["image/jpeg", "image/heic", "image/png"][(i % 3) as usize];
        x.created = NOW - (i % 1500) * DAY;
        x.hint = (i % 17 == 0) as i64;
        x.flags = (i % 40 == 0) as i64;
        put(c, &x);
        if i % 9 == 0 {
            c.execute(
                "INSERT INTO fingerprint(asset_id, blur_score) VALUES (?1, ?2)",
                params![i, (i % 5) as f64 * 0.004],
            )
            .unwrap();
        }
    }
    // Eksakte duplikater: grupper à 3 over de første 3 000 bildene.
    for g in 0..1_000 {
        c.execute(
            "INSERT INTO grp(id, kind, keeper_id, created_at) VALUES (?1, 'exact', ?2, 0)",
            params![g + 1, g * 3 + 1],
        )
        .unwrap();
        for k in 0..3 {
            c.execute(
                "INSERT INTO grp_member(grp_id, asset_id, rank) VALUES (?1, ?2, ?3)",
                params![g + 1, g * 3 + 1 + k, k],
            )
            .unwrap();
        }
    }
    c.execute("INSERT INTO album(id, name) VALUES (1, 'a')", [])
        .unwrap();
    for i in (1..=n).step_by(50) {
        c.execute(
            "INSERT INTO asset_album(asset_id, album_id) VALUES (?1, 1)",
            [i],
        )
        .unwrap();
    }
    c.execute_batch("COMMIT").unwrap();
    protect(&s, "folder", "/DCIM/3/");
    protect(&s, "album", "1");
    (s, dir)
}

fn ten_rules() -> Vec<RuleSpec> {
    vec![
        spec_of(vec![C::IsScreenshot, C::OlderThanDays(90)]),
        spec_of(vec![C::IsBlurry]),
        spec_of(vec![C::IsExactDuplicate]),
        spec_of(vec![C::OlderThanDays(1_000), C::LargerThanBytes(5_000_000)]),
        spec_of(vec![C::SmallerThanBytes(100_000)]),
        spec_of(vec![
            C::InFolder("/DCIM/7/".into()),
            C::Mime("image/png".into()),
        ]),
        spec_of(vec![C::InAlbum(1), C::IsBlurry]),
        spec_of(vec![C::Mime("image/heic".into()), C::OlderThanDays(1_200)]),
        spec_of(vec![C::IsScreenshot, C::LargerThanBytes(1_000_000)]),
        spec_of(vec![
            C::IsBlurry,
            C::IsExactDuplicate,
            C::SmallerThanBytes(4_000_000),
        ]),
    ]
}

/// Pålagt budsjett: ti aktive regler mot 50 000 bilder, hele `evaluate` (lesepass, `rule_hit` og forslag) i
/// jevn drift, dvs. når treffene stort sett er de samme som sist.
#[test]
fn evaluate_ten_rules_p95_under_50ms_on_50k_assets() {
    let (mut s, dir) = big_store(50_000);
    for (i, sp) in ten_rules().iter().enumerate() {
        save(&mut s, None, &format!("r{i}"), sp, NOW).unwrap();
    }
    let hits = count(&s, "SELECT count(*) FROM rule_hit");
    assert!(hits > 1_000, "fixturen skal gi merkbare treff, fikk {hits}");
    let mut eval_ms = Vec::new();
    let mut match_ms = Vec::new();
    for _ in 0..30 {
        let t = Instant::now();
        evaluate(&mut s, NOW).unwrap();
        eval_ms.push(t.elapsed().as_secs_f64() * 1000.0);
        let all = ten_rules();
        let refs: Vec<&RuleSpec> = all.iter().collect();
        let t = Instant::now();
        super::eval::scan(&s, &refs, NOW, None).unwrap();
        match_ms.push(t.elapsed().as_secs_f64() * 1000.0);
    }
    let (pe, pm) = (p95(eval_ms), p95(match_ms));
    eprintln!("rules: evaluate p95 = {pe:.2} ms, ett lesepass p95 = {pm:.2} ms ({hits} treff, 50 000 bilder, faktor {})", factor());
    assert!(pm < 50.0 * factor(), "lesepass p95 {pm:.2} ms");
    assert!(pe < 50.0 * factor(), "evaluate p95 {pe:.2} ms");
    // Forhåndsvisning er interaktiv og må være enda raskere enn én hel evaluering.
    let t = Instant::now();
    let p = preview(&s, &ten_rules()[3], NOW).unwrap();
    let pv = t.elapsed().as_secs_f64() * 1000.0;
    assert!(p.hits > 0 && pv < 50.0 * factor(), "preview {pv:.2} ms");
    let _ = std::fs::remove_dir_all(dir);
}

/// Sperren fra kalleren (keeper-reglenes `ProtectFolder`, docs/mvp/REGLER.md): sperrede stier gir aldri treff,
/// verken i forhåndsvisning eller evaluering, og et ventende forslag fjernes når stien blir sperret.
/// Avviste og godtatte forslag røres ikke, og ingenting endrer `asset` eller `action_log`.
#[test]
fn guard_blocks_hits_and_removes_pending_suggestions() {
    let mut inside = a(1);
    inside.source = "/Arkiv/1".into();
    let mut inside_rejected = a(2);
    inside_rejected.source = "/Arkiv/2".into();
    let mut s = store_with(vec![inside, inside_rejected, a(3)]);
    let spec = spec_of(vec![C::LargerThanBytes(10)]);
    let id = save(&mut s, None, "store", &spec, NOW).unwrap();
    assert_eq!(ids(&s, &spec), vec![1, 2, 3]);
    assert_eq!(
        count(
            &s,
            "SELECT count(*) FROM suggestion WHERE kind = 'rule' AND status = 'pending'"
        ),
        3
    );
    s.conn()
        .execute(
            "UPDATE suggestion SET status = 'rejected' WHERE asset_id = 2",
            [],
        )
        .unwrap();

    let guard = |path: &str| path.starts_with("/Arkiv/");
    let p = super::preview(&s, &spec, NOW, Some(&guard)).unwrap();
    assert_eq!((p.hits, p.sample.clone()), (1, vec![AssetId(3)]));
    assert_eq!(
        super::match_ids(&s, &spec, NOW, Some(&guard)).unwrap(),
        vec![AssetId(3)]
    );
    let rep = super::evaluate(&mut s, NOW, Some(&guard)).unwrap();
    assert_eq!((rep.hits, rep.suggestions_removed), (1, 1));
    assert_eq!(
        count(&s, "SELECT count(*) FROM suggestion WHERE asset_id = 1"),
        0,
        "ventende forslag på sperret sti fjernes"
    );
    assert_eq!(
        count(
            &s,
            "SELECT count(*) FROM suggestion WHERE asset_id = 2 AND status = 'rejected'"
        ),
        1,
        "avvist forslag røres ikke"
    );
    assert_eq!(count(&s, "SELECT count(*) FROM asset WHERE state <> 0"), 0);
    assert_eq!(count(&s, "SELECT count(*) FROM action_log"), 0);
    // Sperren gjelder også når regelen endres, slås av/på eller hentes tilbake.
    super::toggle(&mut s, id, false, NOW, Some(&guard)).unwrap();
    let rep = super::toggle(&mut s, id, true, NOW, Some(&guard)).unwrap();
    assert_eq!(rep.hits, 1);
    // Uten sperre kommer treffet tilbake (sperren er kallerens ansvar, ikke lagret her).
    let rep = evaluate(&mut s, NOW).unwrap();
    assert_eq!(rep.hits, 3);
}
