//! Tester for `people`: indeks av som standard, ingen analyse når den er av, klynging, sammenslåing og splitt
//! (angrebart), beskyttelse som visning og «Slett ansiktsindeks».

use super::*;
use crate::model::{
    AnalyzeError, Embedding, FaceBox, FaceObservation, NullFaceAnalyzer, PersonId, Thumb, ThumbFmt,
};

fn store(n: u64) -> Store {
    let s = Store::open_in_memory().unwrap();
    for i in 1..=n {
        s.conn()
            .execute(
                "INSERT INTO asset(id, source_id, kind, created_at, mod_token, width, height, bytes, mime) \
                 VALUES (?1, ?2, 'photo', 1, 'm', 4, 4, 3, 'image/jpeg')",
                rusqlite::params![i as i64, format!("s{i}")],
            )
            .unwrap();
    }
    s
}

fn thumb(tag: u8) -> Thumb {
    Thumb::new(1, 1, ThumbFmt::Gray8, vec![tag]).unwrap()
}

fn obs(v: &[f32], q: f32) -> FaceObservation {
    FaceObservation {
        bbox: FaceBox {
            x: 100,
            y: 100,
            w: 200,
            h: 200,
        },
        quality: q,
        embedding: Embedding(v.to_vec()),
    }
}

/// Tre bilder: to av person A (retning x), ett av person B (retning y).
fn corpus() -> MemFaceAnalyzer {
    MemFaceAnalyzer::new()
        .with_faces(vec![1], vec![obs(&[1.0, 0.0], 0.8)])
        .with_faces(vec![2], vec![obs(&[0.98, 0.1], 0.9), obs(&[0.0, 1.0], 0.7)])
        .with_faces(vec![3], vec![obs(&[0.97, -0.1], 0.6)])
}

fn indexed() -> Store {
    let mut s = store(3);
    s.set_face_index_enabled(true).unwrap();
    let a = corpus();
    for i in 1..=3u8 {
        index_asset(&mut s, &a, AssetId(i as u64), &thumb(i), 1).unwrap();
    }
    s
}

#[test]
fn index_is_off_by_default_and_never_calls_analyzer() {
    let mut s = store(1);
    let a = corpus();
    let r = index_asset(&mut s, &a, AssetId(1), &thumb(1), 1);
    assert!(matches!(r, Err(PeopleError::Disabled)));
    assert_eq!(a.calls(), 0, "ingen bilder analyseres når indeksen er av");
    assert_eq!(s.face_count().unwrap(), 0);
    // klynging gjør heller ingenting
    assert_eq!(
        cluster_new_faces(&mut s, &PeopleParams::default(), 1).unwrap(),
        ClusterStats::default()
    );
}

#[test]
fn unavailable_analyzer_is_never_no_faces() {
    let mut s = store(1);
    s.set_face_index_enabled(true).unwrap();
    let r = index_asset(&mut s, &NullFaceAnalyzer, AssetId(1), &thumb(1), 1);
    assert!(matches!(
        r,
        Err(PeopleError::Analyze(AnalyzeError::Unavailable))
    ));
    let failing = MemFaceAnalyzer::new().failing(AnalyzeError::Failed("x".into()));
    let r = index_asset(&mut s, &failing, AssetId(1), &thumb(1), 1);
    assert!(matches!(
        r,
        Err(PeopleError::Analyze(AnalyzeError::Failed(_)))
    ));
    assert_eq!(s.face_count().unwrap(), 0);
}

#[test]
fn indexing_is_idempotent_per_algo_version() {
    let mut s = store(2);
    s.set_face_index_enabled(true).unwrap();
    let a = corpus();
    assert_eq!(
        index_asset(&mut s, &a, AssetId(2), &thumb(2), 1).unwrap(),
        2
    );
    assert_eq!(
        index_asset(&mut s, &a, AssetId(2), &thumb(2), 1).unwrap(),
        0
    );
    assert_eq!(a.calls(), 1, "allerede analysert: ikke kall igjen");
    // bilde uten ansikter (ukjent miniatyr) gir 0, ikke feil
    assert_eq!(
        index_asset(&mut s, &a, AssetId(1), &thumb(99), 1).unwrap(),
        0
    );
}

#[test]
fn clusters_into_people_and_keeps_existing_assignments() {
    let mut s = indexed();
    let st = cluster_new_faces(&mut s, &PeopleParams::default(), 10).unwrap();
    assert_eq!(
        st,
        ClusterStats {
            assigned: 4,
            new_people: 2
        }
    );
    let people = list(&s).unwrap();
    assert_eq!(people.len(), 2);
    assert_eq!((people[0].face_count, people[0].asset_count), (3, 3));
    assert_eq!(people[0].cover, Some(AssetId(2)), "høyest kvalitet");
    assert_eq!((people[1].face_count, people[1].asset_count), (1, 1));
    assert!(people.iter().all(|p| p.name.is_none() && !p.protected));
    // ny kjøring uten nye ansikter endrer ingenting
    assert_eq!(
        cluster_new_faces(&mut s, &PeopleParams::default(), 11).unwrap(),
        ClusterStats::default()
    );
}

#[test]
fn new_face_joins_existing_person_even_after_merge() {
    let mut s = indexed();
    cluster_new_faces(&mut s, &PeopleParams::default(), 10).unwrap();
    let people = list(&s).unwrap();
    let (a, b) = (people[0].id, people[1].id);
    assert_eq!(merge(&mut s, a, b).unwrap(), MergeOutcome::Merged(a));
    // et nytt bilde som ligner B havner hos den sammenslåtte personen, ikke i en ny
    s.conn()
        .execute(
            "INSERT INTO asset(id, source_id, kind, created_at, mod_token, width, height, bytes, mime) \
             VALUES (9, 's9', 'photo', 1, 'm', 4, 4, 3, 'image/jpeg')",
            [],
        )
        .unwrap();
    let an = MemFaceAnalyzer::new().with_faces(vec![9], vec![obs(&[0.05, 1.0], 0.5)]);
    index_asset(&mut s, &an, AssetId(9), &thumb(9), 1).unwrap();
    let st = cluster_new_faces(&mut s, &PeopleParams::default(), 20).unwrap();
    assert_eq!(
        st,
        ClusterStats {
            assigned: 1,
            new_people: 0
        }
    );
    let people = list(&s).unwrap();
    assert_eq!(people.len(), 1);
    assert_eq!(people[0].face_count, 5);
}

#[test]
fn merge_is_undoable_and_reports_outcomes() {
    let mut s = indexed();
    cluster_new_faces(&mut s, &PeopleParams::default(), 10).unwrap();
    let people = list(&s).unwrap();
    let (a, b) = (people[0].id, people[1].id);
    assert_eq!(merge(&mut s, a, b).unwrap(), MergeOutcome::Merged(a));
    assert_eq!(list(&s).unwrap().len(), 1);
    assert_eq!(merge(&mut s, a, b).unwrap(), MergeOutcome::SamePerson);
    assert_eq!(
        merge(&mut s, a, PersonId(999)).unwrap(),
        MergeOutcome::Unknown
    );
    assert!(unmerge(&mut s, b).unwrap());
    let back = list(&s).unwrap();
    assert_eq!(back.len(), 2);
    assert_eq!(back[0].face_count + back[1].face_count, 4);
}

#[test]
fn split_and_undo_via_merge() {
    let mut s = indexed();
    cluster_new_faces(&mut s, &PeopleParams::default(), 10).unwrap();
    let a = list(&s).unwrap()[0].id;
    let faces: Vec<u64> = s
        .faces_all()
        .unwrap()
        .iter()
        .filter(|f| f.asset == AssetId(3))
        .map(|f| f.id)
        .collect();
    let q = split(&mut s, a, &faces, 30).unwrap().unwrap();
    assert_eq!(person_assets(&s, q).unwrap(), vec![AssetId(3)]);
    assert_eq!(person_assets(&s, a).unwrap(), vec![AssetId(1), AssetId(2)]);
    assert_eq!(merge(&mut s, a, q).unwrap(), MergeOutcome::Merged(a));
    assert_eq!(
        person_assets(&s, a).unwrap(),
        vec![AssetId(1), AssetId(2), AssetId(3)]
    );
}

#[test]
fn naming_validates_length() {
    let mut s = indexed();
    cluster_new_faces(&mut s, &PeopleParams::default(), 10).unwrap();
    let a = list(&s).unwrap()[0].id;
    assert!(set_name(&mut s, a, Some("Kari")).unwrap());
    assert_eq!(list(&s).unwrap()[0].name.as_deref(), Some("Kari"));
    assert!(!set_name(&mut s, a, Some(&"x".repeat(MAX_NAME_CHARS + 1))).unwrap());
    assert_eq!(list(&s).unwrap()[0].name.as_deref(), Some("Kari"));
    assert!(set_name(&mut s, a, None).unwrap());
    assert_eq!(list(&s).unwrap()[0].name, None);
}

#[test]
fn trashed_assets_are_not_counted_or_clustered() {
    let mut s = indexed();
    s.conn()
        .execute("UPDATE asset SET state = 1 WHERE id = 3", [])
        .unwrap();
    cluster_new_faces(&mut s, &PeopleParams::default(), 10).unwrap();
    let people = list(&s).unwrap();
    assert_eq!(people.iter().map(|p| p.asset_count).sum::<u64>(), 3);
}

#[test]
fn wipe_clears_faces_and_people_but_not_images_or_protection() {
    let mut s = indexed();
    cluster_new_faces(&mut s, &PeopleParams::default(), 10).unwrap();
    let a = list(&s).unwrap()[0].id;
    s.conn()
        .execute(
            "INSERT INTO protection(scope, target, created_at) VALUES ('person', ?1, 1)",
            [a.0.to_string()],
        )
        .unwrap();
    assert!(list(&s).unwrap()[0].protected);
    assert_eq!(wipe(&mut s).unwrap(), 4);
    assert!(list(&s).unwrap().is_empty());
    assert_eq!(s.face_count().unwrap(), 0);
    let kept: i64 = s
        .conn()
        .query_row("SELECT count(*) FROM asset WHERE state = 0", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(kept, 3);
    let prot: i64 = s
        .conn()
        .query_row("SELECT count(*) FROM protection", [], |r| r.get(0))
        .unwrap();
    assert_eq!(prot, 1);
    // indeksen kan bygges opp igjen
    let an = corpus();
    index_asset(&mut s, &an, AssetId(1), &thumb(1), 1).unwrap();
    assert_eq!(s.face_count().unwrap(), 1);
}

#[test]
fn clustering_500_faces_is_fast() {
    let mut s = store(500);
    s.set_face_index_enabled(true).unwrap();
    for i in 1..=500u64 {
        let k = (i % 20) as f32;
        let v = [k.cos(), k.sin(), (k * 0.5).cos(), 0.3];
        s.put_faces(AssetId(i), &[obs(&v, 0.5)], 1).unwrap();
    }
    let t = std::time::Instant::now();
    cluster_new_faces(&mut s, &PeopleParams { threshold: 0.995 }, 1).unwrap();
    assert!(t.elapsed().as_millis() < 1000, "{:?}", t.elapsed());
    assert!(!list(&s).unwrap().is_empty());
}
