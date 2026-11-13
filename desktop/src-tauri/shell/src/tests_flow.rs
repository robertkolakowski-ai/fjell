//! Regresjonstester gjennom `Shell` for red team-funnene F1, F2 og F4 (docs/redteam/SAMMENLIGNING.md).
//!
//! Invariant: «Tøm» sletter bare filer som på Tøm-tidspunktet er i «Nylig slettet» OG som karantenen
//! faktisk inneholder. Testene går veiene restore, angre, angre av restore, navnekollisjon,
//! avbrutt angre (oppstartsavstemming), «Merket, ikke flyttet» og parallelle skall-kall.

use crate::dto::DecisionDto;
use crate::Shell;
use fjell_fs::{FakeRecycleBin, FsSource, RecycleBin, RecycleError};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

const DUP: &[u8] = b"same-bytes-same-bytes";
const NEW: &[u8] = b"NYTT-bilde-aldri-skannet-eller-foreslatt";

/// Papirkurv som flytter til `dir`, og som kan kjøre en krok midt i hver `recycle` (parallelle kall).
struct HookBin {
    inner: FakeRecycleBin,
    hook: OnceLock<Box<dyn Fn() + Send + Sync>>,
}

impl RecycleBin for HookBin {
    fn recycle(&self, path: &Path) -> Result<(), RecycleError> {
        if let Some(h) = self.hook.get() {
            h();
        }
        self.inner.recycle(path)
    }
}

struct Env {
    root: PathBuf,
    lib: PathBuf,
    data: PathBuf,
    bin: Arc<HookBin>,
    sh: Arc<Shell>,
}

impl Drop for Env {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

/// Bibliotek med ett eksakt duplikatpar (a.jpg, b.jpg) og en unik fil, åpnet og skannet.
fn env(tag: &str) -> Env {
    let root = std::env::temp_dir().join(format!("fjell-flow-{tag}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    let (lib, data) = (root.join("bibliotek"), root.join("appdata"));
    fs::create_dir_all(&lib).unwrap();
    fs::create_dir_all(&data).unwrap();
    fs::write(lib.join("a.jpg"), DUP).unwrap();
    fs::write(lib.join("b.jpg"), DUP).unwrap();
    fs::write(lib.join("c.jpg"), b"different-bytes").unwrap();
    let bin = Arc::new(HookBin {
        inner: FakeRecycleBin {
            bin_dir: Some(root.join("papirkurv")),
            ..Default::default()
        },
        hook: OnceLock::new(),
    });
    let sh = Arc::new(Shell::with_data_dir(
        &data,
        bin.clone() as Arc<dyn RecycleBin>,
    ));
    sh.set_library(lib.to_str().unwrap()).unwrap();
    let src = FsSource::new(&lib).without_decoding();
    sh.open_library(&lib, Arc::new(src)).unwrap();
    let e = Env {
        root,
        lib,
        data,
        bin,
        sh,
    };
    e.scan();
    e
}

impl Env {
    fn scan(&self) {
        self.sh.start_scan(true).unwrap();
        let t = Instant::now();
        while self.sh.state().scan_phase != "done" {
            assert!(t.elapsed() < Duration::from_secs(20), "skanning henger");
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    /// Trash duplikatkortet. Gir (asset-id, stien filen lå på).
    fn trash(&self) -> (u64, PathBuf) {
        let cards = self.sh.cards("exact_dup", 50, None).unwrap().cards;
        let ids: Vec<u64> = cards.iter().map(|c| c.id).collect();
        assert_eq!(ids.len(), 1, "oppsett: ett duplikatkort");
        self.sh.decide(&ids, DecisionDto::Trash).unwrap();
        let t = self.sh.trashed().unwrap();
        assert_eq!(t.len(), 1);
        let gone = self.lib.join(&t[0].name);
        assert!(!gone.exists(), "oppsett: filen ligger i karantene");
        (t[0].asset, gone)
    }

    fn recycled(&self) -> Vec<PathBuf> {
        self.bin.inner.calls.lock().unwrap().clone()
    }

    /// Stier (relativt til biblioteket) for jpg-filer utenfor karantenen, sortert.
    fn library_files(&self) -> Vec<String> {
        let mut v: Vec<String> = fs::read_dir(&self.lib)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().is_file())
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        v.sort();
        v
    }

    fn in_quarantine(&self) -> usize {
        let q = self.lib.join(fjell_fs::QUARANTINE_DIR);
        fs::read_dir(q).map_or(0, |rd| {
            rd.filter_map(|b| b.ok())
                .flat_map(|b| fs::read_dir(b.path()).into_iter().flatten())
                .filter_map(|f| f.ok())
                .filter(|f| f.path().extension().is_some_and(|x| x == "jpg"))
                .count()
        })
    }
}

/// Felles slutt for F1: samme bilde legges i «Nylig slettet» på nytt (`again`) og tømmes. Den nye
/// filen på den opprinnelige stien skal stå urørt; det som slettes er det foreslåtte innholdet
/// (nå under kollisjonsnavnet).
fn retrash_and_empty_spares_the_new_file(e: &Env, orig: &Path, again: impl Fn(&Env)) {
    let renamed = orig.with_file_name(format!(
        "{} (gjenopprettet).jpg",
        orig.file_stem().unwrap().to_string_lossy()
    ));
    assert_eq!(
        fs::read(&renamed).unwrap(),
        DUP,
        "gjenopprettet under nytt navn"
    );
    assert_eq!(fs::read(orig).unwrap(), NEW);
    assert_eq!(e.sh.state().trash_count, 0);

    again(e);
    let t = e.sh.trashed().unwrap();
    assert_eq!(t.len(), 1);
    assert_eq!(
        e.lib.join(&t[0].name),
        renamed,
        "indeksen følger filen til kollisjonsnavnet"
    );
    assert!(
        !renamed.exists(),
        "det foreslåtte innholdet ligger i karantenen"
    );
    assert_eq!(
        fs::read(orig).unwrap(),
        NEW,
        "trash rører ikke den nye filen"
    );
    let r = e.sh.empty(&[], None).unwrap();
    assert_eq!(r.purged, 1);
    assert_eq!(
        fs::read(orig).unwrap(),
        NEW,
        "«Tøm» rører ikke den nye filen"
    );
    let rec = e.recycled();
    assert_eq!(rec.len(), 1);
    assert!(rec[0].starts_with(e.lib.join(fjell_fs::QUARANTINE_DIR)));
    let binned = e.root.join("papirkurv").join(rec[0].file_name().unwrap());
    assert_eq!(
        fs::read(binned).unwrap(),
        DUP,
        "bare foreslått innhold slettes"
    );
}

#[test]
fn f1_undo_with_name_collision_points_index_at_the_restored_file() {
    let e = env("f1-undo");
    let (_, orig) = e.trash();
    fs::write(&orig, NEW).unwrap();
    e.sh.undo().unwrap();
    retrash_and_empty_spares_the_new_file(&e, &orig, |e| {
        e.trash();
    });
}

/// Gjenopprett ved kollisjon, og så angre av gjenopprettingen (F1 + F4): tilbake i karantenen går
/// den gjenopprettede filen, ikke den nye som ligger på den opprinnelige stien.
#[test]
fn f1_restore_with_name_collision_points_index_at_the_restored_file() {
    let e = env("f1-restore");
    let (id, orig) = e.trash();
    fs::write(&orig, NEW).unwrap();
    assert_eq!(e.sh.restore(&[id]).unwrap().restored, 1);
    retrash_and_empty_spares_the_new_file(&e, &orig, |e| {
        e.sh.undo().unwrap();
    });
}

/// Krasj mellom indeks-angre og tilbakeflytting: oppstartsavstemmingen flytter filen tilbake og må
/// også oppdatere indeksen ved kollisjon.
#[test]
fn f1_startup_reconcile_with_name_collision_points_index_at_the_restored_file() {
    let e = env("f1-reconcile");
    let (_, orig) = e.trash();
    fs::write(&orig, NEW).unwrap();
    let (api, _) = e.sh.parts().unwrap();
    api.undo().unwrap(); // bare indeksen; filen ligger igjen i karantenen
    assert_eq!(e.in_quarantine(), 1);
    let again = Shell::with_data_dir(&e.data, e.bin.clone() as Arc<dyn RecycleBin>);
    assert_eq!(e.in_quarantine(), 0, "avstemming flyttet filen tilbake");
    let e2 = Env {
        root: e.root.clone(),
        lib: e.lib.clone(),
        data: e.data.clone(),
        bin: e.bin.clone(),
        sh: Arc::new(again),
    };
    let src = FsSource::new(&e2.lib).without_decoding();
    e2.sh.open_library(&e2.lib, Arc::new(src)).unwrap();
    retrash_and_empty_spares_the_new_file(&e2, &orig, |e| {
        e.trash();
    });
    std::mem::forget(e2); // `e` rydder mappen
}

/// F2: «trashed» i indeksen, men filen ligger i biblioteket og ikke i karantenen («Merket, ikke
/// flyttet», eller avbrudd). «Tøm» skal ikke falle tilbake til opprinnelig sti.
#[test]
fn f2_empty_never_deletes_from_the_original_path() {
    let e = env("f2-fallback");
    let (_, orig) = e.trash();
    // Filen tilbake på plass og karantenen tom, uten at indeksen vet det.
    let q = e.lib.join(fjell_fs::QUARANTINE_DIR);
    let batch = fs::read_dir(&q).unwrap().next().unwrap().unwrap().path();
    let stored = fs::read_dir(&batch)
        .unwrap()
        .filter_map(|f| f.ok())
        .find(|f| f.path().extension().is_some_and(|x| x == "jpg"))
        .unwrap()
        .path();
    fs::rename(stored, &orig).unwrap();
    fs::remove_dir_all(&batch).unwrap();

    let r = e.sh.empty(&[], None).unwrap();
    assert_eq!((r.purged, r.kept_trashed), (0, 1), "{r:?}");
    assert!(e.recycled().is_empty(), "ingenting sendt til Papirkurv");
    assert_eq!(fs::read(&orig).unwrap(), DUP);
    assert_eq!(
        e.sh.trashed().unwrap().len(),
        1,
        "fortsatt i «Nylig slettet»"
    );
    // Brukeren kan fortsatt gjenopprette (ren indeksendring; filen ligger der).
    assert_eq!(e.sh.restore_all().unwrap().restored, 1);
    assert_eq!(fs::read(&orig).unwrap(), DUP);
}

/// F2: «Gjenopprett» fra en annen tråd midt i «Tøm» slipper ikke til før «Tøm» er ferdig. Uten
/// låsen ville indeksen sagt `active` for en fil sletteløkken holder på med.
#[test]
fn f2_restore_during_empty_waits_and_never_leaves_an_active_asset_without_file() {
    let e = env("f2-race");
    let (id, orig) = e.trash();
    let restored: Arc<Mutex<Option<std::thread::JoinHandle<u64>>>> = Arc::default();
    let (sh, slot) = (e.sh.clone(), restored.clone());
    let hook = move || {
        let sh = sh.clone();
        let h = std::thread::spawn(move || sh.restore(&[id]).unwrap().restored);
        std::thread::sleep(Duration::from_millis(150)); // gi gjenopprettingen tid til å slippe til
        *slot.lock().unwrap() = Some(h);
    };
    assert!(e.bin.hook.set(Box::new(hook)).is_ok());

    let r = e.sh.empty(&[], None).unwrap();
    let n = restored.lock().unwrap().take().unwrap().join().unwrap();
    assert_eq!(r.purged, 1, "{r:?}");
    assert_eq!(n, 0, "gjenopprettingen kom etter «Tøm» og fant ingenting");
    assert_eq!(e.sh.state().trash_count, 0);
    assert!(!orig.exists(), "filen gikk til Papirkurv, ikke tilbake");
    assert_eq!(e.recycled().len(), 1);
    assert_eq!(
        e.library_files().len(),
        2,
        "én av duplikatene og c.jpg står"
    );
}

/// F4: angre av «Gjenopprett» legger filen tilbake i karantenen, og «Tøm» sletter derfra.
#[test]
fn f4_undo_of_restore_moves_the_file_back_into_quarantine() {
    let e = env("f4");
    let (id, orig) = e.trash();
    assert_eq!(e.sh.restore(&[id]).unwrap().restored, 1);
    assert!(orig.is_file() && e.in_quarantine() == 0);

    e.sh.undo().unwrap(); // angrer gjenopprettingen
    let t = e.sh.trashed().unwrap();
    assert_eq!(t.len(), 1);
    assert_eq!(t[0].asset, id);
    assert!(!orig.exists(), "filen ligger ikke lenger i biblioteket");
    assert_eq!(e.in_quarantine(), 1, "filen er tilbake i karantenen");

    // Fortsatt angrebar: gjenoppretting gir samme fil med samme innhold.
    assert_eq!(e.sh.restore(&[id]).unwrap().restored, 1);
    assert_eq!(fs::read(&orig).unwrap(), DUP);
    e.sh.undo().unwrap();
    assert_eq!(e.in_quarantine(), 1);

    let r = e.sh.empty(&[], None).unwrap();
    assert_eq!(r.purged, 1);
    let rec = e.recycled();
    assert_eq!(rec.len(), 1);
    assert!(
        rec[0].starts_with(e.lib.join(fjell_fs::QUARANTINE_DIR)),
        "«Tøm» slettet fra karantenen, ikke fra biblioteket: {rec:?}"
    );
    assert_eq!(e.library_files().len(), 2);
}
