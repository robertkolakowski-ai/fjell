//! Massesletting i CLI: over terskelen (50 bilder) krever både `trash` og `purge` eksplisitt bekreftelse
//! (`--yes`). Uten den skjer ingenting (exit 3, ingen sletting, ingen kopi). Originalene er uendret hele veien.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const PAIRS: usize = 60;

fn tempdir(tag: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!("fjell-climass-{tag}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&p);
    fs::create_dir_all(&p).unwrap();
    p
}

fn sums(root: &Path) -> BTreeMap<String, String> {
    let mut m = BTreeMap::new();
    for e in fs::read_dir(root).unwrap() {
        let p = e.unwrap().path();
        m.insert(
            p.file_name().unwrap().to_string_lossy().into_owned(),
            blake3::hash(&fs::read(&p).unwrap()).to_hex().to_string(),
        );
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
    String::from_utf8_lossy(&o.stdout).into_owned()
}

#[test]
fn cli_mass_trash_and_purge_need_yes() {
    let root = tempdir("src");
    let work = tempdir("work");
    let db = work.join("index.db");
    let (d, dbs) = (root.to_str().unwrap(), db.to_str().unwrap());
    for p in 0..PAIRS {
        let bytes = format!("pair-{p}-{}", "x".repeat(p % 37));
        fs::write(root.join(format!("p{p:03}a.jpg")), &bytes).unwrap();
        fs::write(root.join(format!("p{p:03}b.jpg")), &bytes).unwrap();
    }
    let before = sums(&root);
    let o = run(&["index", d, "--db", dbs]);
    assert!(o.status.success(), "{o:?}");
    assert!(
        out(&o).contains(&format!("exact_dup: {PAIRS}")),
        "{}",
        out(&o)
    );

    // trash uten --yes: krever bekreftelse, ingenting i «Nylig slettet».
    let o = run(&["trash", d, "--db", dbs]);
    assert_eq!(o.status.code(), Some(3), "{o:?}");
    assert!(!out(&o).contains("trashed asset="), "{}", out(&o));
    let o = run(&["undo", d, "--db", dbs]);
    assert!(!o.status.success(), "ingenting å angre etter avvist trash");

    // Med --yes: alle {PAIRS} i én batch; ett angre tar alle tilbake.
    let o = run(&["trash", d, "--db", dbs, "--yes"]);
    assert!(o.status.success(), "{o:?}");
    let ids: Vec<String> = out(&o)
        .lines()
        .filter_map(|l| l.strip_prefix("trashed asset="))
        .map(|l| l.split_whitespace().next().unwrap().to_string())
        .collect();
    assert_eq!(ids.len(), PAIRS);
    let o = run(&["undo", d, "--db", dbs]);
    assert!(
        out(&o).contains(&format!("restored={PAIRS}")),
        "{}",
        out(&o)
    );
    let o = run(&["trash", d, "--db", dbs, "--yes"]);
    assert!(o.status.success(), "{o:?}");
    assert_eq!(sums(&root), before, "trash rører bare indeksen");

    // purge av alle: uten --yes exit 3 og ingen kopi/sletting.
    let mut args = vec!["purge", d, "--db", dbs, "--fs-temp"];
    args.extend(ids.iter().map(String::as_str));
    let o = run(&args);
    assert_eq!(o.status.code(), Some(3), "{o:?}");
    assert!(!out(&o).contains("temp_copy=") && !out(&o).contains("purged="));
    // Med --yes: mass-bekreftelsen gis, og bare den midlertidige kopien mister filene.
    args.push("--yes");
    let o = run(&args);
    assert!(o.status.success(), "{o:?}");
    let s = out(&o);
    assert!(s.contains(&format!("purged={PAIRS} kept_trashed=0")), "{s}");
    let copy = PathBuf::from(
        s.lines()
            .find_map(|l| l.strip_prefix("temp_copy="))
            .unwrap(),
    );
    assert_eq!(sums(&root), before, "original uendret etter purge");
    assert_eq!(sums(&copy).len(), PAIRS, "kopien mistet {PAIRS} filer");
    let _ = fs::remove_dir_all(&copy);
    let _ = fs::remove_dir_all(&root);
    let _ = fs::remove_dir_all(&work);
}
