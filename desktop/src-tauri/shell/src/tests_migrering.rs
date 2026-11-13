//! KR-3-migrering: indeks navngitt etter gammel hashing (rå sti-streng) tas i bruk under kanonisk navn.
//! Kun kjørt på Linux; Windows-formene (`\\?\`, `/`) er testet som strengkandidater, ikke på Windows.

use crate::config::{adopt_legacy_index, db_path, legacy_index_candidates, locate_index};
use crate::Shell;
use fjell_core::store::Store;
use fjell_fs::{FakeRecycleBin, RecycleBin};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Den gamle hashingen, kopiert ordrett fra før KR-3: FNV-1a av stien som rå tekst.
fn old_index(data: &Path, raw: &str) -> PathBuf {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in raw.bytes() {
        h = (h ^ u64::from(b)).wrapping_mul(0x0000_0100_0000_01b3);
    }
    data.join(format!("index-{h:016x}.sqlite"))
}

struct T {
    root: PathBuf,
    lib: PathBuf,
    data: PathBuf,
}

fn setup(tag: &str) -> T {
    let root = std::env::temp_dir().join(format!("fjell-migr-{tag}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    let (lib, data) = (root.join("bibliotek"), root.join("appdata"));
    fs::create_dir_all(&lib).unwrap();
    fs::create_dir_all(&data).unwrap();
    T { root, lib, data }
}

fn legacy_with_days(path: &Path, days: &str) {
    let mut s = Store::open(path).unwrap();
    s.set_setting("trash_days", days).unwrap();
}

fn shell(t: &T) -> Shell {
    let bin = Arc::new(FakeRecycleBin::default()) as Arc<dyn RecycleBin>;
    Shell::with_data_dir(&t.data, bin)
}

fn raw(p: &Path) -> String {
    p.to_string_lossy().into_owned()
}

fn real_rename(a: &Path, b: &Path) -> io::Result<()> {
    fs::rename(a, b)
}

#[test]
fn gammel_indeks_flyttes_og_tilstand_bevares() {
    let t = setup("flytt");
    // En symlenke gjør rå og kanonisk sti ulike også på Linux.
    let link = t.root.join("lenke");
    std::os::unix::fs::symlink(&t.lib, &link).unwrap();
    let old = old_index(&t.data, &raw(&link));
    legacy_with_days(&old, "7");
    let new = db_path(&t.data, &link);
    assert_ne!(old, new);
    let sh = shell(&t);
    sh.set_library(&raw(&link)).unwrap();
    assert_eq!(sh.state().trash_days, 7, "tilstanden er bevart");
    assert!(new.exists() && !old.exists(), "gammel fil er flyttet");
    let _ = fs::remove_dir_all(&t.root);
}

#[test]
fn wal_shm_og_hovedfil_flyttes_i_rekkefolge_wal_shm_hoved() {
    let t = setup("rekkefolge");
    let raw_lib = format!("{}/./", raw(&t.lib));
    let old = old_index(&t.data, &raw_lib);
    for (suffix, body) in [("", "main"), ("-wal", "wal"), ("-shm", "shm")] {
        fs::write(format!("{}{suffix}", old.display()), body).unwrap();
    }
    let order = std::cell::RefCell::new(Vec::new());
    let rec = |a: &Path, b: &Path| {
        order
            .borrow_mut()
            .push(raw(a).rsplit('.').next().unwrap().to_string());
        fs::rename(a, b)
    };
    let used = adopt_legacy_index(
        &t.data,
        &t.lib.join("."),
        Some(Path::new(&raw_lib)),
        None,
        &rec,
    );
    let new = db_path(&t.data, &t.lib);
    assert_eq!(used, new);
    assert_eq!(*order.borrow(), ["sqlite-wal", "sqlite-shm", "sqlite"]);
    for (suffix, body) in [("", "main"), ("-wal", "wal"), ("-shm", "shm")] {
        assert_eq!(
            fs::read_to_string(format!("{}{suffix}", new.display())).unwrap(),
            body
        );
        assert!(!Path::new(&format!("{}{suffix}", old.display())).exists());
    }
    let _ = fs::remove_dir_all(&t.root);
}

#[test]
fn begge_finnes_ny_brukes_gammel_urort() {
    let t = setup("begge");
    let link = t.root.join("lenke");
    std::os::unix::fs::symlink(&t.lib, &link).unwrap();
    let old = old_index(&t.data, &raw(&link));
    let new = db_path(&t.data, &link);
    legacy_with_days(&old, "7");
    legacy_with_days(&new, "11");
    let old_bytes = fs::read(&old).unwrap();
    let sh = shell(&t);
    sh.set_library(&raw(&link)).unwrap();
    assert_eq!(sh.state().trash_days, 11);
    assert_eq!(fs::read(&old).unwrap(), old_bytes, "gammel indeks er urørt");
    let _ = fs::remove_dir_all(&t.root);
}

#[test]
fn ingen_finnes_ny_opprettes_og_ingenting_annet() {
    let t = setup("ingen");
    let sh = shell(&t);
    sh.set_library(&raw(&t.lib)).unwrap();
    let new = db_path(&t.data, &t.lib);
    assert!(new.exists());
    let mut names: Vec<String> = fs::read_dir(&t.data)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|n| n.starts_with("index-"))
        .collect();
    names.retain(|n| !n.ends_with("-wal") && !n.ends_with("-shm"));
    assert_eq!(
        names,
        [new.file_name().unwrap().to_string_lossy().into_owned()]
    );
    let _ = fs::remove_dir_all(&t.root);
}

#[test]
fn flytting_umulig_faller_tilbake_til_gammel_fil_uten_tap() {
    let t = setup("umulig");
    let old = old_index(&t.data, &raw(&t.lib.join("."))); // rå sti med `.` ≠ kanonisk
    fs::write(&old, "hoved").unwrap();
    fs::write(format!("{}-wal", old.display()), "wal").unwrap();
    fs::write(format!("{}-shm", old.display()), "shm").unwrap();
    // wal flyttes, så feiler hovedfilen: wal må rulles tilbake.
    let fail_main = |a: &Path, b: &Path| {
        if raw(a).ends_with(".sqlite") {
            Err(io::Error::new(io::ErrorKind::PermissionDenied, "låst"))
        } else {
            fs::rename(a, b)
        }
    };
    let lib = t.lib.join(".");
    let used = adopt_legacy_index(&t.data, &lib, None, None, &fail_main);
    assert_eq!(used, old, "gammel fil brukes direkte");
    assert!(!db_path(&t.data, &lib).exists());
    for (s, body) in [("", "hoved"), ("-wal", "wal"), ("-shm", "shm")] {
        assert_eq!(
            fs::read_to_string(format!("{}{s}", old.display())).unwrap(),
            body
        );
        assert!(!Path::new(&format!("{}{s}", db_path(&t.data, &lib).display())).exists());
    }
    // locate_index (kilden for fjernings-sjekken) følger den gamle filen.
    assert_eq!(locate_index(&t.data, &lib, None), old);
    let _ = fs::remove_dir_all(&t.root);
}

#[test]
fn ikke_overskriv_eksisterende_wal_under_nytt_navn() {
    let t = setup("wal-finnes");
    let lib = t.lib.join(".");
    let old = old_index(&t.data, &raw(&lib));
    fs::write(&old, "gammel").unwrap();
    let new_wal = format!("{}-wal", db_path(&t.data, &lib).display());
    fs::write(&new_wal, "uvedkommende").unwrap();
    let used = adopt_legacy_index(&t.data, &lib, None, None, &real_rename);
    assert_eq!(used, old);
    assert_eq!(fs::read_to_string(&new_wal).unwrap(), "uvedkommende");
    assert!(old.exists());
    let _ = fs::remove_dir_all(&t.root);
}

#[test]
fn trailing_separator_variant_finnes() {
    let t = setup("trailing");
    let _ = &t;
    // Ekte eksempel: gammel hash av `lib/` (med skråstrek) fra en symlenke-sti.
    let link = t.root.join("lenke");
    std::os::unix::fs::symlink(&t.lib, &link).unwrap();
    let given = format!("{}/", raw(&link));
    let old = old_index(&t.data, &raw(&link)); // hash av trimmet form
    fs::write(&old, "y").unwrap();
    let used = adopt_legacy_index(&t.data, Path::new(&given), None, None, &real_rename);
    assert_eq!(used, db_path(&t.data, Path::new(&given)));
    assert!(!old.exists() && used.exists());
    let _ = fs::remove_dir_all(&t.root);
}

#[test]
fn windows_former_er_kandidater_men_lagret_sti_til_annen_mappe_ignoreres() {
    let t = setup("win");
    let verb = r"\\?\C:\Bilder\";
    let c = legacy_index_candidates(&t.data, Path::new(verb), None);
    for s in [r"\\?\C:\Bilder", r"C:\Bilder\", r"C:\Bilder"] {
        assert!(c.contains(&old_index(&t.data, s)), "{s}");
    }
    let other = t.root.join("annen");
    fs::create_dir_all(&other).unwrap();
    let c = legacy_index_candidates(&t.data, &t.lib.join("."), Some(&other));
    assert!(!c.contains(&old_index(&t.data, &raw(&other))));
    let _ = fs::remove_dir_all(&t.root);
}
