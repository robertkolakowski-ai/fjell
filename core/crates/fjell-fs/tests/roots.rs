//! Flere røtter (docs/mvp/FLERE-MAPPER.md): id-prefiks, overlapp-vern, skanning på tvers,
//! karantene per rot og id-isolasjon mellom røtter. «Tøm» per rot testes gjennom skallet.

mod common;
use common::*;
use fjell_core::model::{AssetId, AssetSource, BatchId, ChangeToken, SourceError, SourceId};
use fjell_fs::roots::{check_roots, prefixed, split, SharedKnown, PRIMARY_KEY};
use fjell_fs::{FsSource, MultiSource, QuarantineError, Reconciled, RootSet, RootsError};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

fn sid(s: &str) -> SourceId {
    SourceId(s.into())
}

fn two(tag: &str) -> (PathBuf, PathBuf) {
    let base = tempdir(tag);
    let (a, b) = (base.join("a"), base.join("b"));
    write(&a, "x.jpg", b"A-x");
    write(&a, "sub/y.jpg", b"A-y");
    write(&b, "x.jpg", b"B-x");
    write(&b, "z.png", b"B-z");
    (a, b)
}

fn source(a: &PathBuf, b: &PathBuf) -> MultiSource {
    MultiSource::new(vec![
        (1, FsSource::new(a).without_decoding()),
        (2, FsSource::new(b).without_decoding()),
    ])
    .unwrap()
}

#[test]
fn prefiks_er_entydig_og_kan_ikke_kollidere_med_forste_rot() {
    assert_eq!(prefixed(1, &sid("a/b.jpg")), sid("a/b.jpg"));
    assert_eq!(prefixed(2, &sid("a/b.jpg")), sid("/r2/a/b.jpg"));
    assert_eq!(split(&sid("a/b.jpg")), Some((PRIMARY_KEY, sid("a/b.jpg"))));
    assert_eq!(split(&sid("/r2/a/b.jpg")), Some((2, sid("a/b.jpg"))));
    // En mappe i første rot som HETER noe prefiks-lignende er fortsatt første rot.
    for own in ["r2/a.jpg", "r2:a.jpg", "r2:/a.jpg"] {
        assert_eq!(split(&sid(own)), Some((PRIMARY_KEY, sid(own))), "{own}");
    }
    for bad in [
        "/r1/a.jpg",
        "/r0/a.jpg",
        "/r02/a.jpg",
        "/r+2/a.jpg",
        "/x/a.jpg",
        "/r2/",
        "/r2",
        "/",
    ] {
        assert_eq!(split(&sid(bad)), None, "{bad}");
    }
    // Og `resolve` mot en enkelt rot avviser alltid en id med prefiks.
    assert!(fjell_fs::path::resolve(std::path::Path::new("/lib"), &sid("/r2/a.jpg")).is_err());
}

#[test]
fn overlappende_rotter_avvises_med_tydelig_feil() {
    let (a, b) = two("roots-overlap");
    assert_eq!(check_roots(&[(1, a.clone()), (2, b.clone())]), Ok(()));
    let inner = a.join("sub");
    for (x, y) in [
        (a.clone(), inner.clone()),
        (inner.clone(), a.clone()),
        (a.clone(), a.join(".")),
    ] {
        let e = check_roots(&[(1, x), (2, y)]).unwrap_err();
        assert!(matches!(e, RootsError::Overlap { .. }), "{e:?}");
        assert!(e.to_string().contains("overlapper"));
    }
    #[cfg(unix)]
    {
        // Symlenke til en mappe inni en rot er samme mappe etter kanonisering.
        let link = a.parent().unwrap().join("lenke");
        std::os::unix::fs::symlink(&inner, &link).unwrap();
        assert!(check_roots(&[(1, a.clone()), (2, link)]).is_err());
    }
    assert_eq!(check_roots(&[]), Err(RootsError::NoPrimary));
    assert_eq!(check_roots(&[(2, a.clone())]), Err(RootsError::NoPrimary));
    assert_eq!(
        check_roots(&[(1, a.clone()), (1, b.clone())]),
        Err(RootsError::BadKey(1))
    );
    let c = a.parent().unwrap().join("c");
    assert_eq!(
        check_roots(&[(1, a.clone()), (2, b.clone()), (2, c)]),
        Err(RootsError::BadKey(2))
    );
    assert!(MultiSource::new(vec![(1, FsSource::new(&a)), (2, FsSource::new(&inner))]).is_err());
    assert!(RootSet::new(&[(1, a), (2, inner)]).is_err());
}

#[test]
fn skanning_pa_tvers_gir_prefiks_bare_pa_ovrige_rotter() {
    let (a, b) = two("roots-scan");
    let all = source(&a, &b).list_changes(None).unwrap();
    let mut ids: Vec<String> = all.changed.iter().map(|m| m.source_id.0.clone()).collect();
    ids.sort();
    assert_eq!(ids, ["/r2/x.jpg", "/r2/z.png", "sub/y.jpg", "x.jpg"]);
    assert!(
        all.next_token.0.starts_with("fsm1:"),
        "{}",
        all.next_token.0
    );
    // Første rot alene gir nøyaktig samme id-er og token som en vanlig FsSource (migrering).
    let single = MultiSource::new(vec![(1, FsSource::new(&a).without_decoding())]).unwrap();
    let plain = FsSource::new(&a)
        .without_decoding()
        .list_changes(None)
        .unwrap();
    let multi = single.list_changes(None).unwrap();
    assert_eq!(multi.next_token, plain.next_token);
    assert!(multi.next_token.0.starts_with("fs1:"));
    assert_eq!(
        multi
            .changed
            .iter()
            .map(|m| &m.source_id)
            .collect::<Vec<_>>(),
        plain
            .changed
            .iter()
            .map(|m| &m.source_id)
            .collect::<Vec<_>>()
    );
    // Lesing rutes til riktig rot; samme relative sti i to røtter er to ulike filer.
    let s = source(&a, &b);
    let read = |id: &str| {
        let mut v = Vec::new();
        s.read_bytes(&sid(id)).unwrap().read_to_end(&mut v).unwrap();
        v
    };
    assert_eq!(read("x.jpg"), b"A-x");
    assert_eq!(read("/r2/x.jpg"), b"B-x");
    assert!(matches!(
        s.read_bytes(&sid("/r3/x.jpg")),
        Err(SourceError::NotFound)
    ));
    assert!(matches!(
        s.read_bytes(&sid("/r2/../a/x.jpg")),
        Err(SourceError::NotFound)
    ));
}

#[test]
fn gammelt_token_fra_en_rot_gir_full_listing_av_ny_rot_og_ingen_fjerninger_der() {
    let (a, b) = two("roots-token");
    let old = FsSource::new(&a)
        .without_decoding()
        .list_changes(None)
        .unwrap()
        .next_token;
    let known: SharedKnown = Arc::new(|| vec![sid("x.jpg"), sid("sub/y.jpg")]);
    let s = source(&a, &b).with_known_ids(known);
    let inc = s.list_changes(Some(&old)).unwrap();
    let ids: Vec<&str> = inc.changed.iter().map(|m| m.source_id.0.as_str()).collect();
    assert_eq!(ids, ["/r2/x.jpg", "/r2/z.png"], "bare den nye roten er ny");
    assert!(inc.removed.is_empty());
    // Neste runde: ingenting nytt, og token med begge røtter leses tilbake.
    let again = s.list_changes(Some(&inc.next_token)).unwrap();
    assert!(again.changed.is_empty() && again.removed.is_empty());
    assert!(s.list_changes(Some(&ChangeToken("tull".into()))).is_err());
}

#[test]
fn fjernet_fil_meldes_bare_i_sin_rot_og_frakoblet_rot_melder_ingenting() {
    let (a, b) = two("roots-removed");
    let known_ids = Arc::new(Mutex::new(vec![
        sid("x.jpg"),
        sid("sub/y.jpg"),
        sid("/r2/x.jpg"),
        sid("/r2/z.png"),
    ]));
    let k = known_ids.clone();
    let known: SharedKnown = Arc::new(move || k.lock().unwrap().clone());
    let s = source(&a, &b).with_known_ids(known.clone());
    let t = s.list_changes(None).unwrap().next_token;
    std::fs::remove_file(b.join("x.jpg")).unwrap();
    let inc = s.list_changes(Some(&t)).unwrap();
    assert_eq!(inc.removed, [sid("/r2/x.jpg")], "a/x.jpg finnes fortsatt");
    // Rot 2 frakoblet: ingenting derfra er «fjernet», og første rot skannes som vanlig.
    let gone = b.with_file_name("b-borte");
    std::fs::rename(&b, &gone).unwrap();
    let inc = s.list_changes(Some(&t)).unwrap();
    assert!(inc.removed.is_empty(), "{:?}", inc.removed);
    assert_eq!(inc.next_token, t, "tokenet for frakoblet rot står stille");
    // Alle røtter frakoblet: Unavailable, aldri tom liste.
    std::fs::rename(&a, a.with_file_name("a-borte")).unwrap();
    assert!(matches!(
        s.list_changes(Some(&t)),
        Err(SourceError::Unavailable)
    ));
    assert!(matches!(
        s.list_changes(None),
        Err(SourceError::Unavailable)
    ));
    // Rot tatt ut av biblioteket: radene dens meldes fjernet ved inkrementell skanning.
    std::fs::rename(a.with_file_name("a-borte"), &a).unwrap();
    let only_a = MultiSource::new(vec![(1, FsSource::new(&a).without_decoding())])
        .unwrap()
        .with_known_ids(known);
    let mut removed = only_a.list_changes(Some(&t)).unwrap().removed;
    removed.sort();
    assert_eq!(removed, [sid("/r2/x.jpg"), sid("/r2/z.png")]);
    assert!(only_a.list_changes(None).unwrap().removed.is_empty());
}

fn items(v: &[(u64, &str)]) -> Vec<(AssetId, SourceId)> {
    v.iter().map(|(i, s)| (AssetId(*i), sid(s))).collect()
}

#[test]
fn karantene_per_rot_stage_unstage_og_id_isolasjon() {
    let (a, b) = two("roots-q");
    let before = (snapshot(&a), snapshot(&b));
    let set = RootSet::new(&[(1, a.clone()), (2, b.clone())]).unwrap();
    let r = set.stage(
        BatchId(5),
        &items(&[
            (1, "x.jpg"),
            (2, "/r2/x.jpg"),
            (3, "/r9/x.jpg"),
            (4, "/r2/../a/sub/y.jpg"),
        ]),
    );
    let res = |id: u64| r.iter().find(|(i, _)| i.0 == id).unwrap().1.clone();
    assert_eq!((res(1), res(2)), (Ok(()), Ok(())));
    assert_eq!(res(3), Err(QuarantineError::NotFound), "ukjent rot");
    assert_eq!(
        res(4),
        Err(QuarantineError::NotFound),
        "kan ikke forlate roten"
    );
    // Hver fil ligger i SIN rots karantene.
    assert!(a.join(".fjell-karantene/5/1.jpg").is_file());
    assert!(b.join(".fjell-karantene/5/2.jpg").is_file());
    assert!(!a.join(".fjell-karantene/5/2.jpg").exists());
    assert!(a.join("sub/y.jpg").is_file());
    let mut staged = set.staged();
    staged.sort();
    assert_eq!(
        staged,
        [
            (1, AssetId(1), sid("x.jpg")),
            (2, AssetId(2), sid("/r2/x.jpg"))
        ]
    );
    // Oppslag rutes på indeksens sti: id 1 (rot A) finnes aldri via rot B, og omvendt.
    assert!(set.locate_for(AssetId(1), &sid("x.jpg")).is_some());
    assert_eq!(set.locate_for(AssetId(1), &sid("/r2/x.jpg")), None);
    assert_eq!(set.locate_for(AssetId(2), &sid("x.jpg")), None);
    assert_eq!(set.locate_for(AssetId(2), &sid("/r3/x.jpg")), None);
    // Angre med feil rot i indeksstien flytter ingenting.
    let wrong = set.unstage(&items(&[(1, "/r2/x.jpg"), (2, "x.jpg")]));
    assert!(
        wrong
            .iter()
            .all(|(_, r)| *r == Err(QuarantineError::NotFound)),
        "{wrong:?}"
    );
    assert_eq!(set.staged().len(), 2);
    let back = set.unstage(&items(&[(1, "x.jpg"), (2, "/r2/x.jpg")]));
    assert_eq!(back[0].1, Ok(sid("x.jpg")));
    assert_eq!(back[1].1, Ok(sid("/r2/x.jpg")));
    assert_eq!(
        (snapshot(&a), snapshot(&b)),
        before,
        "innhold og stier som før"
    );
}

#[test]
fn gjenoppretting_med_kollisjon_gir_nytt_navn_med_rotprefiks() {
    let (a, b) = two("roots-coll");
    let set = RootSet::new(&[(1, a), (2, b.clone())]).unwrap();
    set.stage(BatchId(1), &items(&[(2, "/r2/x.jpg")]));
    write(&b, "x.jpg", b"ny fil i veien");
    let back = set.unstage(&items(&[(2, "/r2/x.jpg")]));
    assert_eq!(back[0].1, Ok(sid("/r2/x (gjenopprettet).jpg")));
    assert_eq!(std::fs::read(b.join("x.jpg")).unwrap(), b"ny fil i veien");
    assert_eq!(
        std::fs::read(b.join("x (gjenopprettet).jpg")).unwrap(),
        b"B-x"
    );
}

#[test]
fn avstemming_per_rot_og_frakoblet_rot_endrer_ingenting() {
    let (a, b) = two("roots-rec");
    let set = RootSet::new(&[(1, a.clone()), (2, b.clone())]).unwrap();
    set.stage(
        BatchId(1),
        &items(&[(1, "x.jpg"), (2, "/r2/x.jpg"), (3, "/r2/z.png")]),
    );
    // Indeksen sier: 1 og 2 er fortsatt trashed, 3 er ikke (avbrutt angre), 9 har ukjent rot.
    let rec = set.reconcile(&items(&[(1, "x.jpg"), (2, "/r2/x.jpg"), (9, "/r7/q.jpg")]));
    let got = |id: u64| rec.iter().find(|(i, _)| i.0 == id).unwrap().1.clone();
    assert_eq!(got(1), Reconciled::StillStaged);
    assert_eq!(got(2), Reconciled::StillStaged);
    assert_eq!(got(3), Reconciled::Restored(sid("/r2/z.png")));
    assert_eq!(got(9), Reconciled::Unavailable);
    assert!(b.join("z.png").is_file());
    // Rot B frakoblet: dens bilder er Unavailable (aldri Missing), rot A avstemmes som vanlig.
    let gone = b.with_file_name("b-borte");
    std::fs::rename(&b, &gone).unwrap();
    let rec = set.reconcile(&items(&[(1, "x.jpg"), (2, "/r2/x.jpg")]));
    let got = |id: u64| rec.iter().find(|(i, _)| i.0 == id).unwrap().1.clone();
    assert_eq!(got(1), Reconciled::StillStaged);
    assert_eq!(got(2), Reconciled::Unavailable);
    let st = set.stage(BatchId(2), &items(&[(4, "/r2/z.png")]));
    assert_eq!(st[0].1, Err(QuarantineError::Unavailable));
    assert!(gone.join(".fjell-karantene/1/2.jpg").is_file(), "urørt");
}
