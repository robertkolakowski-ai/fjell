//! Red team (Sonnet): angripende tester mot sikkerhetsreglene i CLAUDE.md, via offentlig API i
//! `fjell-fs` og `fjell-cli`/`fjell-core`. Se `docs/mvp/REDTEAM-SONNET.md` for funnliste.
//!
//! - Tester UTEN `#[ignore]` er angrep som holdt (regresjonsvern).
//! - Tester med `#[ignore = "BUG: ..."]` er reprodusenter som feiler på dagens kode.
//!   Kjør dem med: `cargo test -p fjell-cli --test redteam_sonnet -- --ignored`

use fjell_core::api::{Fjell, FjellApi};
use fjell_core::model::{AssetId, AssetSource, BatchId, Clock, SourceId, SuggestionKind};
use fjell_core::store::Store;
use fjell_fs::{
    DeleteError, DeleteMode, FakeRecycleBin, FsDeleter, FsQuarantine, Quarantine, QuarantineError,
    Reconciled,
};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

fn tempdir(tag: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!("fjell-rts-{tag}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&p);
    fs::create_dir_all(&p).unwrap();
    p
}

fn sid(s: &str) -> SourceId {
    SourceId(s.into())
}

struct Zero;
impl Clock for Zero {
    fn now(&self) -> i64 {
        1_700_000_000
    }
}

// ---------------------------------------------------------------- funn (reprodusenter)

/// RT-01: `stage` med samme id to ganger i ett kall. Andre rename feiler (filen er allerede
/// flyttet), og feilhåndteringen fjerner ALLE manifestoppføringer for id-en. Filen ligger da i
/// karantenen uten manifest: `unstage` gir NotFound og Angre/Gjenopprett er umulig.
#[test]
fn rt01_stage_duplicate_id_in_one_call_keeps_file_restorable() {
    let lib = tempdir("rt01");
    fs::write(lib.join("a.jpg"), b"data").unwrap();
    let q = FsQuarantine::in_library(&lib);
    let _ = q.stage(
        BatchId(1),
        &[(AssetId(1), sid("a.jpg")), (AssetId(1), sid("a.jpg"))],
    );
    assert!(!lib.join("a.jpg").exists(), "filen skulle vært flyttet");
    let r = q.unstage(&[AssetId(1)]);
    assert!(
        r[0].1.is_ok(),
        "fil i karantene kan ikke gjenopprettes: {:?}",
        r[0].1
    );
    assert_eq!(fs::read(lib.join("a.jpg")).unwrap(), b"data");
}

/// RT-02: id-gjenbruk. Id 1 ligger allerede i karantene for «old.jpg». `stage(1, "new.jpg")`
/// svarer Ok(()) uten å flytte `new.jpg`. Indeksen tror da at new.jpg er i papirkurven, og
/// `locate(1)` (brukt av «Tøm») peker på den GAMLE filen.
#[test]
#[ignore = "BUG: stage svarer Ok uten å flytte når id allerede er staged for en annen sti (id-gjenbruk)"]
fn rt02_stage_reused_id_must_not_report_ok_without_moving() {
    let lib = tempdir("rt02");
    fs::write(lib.join("old.jpg"), b"OLD").unwrap();
    fs::write(lib.join("new.jpg"), b"NEW").unwrap();
    let q = FsQuarantine::in_library(&lib);
    assert!(q.stage(BatchId(1), &[(AssetId(1), sid("old.jpg"))])[0]
        .1
        .is_ok());
    let r = q.stage(BatchId(2), &[(AssetId(1), sid("new.jpg"))]);
    let moved = !lib.join("new.jpg").exists();
    assert!(
        r[0].1.is_err() || moved,
        "stage meldte Ok, men new.jpg ligger fortsatt i biblioteket"
    );
    let located = q.locate(AssetId(1)).unwrap();
    assert_eq!(
        fs::read(located).unwrap(),
        if moved {
            b"NEW".to_vec()
        } else {
            b"OLD".to_vec()
        },
        "locate peker på feil fil"
    );
    // Konsekvens: Tøm av id 1 («new.jpg») ville slettet old.jpg-innholdet.
    assert!(
        moved,
        "Ok uten flytting er et stille brudd på indeks/karantene-samsvar"
    );
}

/// RT-03: manifestet er data på disk (kan komme fra synket mappe). `stored` valideres ikke:
/// `unstage` flytter en fil UTENFOR karantenen (her utenfor biblioteket) inn i biblioteket.
#[test]
fn rt03_manifest_stored_traversal_does_not_touch_foreign_files() {
    let base = tempdir("rt03");
    let lib = base.join("lib");
    let outside = base.join("outside");
    fs::create_dir_all(&lib).unwrap();
    fs::create_dir_all(&outside).unwrap();
    fs::write(outside.join("secret.jpg"), b"secret").unwrap();
    let bdir = lib.join(".fjell-karantene").join("9");
    fs::create_dir_all(&bdir).unwrap();
    fs::write(
        bdir.join("manifest.json"),
        r#"{"batch":9,"entries":[{"id":7,"original":"stolen.jpg","stored":"../../../outside/secret.jpg","bytes":6}]}"#,
    )
    .unwrap();
    let q = FsQuarantine::in_library(&lib);
    let _ = q.unstage(&[AssetId(7)]);
    assert!(
        outside.join("secret.jpg").exists(),
        "fil utenfor karantenen ble flyttet av unstage"
    );
    assert!(q
        .locate(AssetId(7))
        .is_none_or(|p| p.starts_with(lib.join(".fjell-karantene"))));
}

/// RT-04a: symlenke som peker på mappen selv. Skanningen lister samme fil under mange stier;
/// duplikatdeteksjonen foreslår da filen som duplikat av seg selv, og «slett duplikatet» flytter
/// den eneste kopien (og «beholderen» peker på samme fil).
#[cfg(unix)]
#[test]
fn rt04a_symlink_loop_lists_each_physical_file_once() {
    let root = tempdir("rt04a");
    fs::write(root.join("x.jpg"), b"only-copy").unwrap();
    std::os::unix::fs::symlink(".", root.join("loop")).unwrap();
    assert_unique_physical_files(&root);
}

/// RT-04b: symlenke til fil: `a.jpg -> x.jpg` regnes som eksakt duplikat av `x.jpg`. Keeper blir
/// alfabetisk første (symlenken), og «duplikatet» som foreslås slettet er den ekte filen.
#[cfg(unix)]
#[test]
fn rt04b_file_symlink_is_not_a_duplicate_of_its_target() {
    let root = tempdir("rt04b");
    fs::write(root.join("x.jpg"), b"only-copy").unwrap();
    std::os::unix::fs::symlink("x.jpg", root.join("a.jpg")).unwrap();
    assert_unique_physical_files(&root);
}

fn assert_unique_physical_files(root: &Path) {
    let src = fjell_cli::fsource::FsSource::new(root);
    let ids: Vec<_> = src
        .list_changes(None)
        .unwrap()
        .changed
        .into_iter()
        .map(|m| m.source_id)
        .collect();
    let mut canon: Vec<PathBuf> = ids
        .iter()
        .map(|i| fs::canonicalize(root.join(&i.0)).unwrap())
        .collect();
    let total = canon.len();
    canon.sort();
    canon.dedup();
    assert_eq!(
        canon.len(),
        total,
        "samme fysiske fil listet under {total} id-er: {ids:?}"
    );
}

/// RT-05: sjekken «indeksfilen må ligge utenfor kildemappen» er leksikalsk. En kilde-sti med
/// `..` omgår den, og indeksen (SQLite + WAL) skrives i brukerens mappe.
#[test]
fn rt05_index_db_inside_source_dir_is_rejected_even_with_dotdot() {
    let lib = tempdir("rt05");
    fs::create_dir_all(lib.join("sub")).unwrap();
    fs::write(lib.join("a.jpg"), b"x").unwrap();
    let dir_with_dotdot = lib.join("sub").join("..");
    let db = lib.join("index.db");
    let mut out = Vec::new();
    let r = fjell_cli::flow::index(&dir_with_dotdot, &db, &mut out);
    assert!(
        r.is_err(),
        "indeksfil ble skrevet inne i kildemappen: {}",
        db.exists()
    );
}

/// RT-06: `cards(kind, usize::MAX, ..)` regner `limit + 1` uten overflow-vern: panikk i debug,
/// i release blir grensen 0 og siden tom (kort «forsvinner»).
#[test]
fn rt06_cards_limit_max_does_not_panic_or_return_empty() {
    let root = tempdir("rt06");
    let work = tempdir("rt06w");
    fs::write(root.join("a.jpg"), b"same").unwrap();
    fs::write(root.join("b.jpg"), b"same").unwrap();
    let db = work.join("i.db");
    let mut out = Vec::new();
    fjell_cli::flow::index(&root, &db, &mut out).unwrap();
    let f = Fjell::new(
        Store::open(&db).unwrap(),
        Arc::new(fjell_cli::fsource::FsSource::new(&root)),
        Arc::new(Zero),
    );
    let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        f.cards(SuggestionKind::ExactDup, usize::MAX, None)
    }));
    let page = r.expect("cards panikket ved limit = usize::MAX").unwrap();
    assert_eq!(page.cards.len(), 1, "forventet ett kort");
}

/// RT-07 (bare Windows, IKKE kjørt her): `resolve` splitter bare på `/`. På Windows er `\` også
/// skilletegn, så `..\..\x` og `C:\x` slipper gjennom og `root.join` forlater roten.
#[cfg(windows)]
#[test]
#[ignore = "BUG: path::resolve avviser ikke backslash/stasjonsprefiks (Windows)"]
fn rt07_resolve_rejects_backslash_traversal_and_drive_paths_on_windows() {
    let root = Path::new(r"C:\lib");
    for bad in [
        r"..\..\x.jpg",
        r"a\..\..\x.jpg",
        r"C:\Windows\x.jpg",
        r"C:x.jpg",
    ] {
        assert!(fjell_fs::path::resolve(root, &sid(bad)).is_err(), "{bad}");
    }
}

// ---------------------------------------------------------------- angrep som holdt

#[test]
fn stage_rejects_traversal_and_absolute_ids_and_moves_nothing() {
    let base = tempdir("h1");
    let lib = base.join("lib");
    fs::create_dir_all(&lib).unwrap();
    fs::write(base.join("victim.jpg"), b"v").unwrap();
    let q = FsQuarantine::in_library(&lib);
    for bad in ["../victim.jpg", "/etc/passwd", "a/../../victim.jpg", ""] {
        let r = q.stage(BatchId(1), &[(AssetId(1), sid(bad))]);
        assert_eq!(r[0].1, Err(QuarantineError::NotFound), "{bad}");
    }
    assert!(base.join("victim.jpg").exists());
}

#[test]
fn restore_to_occupied_path_never_overwrites() {
    let lib = tempdir("h2");
    fs::write(lib.join("a.jpg"), b"ORIG").unwrap();
    let q = FsQuarantine::in_library(&lib);
    assert!(q.stage(BatchId(1), &[(AssetId(1), sid("a.jpg"))])[0]
        .1
        .is_ok());
    fs::write(lib.join("a.jpg"), b"NEWER").unwrap(); // noen opptok stien
    let r = q.unstage(&[AssetId(1)]);
    assert_eq!(r[0].1, Ok(sid("a (gjenopprettet).jpg")));
    assert_eq!(fs::read(lib.join("a.jpg")).unwrap(), b"NEWER");
    assert_eq!(
        fs::read(lib.join("a (gjenopprettet).jpg")).unwrap(),
        b"ORIG"
    );
}

#[test]
fn stage_unstage_roundtrip_preserves_bytes_unicode_and_long_names() {
    let lib = tempdir("h3");
    let name = format!("{}æøå 日本.jpg", "n".repeat(200));
    fs::create_dir_all(lib.join("sub dir")).unwrap();
    let rel = format!("sub dir/{name}");
    fs::write(lib.join(&rel), b"bytes").unwrap();
    let q = FsQuarantine::in_library(&lib);
    assert!(q.stage(BatchId(1), &[(AssetId(1), sid(&rel))])[0].1.is_ok());
    assert!(!lib.join(&rel).exists());
    assert_eq!(q.unstage(&[AssetId(1)])[0].1, Ok(sid(&rel)));
    assert_eq!(fs::read(lib.join(&rel)).unwrap(), b"bytes");
}

#[test]
fn double_stage_and_double_unstage_are_idempotent_safe() {
    let lib = tempdir("h4");
    fs::write(lib.join("a.jpg"), b"d").unwrap();
    let q = FsQuarantine::in_library(&lib);
    assert!(q.stage(BatchId(1), &[(AssetId(1), sid("a.jpg"))])[0]
        .1
        .is_ok());
    assert!(q.stage(BatchId(2), &[(AssetId(1), sid("a.jpg"))])[0]
        .1
        .is_ok());
    assert!(q.unstage(&[AssetId(1)])[0].1.is_ok());
    assert_eq!(
        q.unstage(&[AssetId(1)])[0].1,
        Err(QuarantineError::NotFound)
    );
    assert_eq!(fs::read(lib.join("a.jpg")).unwrap(), b"d");
}

#[test]
fn reconcile_offline_root_changes_nothing_and_marks_nothing_missing() {
    let base = tempdir("h5");
    let lib = base.join("lib");
    fs::create_dir_all(&lib).unwrap();
    fs::write(lib.join("a.jpg"), b"d").unwrap();
    let q = FsQuarantine::in_library(&lib);
    assert!(q.stage(BatchId(1), &[(AssetId(1), sid("a.jpg"))])[0]
        .1
        .is_ok());
    let gone = base.join("lib-away");
    fs::rename(&lib, &gone).unwrap(); // «koble fra»
    let r = q.reconcile(&[(AssetId(1), sid("a.jpg"))]);
    assert_eq!(r[0].1, Reconciled::Unavailable);
    fs::rename(&gone, &lib).unwrap();
    assert_eq!(q.unstage(&[AssetId(1)])[0].1, Ok(sid("a.jpg")));
}

#[test]
fn reconcile_crash_between_manifest_and_move_loses_nothing() {
    let lib = tempdir("h6");
    fs::write(lib.join("a.jpg"), b"d").unwrap();
    // Krasj etter manifest, før rename: manifest finnes, filen ligger på originalplass.
    let bdir = lib.join(".fjell-karantene/1");
    fs::create_dir_all(&bdir).unwrap();
    fs::write(
        bdir.join("manifest.json"),
        r#"{"batch":1,"entries":[{"id":1,"original":"a.jpg","stored":"1.jpg","bytes":1}]}"#,
    )
    .unwrap();
    let q = FsQuarantine::in_library(&lib);
    let r = q.reconcile(&[(AssetId(1), sid("a.jpg"))]);
    assert_eq!(r[0].1, Reconciled::NotMoved);
    assert_eq!(fs::read(lib.join("a.jpg")).unwrap(), b"d");
}

#[test]
fn permanent_delete_requires_ack_and_never_defaults_silently() {
    let lib = tempdir("h7");
    let bin = Arc::new(FakeRecycleBin::default());
    let r = FsDeleter::new(&lib, None, bin, DeleteMode::Permanent, None);
    assert!(matches!(r, Err(DeleteError::AckRequired)));
    assert!(fjell_fs::PermanentAck::new(false, 5).is_none());
    assert!(fjell_fs::PermanentAck::new(true, 0).is_none());
}
