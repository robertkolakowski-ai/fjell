//! Red team (Fable) for FLERE MAPPER (docs/mvp/FLERE-MAPPER.md). Reprodusenter gjennom `Shell`.
//! Funnene RT-FM-1..3 er rettet; testene kjører som vanlige regresjonstester
//! (`cargo test -p fjell-shell redteam_flere_mapper`).

use crate::dto::DecisionDto;
use crate::Shell;
use fjell_core::model::AssetId;
use fjell_fs::{FakeRecycleBin, RecycleBin, QUARANTINE_DIR};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

struct Env {
    root: PathBuf,
    a: PathBuf,
    b: PathBuf,
    data: PathBuf,
    bin: Arc<FakeRecycleBin>,
}

impl Drop for Env {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn shell(e: &Env) -> Shell {
    Shell::with_data_dir(&e.data, e.bin.clone() as Arc<dyn RecycleBin>)
}

/// Rot A (a.jpg = b.jpg) og mappen `b_name` (p.jpg = q.jpg). Ingenting er åpnet.
fn dirs(tag: &str, b_name: &str) -> Env {
    let root = std::env::temp_dir().join(format!("fjell-rtfm-{tag}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    let (a, b, data) = (
        root.join("bibliotek"),
        root.join(b_name),
        root.join("appdata"),
    );
    for d in [&a, &b, &data] {
        fs::create_dir_all(d).unwrap();
    }
    fs::write(a.join("a.jpg"), b"duplikat-i-forste-rot").unwrap();
    fs::write(a.join("b.jpg"), b"duplikat-i-forste-rot").unwrap();
    fs::write(b.join("p.jpg"), b"duplikat-i-andre-rot!").unwrap();
    fs::write(b.join("q.jpg"), b"duplikat-i-andre-rot!").unwrap();
    let bin = Arc::new(FakeRecycleBin {
        bin_dir: Some(root.join("papirkurv")),
        ..Default::default()
    });
    Env {
        root,
        a,
        b,
        data,
        bin,
    }
}

fn scan(sh: &Shell) {
    sh.start_scan(false).unwrap();
    let t = Instant::now();
    while sh.state().scan_phase != "done" {
        assert!(t.elapsed() < Duration::from_secs(20), "skanning henger");
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// Duplikatkort som (kort-id, asset-id, kilde-id).
fn cards(sh: &Shell) -> Vec<(u64, u64, String)> {
    let core = sh.core().unwrap();
    sh.cards("exact_dup", 50, None)
        .unwrap()
        .cards
        .iter()
        .map(|c| {
            let sid = core.fjell.sources_for(&[AssetId(c.asset)])[0].1 .0.clone();
            (c.id, c.asset, sid)
        })
        .collect()
}

fn card(sh: &Shell, extra: bool) -> (u64, u64, String) {
    let v: Vec<_> = cards(sh)
        .into_iter()
        .filter(|c| c.2.starts_with("/r") == extra)
        .collect();
    assert_eq!(v.len(), 1, "oppsett: ett kort per rot: {v:?}");
    v[0].clone()
}

/// Filene i mappen utenom karantenen (relativ sti -> innhold).
fn files(dir: &Path) -> BTreeMap<String, Vec<u8>> {
    fs::read_dir(dir)
        .into_iter()
        .flatten()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_file())
        .map(|e| {
            (
                e.file_name().to_string_lossy().into_owned(),
                fs::read(e.path()).unwrap(),
            )
        })
        .collect()
}

/// RT-FM-1 (høy): bytte bort fra et bibliotek med bilder i «Nylig slettet» i en ekstra mappe er
/// lov uten varsel, men veien tilbake er stengt: `set_library(A)` regner A sine lagrede ekstra
/// mapper som «tas ut» og avviser med `root_has_trash`, og intet annet kall åpner A med røttene
/// sine (omstart åpner sist valgte mappe). Bildet blir liggende i rot B sin karantene uten at appen
/// kan gjenopprette det. Fil: `shell/src/lib.rs`, `Shell::set_library` (`dropped` hentes fra
/// config for MÅL-biblioteket) og `open_roots_probe` (`trashed_in(dropped)`).
#[test]
fn rtfm1_bytte_bort_og_tilbake_stenger_ute_nylig_slettet_i_ekstra_mappe() {
    let e = dirs("utestengt", "ekstra");
    let annet = e.root.join("annet-bibliotek");
    fs::create_dir_all(&annet).unwrap();
    let sh = shell(&e);
    sh.set_library(e.a.to_str().unwrap()).unwrap();
    sh.add_library_root(e.b.to_str().unwrap()).unwrap();
    scan(&sh);
    let before_b = files(&e.b);
    let (card_b, _, _) = card(&sh, true);
    sh.decide(&[card_b], DecisionDto::Trash).unwrap();
    assert_eq!(
        files(&e.b).len(),
        1,
        "oppsett: flyttet til rot B sin karantene"
    );

    // Enten må byttet bort nektes (som fjerning av roten), eller så må veien tilbake virke.
    let bort = sh.set_library(annet.to_str().unwrap());
    if bort.is_ok() {
        let tilbake = sh.set_library(e.a.to_str().unwrap());
        assert!(
            tilbake.is_ok(),
            "biblioteket kan ikke åpnes igjen: {:?}",
            tilbake.err().map(|x| (x.code, x.detail))
        );
        // Også etter omstart.
        let sh2 = shell(&e);
        assert_eq!(sh2.state().trash_count, 1, "bildet står i «Nylig slettet»");
        assert_eq!(sh2.restore_all().unwrap().restored, 1);
        assert_eq!(files(&e.b), before_b, "gjenopprettet byte for byte");
    }
}

/// RT-FM-2 (høy, krever tapt/ødelagt `shell.json`): nøkkelen 2 deles ut på nytt når `shell.json`
/// er borte, mens indeksen fortsatt har `/r2/<navn>` i «Nylig slettet». En NY mappe som legges til
/// arver da raden: første angre (av hva som helst) kjører `restage_trashed`, som flytter den nye
/// mappens fil med samme relative navn inn i karantenen, og «Tøm» sletter den. Brukeren har aldri
/// lagt den filen i papirkurven. Fil: `shell/src/config.rs` `add_extra_root` (`next_key` ligger
/// bare i `shell.json`), `shell/src/core.rs` `restage_trashed` -> `RootSet::stage` (kun sti).
#[test]
fn rtfm2_gjenbrukt_nokkel_etter_tapt_config_tommer_fremmed_fil() {
    let e = dirs("gjenbruk", "ekstra");
    let sh = shell(&e);
    sh.set_library(e.a.to_str().unwrap()).unwrap();
    sh.add_library_root(e.b.to_str().unwrap()).unwrap();
    scan(&sh);
    let (card_b, _, src_b) = card(&sh, true);
    sh.decide(&[card_b], DecisionDto::Trash).unwrap();
    let rel = src_b
        .strip_prefix("/r2/")
        .expect("oppsett: rot B har nøkkel 2");
    drop(sh);

    // shell.json går tapt. En helt annen mappe med samme filnavn (typisk kameranavn) legges til.
    fs::remove_file(e.data.join("shell.json")).unwrap();
    let ny = e.root.join("ny-mappe");
    fs::create_dir_all(&ny).unwrap();
    fs::write(ny.join(rel), b"et helt annet bilde som aldri er foreslatt").unwrap();
    fs::write(ny.join("annet.jpg"), b"enda et bilde").unwrap();
    let before_ny = files(&ny);
    let sh = shell(&e);
    sh.set_library(e.a.to_str().unwrap()).unwrap();
    sh.add_library_root(ny.to_str().unwrap()).unwrap();
    scan(&sh);

    // En vanlig trash + angre i rot A.
    let (card_a, _, _) = card(&sh, false);
    sh.decide(&[card_a], DecisionDto::Trash).unwrap();
    sh.undo().unwrap();
    assert_eq!(
        files(&ny),
        before_ny,
        "angre i rot A flyttet en fil ut av den nye mappen"
    );
    let _ = sh.empty(&[], None);
    assert!(
        e.bin.calls.lock().unwrap().is_empty(),
        "«Tøm» slettet en fil brukeren aldri la i papirkurven: {:?}",
        e.bin.calls.lock().unwrap()
    );
    assert_eq!(files(&ny), before_ny);
}

/// RT-FM-3 (middels; KR-5 gjelder nå også hver ekstra mappe): skysperren per rot ser bare på
/// teksten i stien brukeren valgte. En ekstra mappe som er en lenke (symlenke her, junction/subst
/// på Windows) inn i en OneDrive-mappe får `cloud_sync = None`, og trash flytter filen til en
/// karantene som ligger i den skysynkede mappen. Fil: `shell/src/core.rs` `Core::root_cloud`.
#[cfg(unix)]
#[test]
fn rtfm3_lenket_ekstra_mappe_inn_i_skymappe_er_ikke_sperret() {
    let e = dirs("skylenke", "OneDrive");
    let lenke = e.root.join("bilder-snarvei");
    std::os::unix::fs::symlink(&e.b, &lenke).unwrap();
    let sh = shell(&e);
    sh.set_library(e.a.to_str().unwrap()).unwrap();
    sh.add_library_root(lenke.to_str().unwrap()).unwrap();
    scan(&sh);
    let before_b = files(&e.b);
    let (card_b, _, _) = card(&sh, true);
    let r = sh.decide(&[card_b], DecisionDto::Trash);
    assert_eq!(
        r.err().map(|x| x.code),
        Some("cloud_synced_readonly"),
        "trash i skysynket mappe ble ikke nektet"
    );
    assert_eq!(files(&e.b), before_b);
    assert!(
        !e.b.join(QUARANTINE_DIR).exists(),
        "ingenting skal skrives i skymappen"
    );
}
