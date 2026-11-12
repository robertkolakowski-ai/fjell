//! Fullflyt via selve `fjell-cli`-binæren mot filsystemadapteren: index → trash → undo → trash →
//! purge (--fs-temp). Viser at originalmappen er byte-for-byte uendret (BLAKE3) hele veien, at purge
//! uten bekreftelse (og uten --fs-temp) ikke sletter noe, og at bare den midlertidige kopien mister filen.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn tempdir(tag: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!("fjell-cliflow-{tag}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&p);
    fs::create_dir_all(&p).unwrap();
    p
}

fn sums(root: &Path) -> BTreeMap<String, String> {
    fn walk(root: &Path, dir: &Path, out: &mut BTreeMap<String, String>) {
        for e in fs::read_dir(dir).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() {
                walk(root, &p, out);
            } else {
                let rel = p.strip_prefix(root).unwrap().to_string_lossy().into_owned();
                out.insert(
                    rel,
                    blake3::hash(&fs::read(&p).unwrap()).to_hex().to_string(),
                );
            }
        }
    }
    let mut m = BTreeMap::new();
    walk(root, root, &mut m);
    m
}

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_fjell-cli"))
        .args(args)
        .output()
        .unwrap()
}

fn stdout(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).into_owned()
}

fn field(s: &str, key: &str) -> String {
    let i = s.find(key).unwrap_or_else(|| panic!("mangler {key}: {s}"));
    s[i + key.len()..]
        .split(|c: char| c.is_whitespace())
        .next()
        .unwrap()
        .to_string()
}

#[test]
fn cli_scan_trash_undo_purge_fs_temp_leaves_original_unchanged() {
    let root = tempdir("src");
    let work = tempdir("work");
    let db = work.join("index.db");
    let (d, dbs) = (root.to_str().unwrap(), db.to_str().unwrap());
    fs::create_dir_all(root.join("sub")).unwrap();
    fs::write(root.join("a.jpg"), b"same-bytes").unwrap();
    fs::write(root.join("sub/b.jpg"), b"same-bytes").unwrap();
    fs::write(root.join("c.png"), b"other-content!").unwrap();
    let before = sums(&root);

    let o = run(&["index", d, "--db", dbs]);
    assert!(o.status.success(), "{o:?}");
    assert!(stdout(&o).contains("exact_dup: 1"), "{}", stdout(&o));

    let o = run(&["trash", d, "--db", dbs]);
    assert!(o.status.success(), "{o:?}");
    let asset = field(&stdout(&o), "trashed asset=");
    assert_eq!(sums(&root), before, "trash rører bare indeksen");

    let o = run(&["undo", d, "--db", dbs]);
    assert!(o.status.success(), "{o:?}");
    assert!(stdout(&o).contains("restored=1"), "{}", stdout(&o));
    assert_eq!(sums(&root), before, "angre rører bare indeksen");
    // Etter angre er forslaget tilbake, og bildet kan ikke purges (ikke i «Nylig slettet»).
    let o = run(&["purge", d, "--db", dbs, "--fs-temp", "--yes", &asset]);
    assert!(!o.status.success(), "purge av ikke-trashet må feile");

    let o = run(&["trash", d, "--db", dbs]);
    assert!(o.status.success(), "{o:?}");
    let asset = field(&stdout(&o), "trashed asset=");

    // Uten --fs-temp: avvist (exit 2). Uten --yes: krever bekreftelse (exit 3), ingen kopi, ingen sletting.
    assert_eq!(
        run(&["purge", d, "--db", dbs, "--yes", &asset])
            .status
            .code(),
        Some(2)
    );
    let o = run(&["purge", d, "--db", dbs, "--fs-temp", &asset]);
    assert_eq!(o.status.code(), Some(3), "{o:?}");
    assert!(!stdout(&o).contains("temp_copy="), "{}", stdout(&o));
    assert!(!stdout(&o).contains("purged="), "{}", stdout(&o));
    assert_eq!(sums(&root), before);

    // Bekreftet: bare den midlertidige kopien mister filen; originalen er uendret.
    let o = run(&["purge", d, "--db", dbs, "--fs-temp", "--yes", &asset]);
    assert!(o.status.success(), "{o:?}");
    let s = stdout(&o);
    assert!(s.contains("purged=1 kept_trashed=0"), "{s}");
    let copy = PathBuf::from(field(&s, "temp_copy="));
    assert_eq!(sums(&root), before, "original uendret etter purge");
    let after = sums(&copy);
    assert_eq!(after.len(), 2, "kopien mistet nøyaktig én fil");
    assert!(after.iter().all(|(k, v)| before[k] == *v));
    fs::remove_dir_all(&copy).unwrap();
    fs::remove_dir_all(&root).unwrap();
    fs::remove_dir_all(&work).unwrap();
}

#[test]
fn index_file_inside_source_is_rejected() {
    let root = tempdir("inside");
    let db = root.join("index.db");
    let o = run(&[
        "index",
        root.to_str().unwrap(),
        "--db",
        db.to_str().unwrap(),
    ]);
    assert_eq!(o.status.code(), Some(2));
    assert!(!db.exists(), "ingen skriving i kildemappen");
    fs::remove_dir_all(&root).unwrap();
}
