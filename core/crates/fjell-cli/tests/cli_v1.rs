//! V1-kommandoene via `fjell-cli`-binæren: protect, series, dashboard, people, rules. Viser at ingen av dem
//! sletter eller endrer originalfiler (BLAKE3 før og etter), at ansiktsindeksen er av som standard og at
//! wipe krever `--yes`.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn tempdir(tag: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!("fjell-cliv1-{tag}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&p);
    fs::create_dir_all(&p).unwrap();
    p
}

fn sums(root: &Path) -> BTreeMap<String, String> {
    let mut m = BTreeMap::new();
    for e in fs::read_dir(root).unwrap() {
        let p = e.unwrap().path();
        if p.is_file() {
            m.insert(
                p.file_name().unwrap().to_string_lossy().into_owned(),
                blake3::hash(&fs::read(&p).unwrap()).to_hex().to_string(),
            );
        }
    }
    m
}

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_fjell-cli"))
        .args(args)
        .output()
        .unwrap()
}

fn out(o: &Output) -> String {
    assert!(o.status.success(), "{o:?}");
    String::from_utf8_lossy(&o.stdout).into_owned()
}

#[test]
fn v1_commands_never_touch_originals() {
    let root = tempdir("src");
    let work = tempdir("work");
    let db = work.join("index.db");
    let (d, dbs) = (root.to_str().unwrap(), db.to_str().unwrap());
    fs::write(root.join("a.jpg"), b"same-bytes").unwrap();
    fs::write(root.join("b.jpg"), b"same-bytes").unwrap();
    fs::write(root.join("c.png"), b"other-content!").unwrap();
    let before = sums(&root);
    out(&run(&["index", d, "--db", dbs]));

    // Dashboard fra indeksen.
    let s = out(&run(&["dashboard", d, "--db", dbs]));
    assert!(s.contains("active: 3"), "{s}");
    assert!(s.contains("freed_bytes: 0"), "{s}");
    assert!(
        s.contains("protected: 0 series: 0 people: 0 rules_active: 0"),
        "{s}"
    );

    // Beskyttelse: legg til, list, fjern (bildene er urørt), ugyldige mål avvises.
    let s = out(&run(&["protect", d, "--db", dbs, "add", "asset", "1"]));
    assert!(s.contains("protected asset=1"), "{s}");
    let s = out(&run(&["protect", d, "--db", dbs, "list"]));
    assert!(s.contains("protected asset=1 created_at="), "{s}");
    let s = out(&run(&["dashboard", d, "--db", dbs]));
    assert!(s.contains("protected: 1 "), "{s}");
    let o = run(&["protect", d, "--db", dbs, "add", "folder", ""]);
    assert_eq!(o.status.code(), Some(1), "tom mappe avvises: {o:?}");
    let s = out(&run(&["protect", d, "--db", dbs, "remove", "asset", "1"]));
    assert!(s.contains("unprotected asset=1"), "{s}");
    assert!(!out(&run(&["protect", d, "--db", dbs, "list"])).contains("asset=1"));

    // Regler: forhåndsvis lagrer ikke; add, toggle, delete og restore (angre).
    let s = out(&run(&[
        "rules",
        d,
        "--db",
        dbs,
        "preview",
        "mime=image/png",
    ]));
    assert!(s.contains("hits=1 "), "{s}");
    assert_eq!(out(&run(&["rules", d, "--db", dbs, "list"])).trim(), "");
    let s = out(&run(&[
        "rules",
        d,
        "--db",
        dbs,
        "add",
        "PNG",
        "mime=image/png",
    ]));
    let id = s.trim().strip_prefix("rule=").unwrap().to_string();
    let s = out(&run(&["rules", d, "--db", dbs, "list"]));
    assert!(
        s.contains("name=PNG enabled=true hits=1 conditions=mime=image/png"),
        "{s}"
    );
    out(&run(&["rules", d, "--db", dbs, "toggle", &id, "off"]));
    assert!(out(&run(&["rules", d, "--db", dbs, "list"])).contains("enabled=false hits=0"));
    out(&run(&["rules", d, "--db", dbs, "delete", &id]));
    assert_eq!(out(&run(&["rules", d, "--db", dbs, "list"])).trim(), "");
    out(&run(&["rules", d, "--db", dbs, "restore", &id]));
    assert!(out(&run(&["rules", d, "--db", dbs, "list"])).contains("name=PNG"));
    let o = run(&["rules", d, "--db", dbs, "add", "tom"]);
    assert_eq!(o.status.code(), Some(1), "tom regel avvises: {o:?}");
    let o = run(&["rules", d, "--db", dbs, "preview", "bogus"]);
    assert_eq!(o.status.code(), Some(2), "{o:?}");

    // Serier uten miniatyrer: ingen forslag, men kommandoen virker.
    assert!(out(&run(&["series", d, "--db", dbs, "list"])).contains("series_cards: 0"));
    assert_eq!(
        run(&["series", d, "--db", dbs, "compare", "99"])
            .status
            .code(),
        Some(1)
    );

    // Personer: ansiktsindeks er av som standard; wipe krever --yes og fjerner aldri bilder.
    assert!(out(&run(&["people", d, "--db", dbs, "face-index"])).contains("face_index: off"));
    assert!(out(&run(&["people", d, "--db", dbs, "face-index", "on"])).contains("face_index: on"));
    assert!(out(&run(&["people", d, "--db", dbs, "face-index"])).contains("face_index: on"));
    let o = run(&["people", d, "--db", dbs, "face-index", "wipe"]);
    assert_eq!(o.status.code(), Some(3), "{o:?}");
    assert!(out(&run(&[
        "people",
        d,
        "--db",
        dbs,
        "face-index",
        "wipe",
        "--yes"
    ]))
    .contains("wiped faces=0"));
    assert!(out(&run(&["people", d, "--db", dbs, "list"])).contains("people: 0"));
    out(&run(&["people", d, "--db", dbs, "face-index", "off"]));

    // Ukjent underkommando er bruksfeil. Originalene er uendret hele veien.
    assert_eq!(
        run(&["rules", d, "--db", dbs, "nonsense"]).status.code(),
        Some(2)
    );
    assert_eq!(sums(&root), before);
    fs::remove_dir_all(&root).unwrap();
    fs::remove_dir_all(&work).unwrap();
}
