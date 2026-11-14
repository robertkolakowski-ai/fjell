//! Flere mapper i ett bibliotek, gjennom `Shell` (docs/mvp/FLERE-MAPPER.md): migrering av énrots-
//! indeks, karantene/angre/gjenoppretting/«Tøm»/navnekollisjon per rot, id-isolasjon mellom røtter,
//! overlapp-vern, skysperre per rot, fjerning av rot, frakoblet rot og tall per rot.

use crate::dto::DecisionDto;
use crate::{config, Shell};
use fjell_core::model::AssetId;
use fjell_fs::manifest::{Entry, Manifest};
use fjell_fs::{FakeRecycleBin, RecycleBin, QUARANTINE_DIR};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

const DUP_A: &[u8] = b"duplikat-i-forste-rot";
const DUP_B: &[u8] = b"duplikat-i-andre-rot!";

struct Env {
    root: PathBuf,
    a: PathBuf,
    b: PathBuf,
    data: PathBuf,
    bin: Arc<FakeRecycleBin>,
    sh: Shell,
}

impl Drop for Env {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn shell(data: &Path, bin: &Arc<FakeRecycleBin>) -> Shell {
    Shell::with_data_dir(data, bin.clone() as Arc<dyn RecycleBin>)
}

/// Rot A (a.jpg = b.jpg, p.jpg unik) og rot B i mappen `b_name` (p.jpg = q.jpg, c.jpg unik).
/// Samme relative navn `p.jpg` finnes i begge røtter med ulikt innhold. Ingenting er åpnet.
fn dirs(tag: &str, b_name: &str) -> Env {
    let root = std::env::temp_dir().join(format!("fjell-roots-{tag}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    let (a, b, data) = (
        root.join("bibliotek"),
        root.join(b_name),
        root.join("appdata"),
    );
    for d in [&a, &b, &data] {
        fs::create_dir_all(d).unwrap();
    }
    fs::write(a.join("a.jpg"), DUP_A).unwrap();
    fs::write(a.join("b.jpg"), DUP_A).unwrap();
    fs::write(a.join("p.jpg"), b"unik i A, samme navn som i B").unwrap();
    fs::write(b.join("p.jpg"), DUP_B).unwrap();
    fs::write(b.join("q.jpg"), DUP_B).unwrap();
    fs::write(b.join("c.jpg"), b"unik i B").unwrap();
    let bin = Arc::new(FakeRecycleBin {
        bin_dir: Some(root.join("papirkurv")),
        ..Default::default()
    });
    let sh = shell(&data, &bin);
    Env {
        root,
        a,
        b,
        data,
        bin,
        sh,
    }
}

/// Bibliotek med to røtter, åpnet og skannet: ett duplikatkort i hver rot.
fn env(tag: &str) -> Env {
    let e = dirs(tag, "ekstra");
    e.sh.set_library(e.a.to_str().unwrap()).unwrap();
    let roots = e.sh.add_library_root(e.b.to_str().unwrap()).unwrap();
    assert_eq!(roots.iter().map(|r| r.key).collect::<Vec<_>>(), [1, 2]);
    e.scan();
    e
}

fn files(dir: &Path) -> BTreeMap<String, Vec<u8>> {
    fn walk(base: &Path, d: &Path, out: &mut BTreeMap<String, Vec<u8>>) {
        for e in fs::read_dir(d).into_iter().flatten().filter_map(|e| e.ok()) {
            let p = e.path();
            if p.is_dir() {
                if p.file_name().is_some_and(|n| n != QUARANTINE_DIR) {
                    walk(base, &p, out);
                }
            } else {
                let rel = p.strip_prefix(base).unwrap().to_string_lossy().into_owned();
                out.insert(rel, fs::read(&p).unwrap());
            }
        }
    }
    let mut out = BTreeMap::new();
    walk(dir, dir, &mut out);
    out
}

/// Bildefiler i mappens karantene (uten manifest og README).
fn quarantined(dir: &Path) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = fs::read_dir(dir.join(QUARANTINE_DIR))
        .into_iter()
        .flatten()
        .filter_map(|b| b.ok())
        .flat_map(|b| fs::read_dir(b.path()).into_iter().flatten())
        .filter_map(|f| f.ok().map(|f| f.path()))
        .filter(|p| {
            p.file_name()
                .is_some_and(|n| n != "manifest.json" && n != "README.txt")
        })
        .collect();
    v.sort();
    v
}

impl Env {
    fn scan(&self) {
        // Rask skanning: eksakte duplikater via hash, ingen dekoding (testfilene er ikke ekte JPEG).
        self.sh.start_scan(false).unwrap();
        let t = Instant::now();
        while self.sh.state().scan_phase != "done" {
            assert!(t.elapsed() < Duration::from_secs(20), "skanning henger");
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    fn source_of(&self, asset: u64) -> String {
        let core = self.sh.core().unwrap();
        core.fjell.sources_for(&[AssetId(asset)])[0].1 .0.clone()
    }

    /// Duplikatkort som (kort-id, asset-id, kilde-id).
    fn cards(&self) -> Vec<(u64, u64, String)> {
        let page = self.sh.cards("exact_dup", 50, None).unwrap();
        page.cards
            .iter()
            .map(|c| (c.id, c.asset, self.source_of(c.asset)))
            .collect()
    }

    /// Kortet som ligger i rot A (`false`) eller rot B (`true`).
    fn card(&self, in_b: bool) -> (u64, u64, String) {
        let v: Vec<_> = self
            .cards()
            .into_iter()
            .filter(|c| c.2.starts_with("/r") == in_b)
            .collect();
        assert_eq!(v.len(), 1, "oppsett: ett kort per rot: {v:?}");
        v[0].clone()
    }

    fn all_sources(&self) -> Vec<String> {
        let core = self.sh.core().unwrap();
        let mut v: Vec<String> = core
            .fjell
            .with_store(|s| s.active_source_ids())
            .unwrap()
            .into_iter()
            .map(|s| s.0)
            .collect();
        v.sort();
        v
    }

    fn recycled(&self) -> Vec<PathBuf> {
        self.bin.calls.lock().unwrap().clone()
    }
}

#[test]
fn enrots_bibliotek_er_uendret_og_utvides_uten_a_miste_noe() {
    let e = dirs("migrering", "ekstra");
    e.sh.set_library(e.a.to_str().unwrap()).unwrap();
    e.scan();
    let before = e.all_sources();
    assert_eq!(
        before,
        ["a.jpg", "b.jpg", "p.jpg"],
        "ingen prefiks i første rot"
    );
    let ids = e.cards().iter().map(|c| c.0).collect::<Vec<_>>();
    e.sh.decide(&ids, DecisionDto::Trash).unwrap();
    let db = e.sh.core().unwrap().db.clone();
    assert_eq!(
        db,
        config::db_path(&e.data, &e.a),
        "samme indeksfil som før"
    );
    let trashed = e.sh.trashed().unwrap();
    assert_eq!(trashed.len(), 1);
    let st = e.sh.state();
    assert_eq!(st.roots.len(), 1);
    assert!(st.roots[0].primary && st.roots[0].key == 1);
    drop(e.sh.core());

    // En shell.json slik eldre versjoner skrev den (uten `libraries`) leses uendret.
    let old = serde_json::json!({ "library": e.a.to_string_lossy(), "onboarded": true });
    fs::write(e.data.join("shell.json"), old.to_string()).unwrap();
    let sh = shell(&e.data, &e.bin);
    assert_eq!(sh.library().as_deref(), e.a.to_str());
    assert_eq!(sh.core().unwrap().db, db);
    assert_eq!(sh.trashed().unwrap(), trashed, "«Nylig slettet» er som før");
    assert_eq!(sh.list_library_roots().unwrap().len(), 1);

    // Flere røtter utvider samme indeks: samme fil, samme id-er, samme papirkurv.
    let roots = sh.add_library_root(e.b.to_str().unwrap()).unwrap();
    assert_eq!(roots.len(), 2);
    assert_eq!((roots[1].key, roots[1].primary), (2, false));
    assert_eq!(sh.core().unwrap().db, db);
    assert_eq!(sh.trashed().unwrap(), trashed);
    let e2 = Env {
        sh,
        root: e.root.clone(),
        a: e.a.clone(),
        b: e.b.clone(),
        data: e.data.clone(),
        bin: e.bin.clone(),
    };
    e2.scan();
    let after = e2.all_sources();
    for id in before.iter().filter(|s| after.contains(s)) {
        assert!(!id.starts_with('/'));
    }
    assert_eq!(
        after
            .iter()
            .filter(|s| s.starts_with("/r2/"))
            .collect::<Vec<_>>(),
        ["/r2/c.jpg", "/r2/p.jpg", "/r2/q.jpg"]
    );
    assert!(
        after.contains(&"p.jpg".to_string()),
        "A/p.jpg og B/p.jpg er to rader"
    );
    assert_eq!(e2.sh.trashed().unwrap(), trashed);
    // `library` i shell.json er fortsatt første rot, og røttene overlever omstart.
    assert_eq!(
        config::last_library(&e.data).as_deref(),
        Some(e.a.as_path())
    );
    assert_eq!(config::extra_roots(&e.data, &e.a), [(2, e.b.clone())]);
    let again = shell(&e.data, &e.bin);
    assert_eq!(again.state().roots.len(), 2);
    assert_eq!(again.core().unwrap().db, db);
    // Gjenoppretting av det gamle bildet virker som før.
    assert_eq!(again.restore_all().unwrap().restored, 1);
    assert_eq!(files(&e.a).len(), 3);
    std::mem::forget(e2); // `e` rydder mappen
}

#[test]
fn trash_angre_og_gjenoppretting_gar_til_hver_rots_egen_karantene() {
    let e = env("flyt");
    let (before_a, before_b) = (files(&e.a), files(&e.b));
    let ids: Vec<u64> = e.cards().iter().map(|c| c.0).collect();
    assert_eq!(ids.len(), 2);
    e.sh.decide(&ids, DecisionDto::Trash).unwrap();
    assert_eq!(e.sh.state().trash_count, 2);
    assert_eq!((quarantined(&e.a).len(), quarantined(&e.b).len()), (1, 1));
    assert_eq!((files(&e.a).len(), files(&e.b).len()), (2, 2));
    // Angre: begge tilbake på plass, byte for byte.
    e.sh.undo().unwrap();
    assert_eq!(
        (files(&e.a), files(&e.b)),
        (before_a.clone(), before_b.clone())
    );
    assert!(quarantined(&e.a).is_empty() && quarantined(&e.b).is_empty());
    // Trash én og én, gjenopprett bare den i rot B.
    let (card_b, asset_b, src_b) = e.card(true);
    let (card_a, asset_a, _) = e.card(false);
    e.sh.decide(&[card_b], DecisionDto::Trash).unwrap();
    assert_eq!((quarantined(&e.a).len(), quarantined(&e.b).len()), (0, 1));
    assert_eq!(files(&e.a), before_a, "rot A urørt av trash i rot B");
    e.sh.decide(&[card_a], DecisionDto::Trash).unwrap();
    assert_eq!(e.sh.restore(&[asset_b]).unwrap().restored, 1);
    assert_eq!(files(&e.b), before_b);
    assert_eq!(e.source_of(asset_b), src_b);
    assert_eq!(
        quarantined(&e.a).len(),
        1,
        "rot A sin ligger fortsatt i karantene"
    );
    assert_eq!(
        e.sh.trashed()
            .unwrap()
            .iter()
            .map(|t| t.asset)
            .collect::<Vec<_>>(),
        [asset_a]
    );
    // Angre av gjenopprettingen legger filen tilbake i rot B sin karantene (F4), ikke rot A sin.
    e.sh.undo().unwrap();
    assert_eq!((quarantined(&e.a).len(), quarantined(&e.b).len()), (1, 1));
    assert_eq!(e.sh.restore_all().unwrap().restored, 2);
    assert_eq!((files(&e.a), files(&e.b)), (before_a, before_b));
}

#[test]
fn tom_sletter_bare_fra_karantenen_i_bildets_egen_rot() {
    let e = env("tom");
    let (card_b, asset_b, src_b) = e.card(true);
    let rel = src_b.strip_prefix("/r2/").unwrap().to_string();
    e.sh.decide(&[card_b], DecisionDto::Trash).unwrap();
    let staged = quarantined(&e.b);
    assert_eq!(staged.len(), 1);
    // Fremmed manifest i rot A sin karantene som påstår å ha SAMME asset-id og samme relative sti,
    // med en lokkefil. Og en ny fil på den opprinnelige stien i rot B.
    let decoy_dir = e.a.join(QUARANTINE_DIR).join("999");
    let stored = format!("{asset_b}.jpg");
    Manifest {
        batch: 999,
        entries: vec![Entry {
            id: asset_b,
            original: rel.clone(),
            stored: stored.clone(),
            bytes: 5,
        }],
    }
    .save(&decoy_dir)
    .unwrap();
    fs::write(decoy_dir.join(&stored), b"lokke").unwrap();
    fs::write(e.b.join(&rel), b"NY fil paa opprinnelig sti").unwrap();
    let a_before = files(&e.a);

    let r = e.sh.empty(&[], None).unwrap();
    assert_eq!((r.purged, r.kept_trashed), (1, 0));
    assert_eq!(e.recycled(), staged, "bare filen i rot B sin karantene");
    assert!(
        decoy_dir.join(&stored).is_file(),
        "karantene A er aldri brukt for en id fra rot B"
    );
    assert_eq!(
        fs::read(e.b.join(&rel)).unwrap(),
        b"NY fil paa opprinnelig sti"
    );
    assert_eq!(files(&e.a), a_before);
    assert_eq!(e.sh.state().trash_count, 0);
}

#[test]
fn tom_tar_begge_rotter_og_merket_ikke_flyttet_blir_staende() {
    let e = env("tom2");
    let ids: Vec<u64> = e.cards().iter().map(|c| c.0).collect();
    e.sh.decide(&ids, DecisionDto::Trash).unwrap();
    let mut staged = quarantined(&e.a);
    staged.extend(quarantined(&e.b));
    // Filen i rot B forsvinner fra karantenen utenfor appen: den skal ikke erstattes av noe annet.
    let lost = quarantined(&e.b)[0].clone();
    let (a_files, b_files) = (files(&e.a), files(&e.b));
    fs::rename(&lost, e.root.join("flyttet-ut.jpg")).unwrap();
    let r = e.sh.empty(&[], None).unwrap();
    assert_eq!((r.purged, r.kept_trashed), (1, 1));
    assert_eq!(e.recycled(), [staged[0].clone()], "bare rot A sin fil");
    assert_eq!(
        (files(&e.a), files(&e.b)),
        (a_files, b_files),
        "ingen originalsti rørt"
    );
}

#[test]
fn navnekollisjon_ved_gjenoppretting_flytter_indeksen_innenfor_samme_rot() {
    let e = env("kollisjon");
    let (card_b, asset_b, src_b) = e.card(true);
    let rel = src_b.strip_prefix("/r2/").unwrap().to_string();
    let a_before = files(&e.a);
    e.sh.decide(&[card_b], DecisionDto::Trash).unwrap();
    fs::write(e.b.join(&rel), b"fremmed fil").unwrap();
    e.sh.undo().unwrap();
    let stem = rel.strip_suffix(".jpg").unwrap();
    let moved = format!("{stem} (gjenopprettet).jpg");
    assert_eq!(fs::read(e.b.join(&moved)).unwrap(), DUP_B);
    assert_eq!(
        e.source_of(asset_b),
        format!("/r2/{moved}"),
        "indeksen følger filen, med rotprefiks"
    );
    // Nytt trash + «Tøm» tar det foreslåtte innholdet, ikke den fremmede filen (F1).
    let again: Vec<u64> = e
        .cards()
        .iter()
        .filter(|c| c.1 == asset_b)
        .map(|c| c.0)
        .collect();
    assert_eq!(again.len(), 1);
    e.sh.decide(&again, DecisionDto::Trash).unwrap();
    assert!(!e.b.join(&moved).exists());
    let r = e.sh.empty(&[], None).unwrap();
    assert_eq!(r.purged, 1);
    assert_eq!(fs::read(e.b.join(&rel)).unwrap(), b"fremmed fil");
    assert_eq!(
        files(&e.a),
        a_before,
        "samme relative navn i rot A er urørt"
    );
}

#[test]
fn oppstartsavstemming_flytter_tilbake_i_riktig_rot() {
    let e = env("avstem");
    let (card_b, asset_b, src_b) = e.card(true);
    let before_b = files(&e.b);
    e.sh.decide(&[card_b], DecisionDto::Trash).unwrap();
    // Avbrutt angre: indeksen sier aktiv, filen ligger fortsatt i karantenen.
    let core = e.sh.core().unwrap();
    core.fjell.restore_assets(&[AssetId(asset_b)]).unwrap();
    assert_eq!(quarantined(&e.b).len(), 1);
    drop(core);
    let sh = shell(&e.data, &e.bin);
    assert_eq!(files(&e.b), before_b, "avstemt tilbake til rot B");
    assert!(quarantined(&e.b).is_empty());
    let src = sh.core().unwrap().fjell.sources_for(&[AssetId(asset_b)]);
    assert_eq!(src[0].1 .0, src_b);
}

#[test]
fn overlapp_og_ugyldige_mapper_avvises_uten_endring() {
    let e = env("overlapp");
    let sub = e.a.join("album");
    fs::create_dir_all(&sub).unwrap();
    let add = |p: &Path| e.sh.add_library_root(p.to_str().unwrap()).unwrap_err();
    for inside in [
        sub.clone(),
        e.a.clone(),
        e.b.clone(),
        e.root.clone(),
        e.b.join("."),
    ] {
        let err = add(&inside);
        assert_eq!(err.code, "root_overlap", "{inside:?}");
        assert!(err.detail.contains("overlapper"), "{}", err.detail);
    }
    assert_eq!(add(&e.root.join("finnes-ikke")).code, "not_a_folder");
    // Mappe som allerede har en Fjell-karantene med innhold (fra et annet bibliotek).
    let foreign = e.root.join("fremmed");
    let batch = foreign.join(QUARANTINE_DIR).join("1");
    Manifest {
        batch: 1,
        entries: vec![Entry {
            id: 1,
            original: "x.jpg".into(),
            stored: "1.jpg".into(),
            bytes: 1,
        }],
    }
    .save(&batch)
    .unwrap();
    fs::write(batch.join("1.jpg"), b"x").unwrap();
    assert_eq!(add(&foreign).code, "root_has_quarantine");
    assert!(batch.join("1.jpg").is_file(), "fremmed karantene er urørt");
    // Uten manifest, bare løse filer: også avvist.
    fs::remove_file(batch.join("manifest.json")).unwrap();
    assert_eq!(add(&foreign).code, "root_has_quarantine");
    assert_eq!(
        e.sh.remove_library_root(1).unwrap_err().code,
        "root_is_primary"
    );
    assert_eq!(e.sh.remove_library_root(7).unwrap_err().code, "not_found");
    assert_eq!(e.sh.state().roots.len(), 2);
    assert_eq!(config::extra_roots(&e.data, &e.a), [(2, e.b.clone())]);
    // Nøkler brukt av avviste forsøk deles ikke ut på nytt, og en gyldig mappe går fint.
    let ok = e.root.join("tredje");
    fs::create_dir_all(&ok).unwrap();
    let roots = e.sh.add_library_root(ok.to_str().unwrap()).unwrap();
    assert_eq!(roots.len(), 3);
    assert!(roots[2].key >= 3);
}

#[test]
fn rot_med_bilder_i_nylig_slettet_kan_ikke_fjernes() {
    let e = env("fjern");
    let (card_b, asset_b, _) = e.card(true);
    let before_b = files(&e.b);
    e.sh.decide(&[card_b], DecisionDto::Trash).unwrap();
    let err = e.sh.remove_library_root(2).unwrap_err();
    assert_eq!((err.code, err.detail.as_str()), ("root_has_trash", "1"));
    // `set_library` («erstatt med én rot») ville også tatt roten ut: samme avvisning.
    let err = e.sh.set_library(e.a.to_str().unwrap()).unwrap_err();
    assert_eq!(err.code, "root_has_trash");
    assert_eq!(e.sh.state().roots.len(), 2);
    assert_eq!(config::extra_roots(&e.data, &e.a).len(), 2 - 1);
    assert_eq!(
        quarantined(&e.b).len(),
        1,
        "bildet er fortsatt i karantenen og kan gjenopprettes"
    );
    assert_eq!(e.sh.restore(&[asset_b]).unwrap().restored, 1);
    assert_eq!(files(&e.b), before_b);

    // Etter gjenoppretting går fjerningen; ingen filer røres, radene forsvinner ved neste skann.
    let roots = e.sh.remove_library_root(2).unwrap();
    assert_eq!(roots.len(), 1);
    assert_eq!(files(&e.b), before_b);
    assert!(config::extra_roots(&e.data, &e.a).is_empty());
    e.scan();
    assert_eq!(e.all_sources(), ["a.jpg", "b.jpg", "p.jpg"]);
    assert_eq!(e.cards().len(), 1, "bare kortet i rot A er igjen");
    assert_eq!(files(&e.b), before_b);
    // Angre etter fjerning kan ikke flytte noe i den fjernede roten.
    let _ = e.sh.undo();
    assert_eq!(files(&e.b), before_b);
    assert!(quarantined(&e.b).is_empty());
    // Lagt til på nytt får roten NY nøkkel (gamle `/r2/`-rader kan aldri peke inn i en ny mappe).
    let roots = e.sh.add_library_root(e.b.to_str().unwrap()).unwrap();
    assert_eq!(roots[1].key, 3);
    e.scan();
    assert!(e.all_sources().iter().any(|s| s == "/r3/p.jpg"));
    assert!(!e.all_sources().iter().any(|s| s.starts_with("/r2/")));
}

#[test]
fn set_library_erstatter_med_en_rot() {
    let e = env("erstatt");
    let st = e.sh.set_library(e.a.to_str().unwrap()).unwrap();
    assert_eq!(st.roots.len(), 1);
    assert!(config::extra_roots(&e.data, &e.a).is_empty());
    assert_eq!(shell(&e.data, &e.bin).state().roots.len(), 1);
    // Og bytte til en helt annen mappe virker som før.
    let st = e.sh.set_library(e.b.to_str().unwrap()).unwrap();
    assert_eq!(st.roots.len(), 1);
    assert_eq!(st.roots[0].path, e.b.to_string_lossy());
    e.scan();
    assert_eq!(
        e.all_sources(),
        ["c.jpg", "p.jpg", "q.jpg"],
        "som første rot: ingen prefiks"
    );
}

#[test]
fn skymappe_sperrer_bare_bildene_i_sin_egen_rot() {
    let e = dirs("sky", "OneDrive");
    e.sh.set_library(e.a.to_str().unwrap()).unwrap();
    e.sh.add_library_root(e.b.to_str().unwrap()).unwrap();
    e.scan();
    let st = e.sh.state();
    assert_eq!(st.cloud_sync, None, "ikke hele biblioteket");
    assert_eq!(st.roots[0].cloud_sync, None);
    assert_eq!(st.roots[1].cloud_sync, Some("onedrive"));
    let (card_b, _, _) = e.card(true);
    let (card_a, _, _) = e.card(false);
    let (before_a, before_b) = (files(&e.a), files(&e.b));
    let err = e.sh.decide(&[card_b], DecisionDto::Trash).unwrap_err();
    assert_eq!(
        (err.code, err.detail.as_str()),
        ("cloud_synced_readonly", "onedrive")
    );
    // Blandet kall: avvises helt, ingenting er endret.
    let err =
        e.sh.decide(&[card_a, card_b], DecisionDto::Trash)
            .unwrap_err();
    assert_eq!(err.code, "cloud_synced_readonly");
    assert_eq!(e.sh.state().trash_count, 0);
    assert_eq!((files(&e.a), files(&e.b)), (before_a, before_b.clone()));
    assert!(
        !e.b.join(QUARANTINE_DIR).exists(),
        "ingenting skrevet i skymappen"
    );
    // Rot A er ikke sperret, og Behold er lov i skyroten.
    e.sh.decide(&[card_a], DecisionDto::Trash).unwrap();
    assert_eq!(quarantined(&e.a).len(), 1);
    e.sh.decide(&[card_b], DecisionDto::Keep).unwrap();
    assert_eq!(files(&e.b), before_b);
}

#[test]
fn bibliotek_med_bare_skyrotter_er_helt_sperret() {
    let e = dirs("sky2", "Dropbox");
    let cloud_a = e.root.join("OneDrive").join("Bilder");
    fs::create_dir_all(&cloud_a).unwrap();
    fs::write(cloud_a.join("a.jpg"), DUP_A).unwrap();
    fs::write(cloud_a.join("b.jpg"), DUP_A).unwrap();
    e.sh.set_library(cloud_a.to_str().unwrap()).unwrap();
    e.sh.add_library_root(e.b.to_str().unwrap()).unwrap();
    e.scan();
    assert_eq!(e.sh.state().cloud_sync, Some("onedrive"));
    let ids: Vec<u64> = e.cards().iter().map(|c| c.0).collect();
    assert_eq!(ids.len(), 2);
    for id in ids {
        assert_eq!(
            e.sh.decide(&[id], DecisionDto::Trash).unwrap_err().code,
            "cloud_synced_readonly"
        );
    }
}

#[test]
fn frakoblet_rot_mister_ingenting_og_kan_ikke_tommes() {
    let e = env("frakoblet");
    let (card_b, asset_b, _) = e.card(true);
    let before_b = files(&e.b);
    e.sh.decide(&[card_b], DecisionDto::Trash).unwrap();
    let away = e.root.join("ekstra-frakoblet");
    fs::rename(&e.b, &away).unwrap();
    // Omstart med rot B borte: biblioteket åpnes, og bildet er fortsatt i «Nylig slettet».
    let sh = shell(&e.data, &e.bin);
    let roots = sh.list_library_roots().unwrap();
    assert_eq!(
        (roots[0].online, roots[1].online),
        (Some(true), Some(false))
    );
    assert_eq!(roots[1].trashed, Some(1));
    assert_eq!(sh.state().trash_count, 1);
    // Skanning: ingenting i den frakoblede roten regnes som fjernet.
    sh.start_scan(false).unwrap();
    let t = Instant::now();
    while sh.state().scan_phase != "done" {
        assert!(t.elapsed() < Duration::from_secs(20));
        std::thread::sleep(Duration::from_millis(10));
    }
    let roots = sh.list_library_roots().unwrap();
    assert_eq!(roots[1].count, Some(2), "radene i rot B står");
    // «Tøm» sletter ingenting der, og rot kan ikke fjernes mens bildet ligger i papirkurven.
    let r = sh.empty(&[], None).unwrap();
    assert_eq!((r.purged, r.kept_trashed, r.unavailable), (0, 1, 1));
    assert!(e.recycled().is_empty());
    assert_eq!(
        sh.remove_library_root(2).unwrap_err().code,
        "root_has_trash"
    );
    // Koblet til igjen: gjenoppretting virker.
    fs::rename(&away, &e.b).unwrap();
    assert_eq!(sh.restore(&[asset_b]).unwrap().restored, 1);
    assert_eq!(files(&e.b), before_b);
}

#[test]
fn tall_per_rot_og_video_som_eksakt_duplikat() {
    let e = dirs("tall", "ekstra");
    // Video og HEIC i rot B: synlige og telt, video-paret er eksakt duplikat.
    fs::write(e.b.join("film.mp4"), vec![7u8; 5000]).unwrap();
    fs::write(e.b.join("film-kopi.mov"), vec![7u8; 5000]).unwrap();
    fs::write(e.b.join("bilde.heic"), b"ikke ekte heic").unwrap();
    fs::write(e.b.join("raw.cr2"), b"ikke ekte raw!").unwrap();
    fs::write(e.b.join("notat.txt"), b"ikke med").unwrap();
    e.sh.set_library(e.a.to_str().unwrap()).unwrap();
    e.sh.add_library_root(e.b.to_str().unwrap()).unwrap();
    // Full skanning: HEIC/RAW uten dekoder hoppes over uten å stoppe noe.
    e.sh.start_scan(true).unwrap();
    let t = Instant::now();
    while e.sh.state().scan_phase != "done" {
        assert!(t.elapsed() < Duration::from_secs(30), "skanning henger");
        std::thread::sleep(Duration::from_millis(10));
    }
    let roots = e.sh.list_library_roots().unwrap();
    let size = |d: &Path, names: &[&str]| -> u64 {
        names
            .iter()
            .map(|n| fs::metadata(d.join(n)).unwrap().len())
            .sum()
    };
    assert_eq!(roots[0].count, Some(3));
    assert_eq!(
        roots[0].bytes,
        Some(size(&e.a, &["a.jpg", "b.jpg", "p.jpg"]))
    );
    assert_eq!((roots[0].videos, roots[0].video_bytes), (Some(0), Some(0)));
    assert_eq!(roots[1].count, Some(7));
    assert_eq!(
        (roots[1].videos, roots[1].video_bytes),
        (Some(2), Some(10_000))
    );
    assert_eq!(roots[1].trashed, Some(0));
    assert_eq!(
        roots[1].quarantine_dir,
        e.b.join(QUARANTINE_DIR).to_string_lossy()
    );
    // `state()` har røttene uten tall (ingen filsystem- eller indeksarbeid i pollingen).
    let st = e.sh.state();
    assert_eq!(st.roots.len(), 2);
    assert!(st
        .roots
        .iter()
        .all(|r| r.count.is_none() && r.online.is_none()));
    // Video-paret er et eksakt duplikatkort, og kan legges i (og hentes fra) rot B sin karantene.
    let video: Vec<_> = e
        .cards()
        .into_iter()
        .filter(|c| c.2.ends_with(".mp4") || c.2.ends_with(".mov"))
        .collect();
    assert_eq!(video.len(), 1, "{:?}", e.cards());
    let before_b = files(&e.b);
    e.sh.decide(&[video[0].0], DecisionDto::Trash).unwrap();
    assert_eq!(quarantined(&e.b).len(), 1);
    assert_eq!(e.sh.list_library_roots().unwrap()[1].trashed, Some(1));
    e.sh.undo().unwrap();
    assert_eq!(files(&e.b), before_b);
    // HEIC uten dekoder: miniatyr er `unavailable`, ikke feil. (På Windows tar WIC over.)
    #[cfg(not(windows))]
    {
        let core = e.sh.core().unwrap();
        let heic = core.fjell.with_store(|s| {
            let conn = s.reader().unwrap().unwrap();
            let q = "SELECT id FROM asset WHERE source_id = '/r2/bilde.heic'";
            conn.query_row(q, [], |r| r.get::<_, i64>(0)).unwrap()
        });
        assert_eq!(e.sh.thumbnail(heic as u64).unwrap_err().code, "unavailable");
    }
}

#[test]
fn pensjonert_kjerne_avviser_handlinger() {
    let e = env("pensjon");
    let old = e.sh.core().unwrap();
    let (_, asset_b, _) = e.card(true);
    let third = e.root.join("tredje");
    fs::create_dir_all(&third).unwrap();
    e.sh.add_library_root(third.to_str().unwrap()).unwrap();
    // En handling som fikk tak i den gamle kjernen før byttet, avvises og flytter ingenting.
    assert_eq!(old.restore(&[asset_b]).unwrap_err().code, "unavailable");
    assert_eq!(old.undo().unwrap_err().code, "unavailable");
    assert_eq!(
        old.empty(e.bin.clone(), Vec::new(), None).unwrap_err().code,
        "unavailable"
    );
    // Den nye virker.
    let (card_a, _, _) = e.card(false);
    e.sh.decide(&[card_a], DecisionDto::Trash).unwrap();
    assert_eq!(quarantined(&e.a).len(), 1);
}

/// Kjent begrensning (FLERE-MAPPER.md): går `shell.json` tapt, er ekstra røtter ukjente. Da skal
/// biblioteket fortsatt kunne åpnes, og ingenting i den ukjente rotens karantene skal slettes.
#[test]
fn tapt_config_sletter_aldri_noe_i_ukjent_rot() {
    let e = env("taptconfig");
    let (card_b, _, _) = e.card(true);
    e.sh.decide(&[card_b], DecisionDto::Trash).unwrap();
    let staged = quarantined(&e.b);
    assert_eq!(staged.len(), 1);
    let b_files = files(&e.b);
    fs::remove_file(e.data.join("shell.json")).unwrap();
    let sh = shell(&e.data, &e.bin);
    assert_eq!(sh.state().roots.len(), 0, "ingen mappe valgt");
    sh.set_library(e.a.to_str().unwrap()).unwrap();
    assert_eq!(sh.state().roots.len(), 1);
    assert_eq!(
        sh.state().trash_count,
        1,
        "bildet står fortsatt i «Nylig slettet»"
    );
    let r = sh.empty(&[], None).unwrap();
    assert_eq!((r.purged, r.kept_trashed), (0, 1));
    assert!(e.recycled().is_empty());
    let _ = sh.restore_all();
    let _ = sh.undo();
    assert_eq!(
        quarantined(&e.b),
        staged,
        "filen ligger urørt i rot B sin karantene"
    );
    assert_eq!(files(&e.b), b_files);
}
