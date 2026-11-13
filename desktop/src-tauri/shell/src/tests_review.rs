//! Kritikerens reprodusent gjennom `Shell` (se også core/crates/fjell-fs/tests/redteam_review.rs).
//! Kjør: `cargo test --locked -p fjell-shell tests_review -- --ignored`

use crate::dto::DecisionDto;
use crate::Shell;
use fjell_fs::{FakeRecycleBin, FsSource, RecycleBin};
use std::fs;
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

const DUP_A: &[u8] = b"par-A-par-A-par-A-par-A";
const DUP_B: &[u8] = b"par-B-par-B-par-B-par-B-par-B";

fn open(sh: &Shell, lib: &Path) {
    let src = FsSource::new(lib).without_decoding();
    sh.open_library(lib, Arc::new(src)).unwrap();
}

fn scan(sh: &Shell) {
    sh.start_scan(true).unwrap();
    let t = Instant::now();
    while sh.state().scan_phase != "done" {
        assert!(t.elapsed() < Duration::from_secs(20), "skanning henger");
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn trash_all(sh: &Shell) {
    let cards = sh.cards("exact_dup", 50, None).unwrap().cards;
    let ids: Vec<u64> = cards.iter().map(|c| c.id).collect();
    assert!(!ids.is_empty(), "oppsett: duplikatkort");
    sh.decide(&ids, DecisionDto::Trash).unwrap();
}

/// KR-3: indeksen velges av hash over bibliotekstien som TEKST (`config::db_path`), mens karantenen
/// ligger i selve mappen og er nøklet på asset-id. Samme mappe åpnet under to stavemåter gir to
/// indekser med hver sin id-rekke og én felles karantene. «Tøm» slår opp karantenefilen bare på id
/// (KR-1), så indeks A kan slette en fil den aldri har vist i «Nylig slettet».
#[test]
#[ignore = "BUG: to stavemåter av samme bibliotek deler karantene (årsak rettet: db_path kanoniserer, se kr3_to_stavemater_gir_samme_indeks). Oppsettet forutsetter to indekser og feiler nå på «oppsett: duplikatkort» (ukjent: skjules ny gruppe av gammel beslutning i samme indeks?)"]
fn kr3_samme_mappe_to_stavemater_tom_sletter_bare_det_som_ble_vist() {
    let root = std::env::temp_dir().join(format!("fjell-review-kr3-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    let (lib, data) = (root.join("bibliotek"), root.join("appdata"));
    fs::create_dir_all(&lib).unwrap();
    fs::create_dir_all(&data).unwrap();
    fs::write(lib.join("m.jpg"), DUP_A).unwrap();
    fs::write(lib.join("n.jpg"), DUP_A).unwrap();
    let bin = Arc::new(FakeRecycleBin {
        bin_dir: Some(root.join("papirkurv")),
        ..Default::default()
    });
    let sh = Shell::with_data_dir(&data, bin.clone() as Arc<dyn RecycleBin>);

    // Indeks A: par A i «Nylig slettet».
    open(&sh, &lib);
    scan(&sh);
    trash_all(&sh);
    let shown_a: Vec<String> = sh.trashed().unwrap().into_iter().map(|t| t.name).collect();
    assert_eq!(shown_a.len(), 1);

    // Indeks B: samme mappe, annen stavemåte. Nye filer som sorteres først får de lave id-ene.
    let lib_b = lib.join(".");
    fs::write(lib.join("0a.jpg"), DUP_B).unwrap();
    fs::write(lib.join("0b.jpg"), DUP_B).unwrap();
    open(&sh, &lib_b);
    scan(&sh);
    trash_all(&sh);

    // Tilbake til A: «Nylig slettet» viser fortsatt bare det A la der.
    open(&sh, &lib);
    let shown: Vec<String> = sh.trashed().unwrap().into_iter().map(|t| t.name).collect();
    assert_eq!(shown, shown_a, "oppsett: A viser samme fil som før");
    let _ = sh.empty(&[], None);

    let binned: Vec<Vec<u8>> = fs::read_dir(root.join("papirkurv"))
        .map(|rd| {
            rd.filter_map(|f| f.ok())
                .map(|f| fs::read(f.path()).unwrap())
                .collect()
        })
        .unwrap_or_default();
    let wrong = binned.iter().any(|b| b == DUP_B);
    let _ = fs::remove_dir_all(&root);
    assert!(
        !wrong,
        "BRUDD: «Tøm» i indeks A (viste {shown_a:?}) sendte en fil fra par B til Papirkurv"
    );
}

/// KR-3 (årsak): to stavemåter av samme mappe gir samme indeksfil, så de kan ikke dele karantene
/// med to id-rekker.
#[test]
fn kr3_to_stavemater_gir_samme_indeks() {
    let root = std::env::temp_dir().join(format!("fjell-review-kr3b-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    let (lib, data) = (root.join("bibliotek"), root.join("appdata"));
    fs::create_dir_all(&lib).unwrap();
    let a = crate::config::db_path(&data, &lib);
    assert_eq!(a, crate::config::db_path(&data, &lib.join(".")));
    assert_eq!(
        a,
        crate::config::db_path(&data, &root.join("bibliotek").join(""))
    );
    let _ = fs::remove_dir_all(&root);
}
