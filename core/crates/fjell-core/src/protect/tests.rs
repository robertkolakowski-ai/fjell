//! Tester for `protect`: sette/fjerne/angre, mappegrenser, album, person (med sammenslåing),
//! flere omfang per bilde, ingen sideeffekt på bilder, og ytelse.

use super::*;
use crate::model::{AssetId, PersonId, ProtectScope, State};
use rusqlite::params;

fn store_with(sources: &[&str]) -> Store {
    let s = Store::open_in_memory().unwrap();
    for (i, src) in sources.iter().enumerate() {
        s.conn()
            .execute(
                "INSERT INTO asset(id, source_id, kind, created_at, mod_token, width, height, bytes, mime) \
                 VALUES (?1, ?2, 'photo', 1, 'm', 1, 1, 10, 'x')",
                params![i as i64 + 1, src],
            )
            .unwrap();
    }
    s
}

fn a(n: u64) -> AssetId {
    AssetId(n)
}

fn folder(p: &str) -> ProtectScope {
    ProtectScope::Folder(p.to_string())
}

fn add_person(s: &Store, id: i64, merged_into: Option<i64>) {
    s.conn()
        .execute(
            "INSERT INTO person(id, created_at, merged_into) VALUES (?1, 1, ?2)",
            params![id, merged_into],
        )
        .unwrap();
}

fn add_face(s: &Store, asset: i64, person: i64) {
    s.conn()
        .execute(
            "INSERT INTO face_embedding(asset_id, person_id, box_x, box_y, box_w, box_h, quality, embedding) \
             VALUES (?1, ?2, 0, 0, 100, 100, 1.0, x'00')",
            params![asset, person],
        )
        .unwrap();
}

#[test]
fn smoke_scope_coding_roundtrips() {
    let s = ProtectScope::Asset(AssetId(9));
    assert_eq!(
        ProtectScope::from_parts(s.scope_code(), &s.target()),
        Some(s)
    );
    assert_eq!(ProtectScope::from_parts("tag", "x"), None);
}

#[test]
fn set_is_idempotent_and_unset_can_be_undone() {
    let s = store_with(&["a", "b"]);
    let sc = ProtectScope::Asset(a(1));
    assert!(set(&s, &sc, 100).unwrap());
    assert!(!set(&s, &sc, 200).unwrap(), "andre gang endrer ingenting");
    assert_eq!(list(&s).unwrap(), vec![(sc.clone(), 100)]);

    let removal = unset(&s, &sc).unwrap().unwrap();
    assert_eq!(removal.created_at, 100);
    assert!(list(&s).unwrap().is_empty());
    assert_eq!(unset(&s, &sc).unwrap(), None, "fjerne to ganger er ufarlig");

    assert!(undo_unset(&s, &removal).unwrap());
    assert_eq!(
        list(&s).unwrap(),
        vec![(sc, 100)],
        "opprinnelig tidspunkt bevart"
    );
}

#[test]
fn unknown_targets_and_empty_folder_are_rejected() {
    let s = store_with(&["a"]);
    for sc in [
        ProtectScope::Asset(a(99)),
        ProtectScope::Album(7),
        ProtectScope::Person(PersonId(7)),
    ] {
        assert!(matches!(
            set(&s, &sc, 1),
            Err(ProtectError::UnknownTarget(_))
        ));
    }
    for p in ["", "  ", "/", "\\"] {
        assert!(matches!(
            set(&s, &folder(p), 1),
            Err(ProtectError::EmptyFolder)
        ));
    }
    assert_eq!(s.protection_count().unwrap(), 0);
}

#[test]
fn folder_is_normalized_and_matches_on_folder_boundary() {
    let s = store_with(&[
        "photos/2020/a.jpg",
        "photos2/b.jpg",
        "photos/c.jpg",
        "photos",
    ]);
    assert!(set(&s, &folder(" photos/ "), 1).unwrap());
    assert_eq!(list(&s).unwrap()[0].0, folder("photos"));
    let eff = effective(&s, &[a(1), a(2), a(3), a(4)]).unwrap();
    assert_eq!(eff.ids(), vec![a(1), a(3), a(4)]);
    assert!(!eff.is_protected(a(2)), "photos2 er en annen mappe");
    assert_eq!(eff.scopes(a(1)), &[folder("photos")]);
}

#[test]
fn windows_separators_work_in_folder_scope() {
    let s = store_with(&["C:\\Bilder\\x.jpg", "C:\\Bilderfoto\\y.jpg"]);
    set(&s, &folder("C:\\Bilder\\"), 1).unwrap();
    assert_eq!(effective(&s, &[a(1), a(2)]).unwrap().ids(), vec![a(1)]);
}

#[test]
fn album_scope_covers_members() {
    let s = store_with(&["a", "b", "c"]);
    s.conn()
        .execute(
            "INSERT INTO album(id, name, shared) VALUES (5, 'Ferie', 0)",
            [],
        )
        .unwrap();
    s.conn()
        .execute("INSERT INTO asset_album VALUES (1, 5), (3, 5)", [])
        .unwrap();
    set(&s, &ProtectScope::Album(5), 1).unwrap();
    assert_eq!(
        effective(&s, &[a(1), a(2), a(3)]).unwrap().ids(),
        vec![a(1), a(3)]
    );
}

#[test]
fn person_scope_follows_merges_transitively() {
    let s = store_with(&["a", "b", "c", "d"]);
    add_person(&s, 1, None);
    add_person(&s, 2, Some(1));
    add_person(&s, 3, Some(2));
    add_person(&s, 4, None);
    add_face(&s, 1, 1);
    add_face(&s, 2, 3); // 3 -> 2 -> 1
    add_face(&s, 3, 4); // annen person
    set(&s, &ProtectScope::Person(PersonId(1)), 1).unwrap();
    let eff = effective(&s, &[a(1), a(2), a(3), a(4)]).unwrap();
    assert_eq!(eff.ids(), vec![a(1), a(2)]);
    assert_eq!(eff.scopes(a(2)), &[ProtectScope::Person(PersonId(1))]);
}

#[test]
fn person_scope_survives_face_index_wipe_dormant() {
    let s = store_with(&["a"]);
    add_person(&s, 1, None);
    add_face(&s, 1, 1);
    set(&s, &ProtectScope::Person(PersonId(1)), 1).unwrap();
    s.conn().execute("DELETE FROM face_embedding", []).unwrap();
    s.conn().execute("DELETE FROM person", []).unwrap();
    assert_eq!(s.protection_count().unwrap(), 1, "raden bevares");
    assert!(effective(&s, &[a(1)]).unwrap().ids().is_empty());
}

#[test]
fn asset_can_have_several_scopes() {
    let s = store_with(&["x/a.jpg"]);
    set(&s, &ProtectScope::Asset(a(1)), 1).unwrap();
    set(&s, &folder("x"), 2).unwrap();
    let eff = effective(&s, &[a(1)]).unwrap();
    assert_eq!(eff.count(), 1);
    assert_eq!(eff.scopes(a(1)).len(), 2);
}

#[test]
fn protect_and_unprotect_never_touch_assets() {
    let s = store_with(&["a", "b"]);
    let before: Vec<(i64, i64, i64)> = {
        let mut st = s
            .conn()
            .prepare("SELECT id, state, protect_flags FROM asset ORDER BY id")
            .unwrap();
        st.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
            .unwrap()
            .map(|r| r.unwrap())
            .collect()
    };
    let sc = ProtectScope::Asset(a(1));
    set(&s, &sc, 1).unwrap();
    let r = unset(&s, &sc).unwrap().unwrap();
    undo_unset(&s, &r).unwrap();
    let after: Vec<(i64, i64, i64)> = {
        let mut st = s
            .conn()
            .prepare("SELECT id, state, protect_flags FROM asset ORDER BY id")
            .unwrap();
        st.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
            .unwrap()
            .map(|r| r.unwrap())
            .collect()
    };
    assert_eq!(before, after);
    assert_eq!(s.asset_row(a(1)).unwrap().unwrap().state, State::Active);
}

#[test]
fn effective_for_5000_assets_is_fast() {
    let s = Store::open_in_memory().unwrap();
    s.conn().execute_batch("BEGIN").unwrap();
    for i in 1..=5000i64 {
        s.conn()
            .execute(
                "INSERT INTO asset(id, source_id, kind, created_at, mod_token, width, height, bytes, mime) \
                 VALUES (?1, ?2, 'photo', 1, 'm', 1, 1, 10, 'x')",
                params![i, format!("dir{}/f{i}.jpg", i % 50)],
            )
            .unwrap();
    }
    s.conn().execute_batch("COMMIT").unwrap();
    for d in 0..5 {
        set(&s, &folder(&format!("dir{d}")), 1).unwrap();
    }
    let ids: Vec<AssetId> = (1..=5000).map(AssetId).collect();
    let t = std::time::Instant::now();
    let eff = effective(&s, &ids).unwrap();
    assert!(t.elapsed().as_millis() < 100, "tok {:?}", t.elapsed());
    assert_eq!(eff.count(), 500);
}
