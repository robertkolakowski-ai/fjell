//! Originalsjekksum-test: skann og liste (pluss full lesing via adapteren) endrer aldri
//! originalfilene. Sammenligner innhold (BLAKE3), størrelse, mtime og filliste før og etter.

use fjell_cli::{cmd, fsource::FsSource};
use fjell_core::model::AssetSource;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

fn tempdir(tag: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!("fjell-{tag}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&p);
    fs::create_dir_all(&p).unwrap();
    p
}

type Snap = BTreeMap<String, (String, u64, std::time::SystemTime)>;

fn snapshot(root: &Path, dir: &Path, out: &mut Snap) {
    for e in fs::read_dir(dir).unwrap() {
        let p = e.unwrap().path();
        if p.is_dir() {
            snapshot(root, &p, out);
        } else {
            let md = fs::metadata(&p).unwrap();
            let h = blake3::hash(&fs::read(&p).unwrap()).to_hex().to_string();
            let rel = p.strip_prefix(root).unwrap().to_string_lossy().into_owned();
            out.insert(rel, (h, md.len(), md.modified().unwrap()));
        }
    }
}

#[test]
fn scan_list_and_reads_never_modify_originals() {
    let root = tempdir("orig");
    fs::create_dir_all(root.join("sub")).unwrap();
    fs::write(root.join("a.jpg"), b"same-bytes").unwrap();
    fs::write(root.join("sub/b.jpg"), b"same-bytes").unwrap();
    fs::write(root.join("c.png"), b"other").unwrap();
    fs::write(root.join("notes.txt"), b"ignored").unwrap();

    let mut before = Snap::new();
    snapshot(&root, &root, &mut before);

    let src = FsSource::new(&root);
    let mut out = Vec::new();
    assert_eq!(cmd::list(&src, &mut out).unwrap(), 3);
    let mut out2 = Vec::new();
    cmd::scan(&src, &mut out2).unwrap();
    let s = String::from_utf8(out2).unwrap();
    assert!(s.contains("exact_dup: 1"), "{s}");
    for m in src.list_changes(None).unwrap().changed {
        let mut r = src.read_bytes(&m.source_id).unwrap();
        std::io::copy(&mut r, &mut std::io::sink()).unwrap();
        let _ = src.thumbnail(&m.source_id);
    }

    let mut after = Snap::new();
    snapshot(&root, &root, &mut after);
    assert_eq!(before, after, "originaler ble endret");
    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn path_escape_is_rejected() {
    let root = tempdir("esc");
    let src = FsSource::new(&root);
    let id = fjell_core::model::SourceId("../x.jpg".into());
    assert!(src.read_bytes(&id).is_err());
    fs::remove_dir_all(&root).unwrap();
}
