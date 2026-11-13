//! Kritikerens reprodusenter etter rettelsene for F1..F5/RT-01..06 (docs/redteam/SAMMENLIGNING.md).
//! Kjør: `cargo test -p fjell-fs --test redteam_review -- --ignored`

mod common;
use common::*;
use fjell_core::api::{ApiError, Decision, Fjell, FjellApi, ScanConfig, ScanMode};
use fjell_core::model::{BatchId, Clock, PurgeTicket, SourceId};
// Brukes bare av Unix-testene under (symlenker); ellers ubrukt på Windows (clippy -D warnings).
#[cfg(unix)]
use fjell_core::model::AssetId;
use fjell_core::store::Store;
#[cfg(unix)]
use fjell_fs::PermanentAck;
use fjell_fs::{
    DeleteMode, FakeRecycleBin, FsDeleter, FsQuarantine, FsSource, Quarantine, QUARANTINE_DIR,
};
#[cfg(unix)]
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

struct Zero;
impl Clock for Zero {
    fn now(&self) -> i64 {
        1_700_000_000
    }
}

/// To identiske filer og én unik; duplikatet er trashed i indeksen (ikke staged). Billett for det.
fn world(tag: &str) -> (PathBuf, Fjell, PurgeTicket) {
    let root = tempdir(tag);
    let j = jpeg(64, 48, None);
    write(&root, "a.jpg", &j);
    write(&root, "dup/a2.jpg", &j);
    write(&root, "other.jpg", &jpeg(50, 50, None));
    let db = tempdir(&format!("{tag}-db")).join("i.db");
    let f = Fjell::new(
        Store::open(&db).unwrap(),
        Arc::new(FsSource::new(&root)),
        Arc::new(Zero),
    );
    f.start_scan(ScanConfig {
        mode: ScanMode::Full,
        priority: 1,
        deadline: None,
    })
    .unwrap();
    f.wait_scan().unwrap().unwrap();
    let cards = f
        .cards(fjell_core::model::SuggestionKind::ExactDup, 100, None)
        .unwrap()
        .cards;
    let ds: Vec<_> = cards.iter().map(|c| (c.id, Decision::Trash)).collect();
    match f.decide_many(ds.clone()) {
        Err(ApiError::AckRequired(codes)) => f.decide_many_confirmed(ds, codes).unwrap(),
        r => r.unwrap(),
    };
    let intent = f.begin_purge(vec![cards[0].asset]).unwrap();
    let ticket = match f.confirm_purge(&intent) {
        Err(ApiError::AckRequired(codes)) => f.confirm_purge_confirmed(&intent, codes).unwrap(),
        r => r.unwrap(),
    };
    (root, f, ticket)
}

/// KR-1: `FsDeleter::path_for` slår opp i karantenen bare på id (`locate(id)`), og sammenligner ikke
/// manifestets `original` med stien indeksen har for id-en. Ligger det en oppføring med samme id for
/// en ANNEN fil (samme mappe åpnet under to stavemåter gir to indekser som deler én karantene;
/// eller RT-02-tilfellet), svarer `stage` nå Err (filen blir «Merket, ikke flyttet»), men «Tøm»
/// sletter den andre filen under denne id-en.
#[test]
fn kr1_tom_sletter_ikke_karantenefil_som_tilhorer_en_annen_sti() {
    let (root, f, ticket) = world("kr1");
    let x = ticket.ids()[0];
    let sources = f.sources_for(ticket.ids());
    let q = Arc::new(FsQuarantine::in_library(&root));
    // Fremmed oppføring med samme id (fra en annen indeks over samme mappe).
    let foreign = q.stage(BatchId(900), &[(x, SourceId("other.jpg".into()))]);
    assert!(foreign[0].1.is_ok(), "oppsett");
    let held = root
        .join(QUARANTINE_DIR)
        .join("900")
        .join(format!("{}.jpg", x.0));
    assert!(held.is_file(), "oppsett: other.jpg ligger i karantenen");
    let r = q.stage(BatchId(1), &sources);
    assert!(r[0].1.is_err(), "oppsett: gjenbrukt id avvises (RT-02)");

    let bin = Arc::new(FakeRecycleBin::default());
    let d = FsDeleter::new(&root, Some(q), bin.clone(), DeleteMode::Recycle, None).unwrap();
    let res = d.delete_detailed(&ticket, &sources).unwrap();
    let calls = bin.calls.lock().unwrap().clone();
    assert!(
        calls.is_empty(),
        "BRUDD: bekreftet sletting av {:?}, men «Tøm» sendte {calls:?} (other.jpg) til Papirkurv ({res:?})",
        sources[0].1
    );
}

/// KR-2: `safe_stored` validerer bare strengen. Er selve batchmappen en symlenke ut av biblioteket,
/// peker et «trygt» filnavn fortsatt på en fil utenfor karantenen (samme angriper som F3: kan skrive
/// i karantenemappen; må i tillegg kunne legge et manifest i målmappen).
#[cfg(unix)]
#[test]
fn kr2_batchmappe_som_symlenke_sletter_ikke_utenfor_biblioteket() {
    let (root, _f, ticket) = world("kr2");
    let x = ticket.ids()[0];
    let ext = tempdir("kr2-utenfor");
    let viktig = ext.join("viktig.txt");
    fs::write(&viktig, b"utenfor biblioteket").unwrap();
    fs::write(
        ext.join("manifest.json"),
        format!(
            r#"{{"batch":7,"entries":[{{"id":{},"original":"dup/a2.jpg","stored":"viktig.txt","bytes":1}}]}}"#,
            x.0
        ),
    )
    .unwrap();
    fs::create_dir_all(root.join(QUARANTINE_DIR)).unwrap();
    std::os::unix::fs::symlink(&ext, root.join(QUARANTINE_DIR).join("7")).unwrap();

    let q = Arc::new(FsQuarantine::in_library(&root));
    let d = FsDeleter::new(
        &root,
        Some(q),
        Arc::new(FakeRecycleBin::default()),
        DeleteMode::Permanent,
        PermanentAck::new(true, 1),
    )
    .unwrap();
    let sources: Vec<(AssetId, SourceId)> = vec![(x, SourceId("dup/a2.jpg".into()))];
    let res = d.delete_detailed(&ticket, &sources).unwrap();
    assert!(
        viktig.is_file(),
        "BRUDD: fil utenfor biblioteket permanent slettet via symlenket batchmappe ({res:?})"
    );
}
