//! Fjernede filer ved inkrementell skanning (T11) mot ekte filsystem, og at frakoblet rot aldri
//! gir `removed` (T27).

mod common;
use common::*;
use fjell_core::model::{AssetSource, ChangeToken, SourceError, SourceId};
use fjell_fs::FsSource;
use std::fs;
use std::sync::{Arc, Mutex};

fn ids(v: &[SourceId]) -> Vec<String> {
    let mut o: Vec<String> = v.iter().map(|s| s.0.clone()).collect();
    o.sort();
    o
}

/// Kjente id-er som en liste testen kan endre (stand-in for indeksens aktive bilder).
fn known(v: &[&str]) -> (Arc<Mutex<Vec<SourceId>>>, fjell_fs::source::KnownIds) {
    let a = Arc::new(Mutex::new(
        v.iter().map(|s| SourceId((*s).into())).collect::<Vec<_>>(),
    ));
    let b = a.clone();
    (a, Box::new(move || b.lock().unwrap().clone()))
}

#[test]
fn incremental_scan_reports_removed_files() {
    let root = tempdir("rm-inc");
    write(&root, "a.jpg", &jpeg(20, 20, None));
    write(&root, "sub/b.jpg", &jpeg(20, 20, None));
    write(&root, "c.jpg", &jpeg(20, 20, None));
    let (list, f) = known(&["a.jpg", "sub/b.jpg", "c.jpg"]);
    let src = FsSource::new(&root).with_known_ids(f);
    let first = src.list_changes(None).unwrap();
    assert!(
        first.removed.is_empty(),
        "full listing melder ingen fjerninger"
    );
    fs::remove_file(root.join("sub/b.jpg")).unwrap();
    let second = src.list_changes(Some(&first.next_token)).unwrap();
    assert_eq!(ids(&second.removed), vec!["sub/b.jpg"]);
    assert!(second.changed.is_empty());
    // Når indeksen har avstemt, meldes den ikke på nytt.
    list.lock().unwrap().retain(|s| s.0 != "sub/b.jpg");
    let third = src.list_changes(Some(&second.next_token)).unwrap();
    assert!(third.removed.is_empty());
}

#[test]
fn known_ids_from_index_survive_restart() {
    let root = tempdir("rm-known");
    write(&root, "a.jpg", &jpeg(20, 20, None));
    write(&root, "b.jpg", &jpeg(20, 20, None));
    // Nytt objekt uten minne (som etter omstart), men med indeksens kjente id-er.
    let (_l, f) = known(&["a.jpg", "b.jpg", "gone.jpg"]);
    let src = FsSource::new(&root).with_known_ids(f);
    let cs = src
        .list_changes(Some(&ChangeToken("fs1:0".into())))
        .unwrap();
    assert_eq!(ids(&cs.removed), vec!["gone.jpg"]);
}

#[test]
fn without_known_list_first_incremental_scan_reports_nothing() {
    let root = tempdir("rm-cold");
    write(&root, "a.jpg", &jpeg(20, 20, None));
    let src = FsSource::new(&root);
    let cs = src
        .list_changes(Some(&ChangeToken("fs1:0".into())))
        .unwrap();
    assert!(cs.removed.is_empty());
}

#[test]
fn offline_or_emptied_root_is_unavailable_never_removed() {
    let root = tempdir("rm-offline");
    write(&root, "a.jpg", &jpeg(20, 20, None));
    let (_l, f) = known(&["a.jpg"]);
    let src = FsSource::new(&root).with_known_ids(f);
    let first = src.list_changes(None).unwrap();
    // Roten borte (frakoblet disk).
    let moved = root.with_extension("away");
    fs::rename(&root, &moved).unwrap();
    assert_eq!(
        src.list_changes(Some(&first.next_token)).unwrap_err(),
        SourceError::Unavailable
    );
    // Tom monteringsmappe: roten finnes, men er tom mens indeksen kjenner filer.
    fs::create_dir_all(&root).unwrap();
    assert_eq!(
        src.list_changes(Some(&first.next_token)).unwrap_err(),
        SourceError::Unavailable
    );
    // Tilbake: ingenting er fjernet.
    fs::remove_dir_all(&root).unwrap();
    fs::rename(&moved, &root).unwrap();
    let back = src.list_changes(Some(&first.next_token)).unwrap();
    assert!(back.removed.is_empty());
}

#[test]
fn quarantined_files_are_missing_from_the_listing() {
    // Karantene er en flytting i roten; filen er borte fra listingen. Derfor må `with_known_ids` bare
    // gi AKTIVE bilder (papirkurven er med vilje i karantenen); her gjør testen det motsatte for å vise hvorfor.
    let root = tempdir("rm-quar");
    write(&root, "a.jpg", &jpeg(20, 20, None));
    write(&root, "b.jpg", &jpeg(20, 20, None));
    let (_l, f) = known(&["a.jpg", "b.jpg"]);
    let src = FsSource::new(&root).with_known_ids(f);
    let first = src.list_changes(None).unwrap();
    write(&root, ".fjell-karantene/b.jpg", &jpeg(20, 20, None));
    fs::remove_file(root.join("b.jpg")).unwrap();
    let cs = src.list_changes(Some(&first.next_token)).unwrap();
    assert_eq!(ids(&cs.removed), vec!["b.jpg"]);
}
