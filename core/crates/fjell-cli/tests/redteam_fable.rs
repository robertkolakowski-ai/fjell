//! Red team (Fable): reprodusenter for brudd på sikkerhetsmodellen. Rapport: docs/mvp/REDTEAM-FABLE.md.
//!
//! Hver test er `#[ignore = "BUG: ..."]` og FEILER på dagens kode når den kjøres med `--ignored`:
//! `cargo test -p fjell-cli --test redteam_fable -- --ignored`
//!
//! Testene bruker ekte `Fjell` + `FsSource` + `FsQuarantine` + `FsDeleter`. Limet mellom indeks og
//! filsystem (`stage_trashed`, `restore_undone`, `empty`) ligger i skallet
//! (desktop/src-tauri/shell/src/core.rs) og kan ikke nås herfra; hjelperne under gjentar de samme
//! kallene i samme rekkefølge (linjehenvisning ved hver hjelper).

use fjell_core::api::{Decision, Fjell, FjellApi, ScanConfig, ScanMode};
use fjell_core::model::{AssetId, BatchId, FakeClock, SourceId, State, SuggestionKind};
use fjell_core::store::Store;
use fjell_fs::{
    cloud_provider, DeleteMode, DeleteResult, FakeRecycleBin, FsDeleter, FsQuarantine, FsSource,
    PermanentAck, Quarantine, QUARANTINE_DIR,
};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

struct Env {
    root: PathBuf,
    lib: PathBuf,
    fjell: Arc<Fjell>,
    q: Arc<FsQuarantine>,
}

fn tmp(tag: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!("fjell-rt-fable-{tag}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&p);
    fs::create_dir_all(&p).unwrap();
    p
}

fn put(root: &Path, rel: &str, bytes: &[u8]) {
    let p = root.join(rel);
    fs::create_dir_all(p.parent().unwrap()).unwrap();
    fs::write(p, bytes).unwrap();
}

impl Env {
    /// Bibliotek med to eksakte kopier (`a/IMG_0001.jpg`, `b/IMG_0001.jpg`), skannet.
    fn new(tag: &str) -> Env {
        let root = tmp(tag);
        let lib = root.join("bibliotek");
        put(&lib, "a/IMG_0001.jpg", b"ORIGINALT-BILDE-innhold-0001");
        put(&lib, "b/IMG_0001.jpg", b"ORIGINALT-BILDE-innhold-0001");
        put(&lib, "a/annet.jpg", b"et helt annet bilde");
        let store = Store::open(&root.join("indeks.db")).unwrap();
        let src = FsSource::new(&lib).without_decoding();
        let fjell = Arc::new(Fjell::new(
            store,
            Arc::new(src),
            Arc::new(FakeClock::new(1_000)),
        ));
        let e = Env {
            q: Arc::new(FsQuarantine::in_library(&lib)),
            root,
            lib,
            fjell,
        };
        e.fjell
            .start_scan(ScanConfig {
                mode: ScanMode::Full,
                priority: 1,
                deadline: None,
            })
            .unwrap();
        e.fjell.wait_scan().unwrap().unwrap();
        e
    }

    /// Skallets `decide` + `Core::stage_trashed` (shell lib.rs 305–342, core.rs 180–183).
    fn trash_all_cards(&self) -> (BatchId, Vec<AssetId>) {
        self.fjell.sync_groups().unwrap();
        let cards = self
            .fjell
            .cards(SuggestionKind::ExactDup, 100, None)
            .unwrap()
            .cards;
        assert!(!cards.is_empty(), "oppsett: forventet et duplikatkort");
        let assets: Vec<AssetId> = cards.iter().map(|c| c.asset).collect();
        let batch = self
            .fjell
            .decide_many(cards.iter().map(|c| (c.id, Decision::Trash)).collect())
            .unwrap();
        let _ = self.q.stage(batch, &self.fjell.sources_for(&assets));
        (batch, assets)
    }

    /// `Core::restore_undone` (core.rs 178–193): alt i karantene som er `active` flyttes tilbake.
    fn restore_undone(&self) {
        let ids: Vec<AssetId> = self.q.staged().into_iter().map(|s| s.0).collect();
        let rows = self.fjell.with_store(|s| s.asset_rows(&ids)).unwrap();
        let back: Vec<AssetId> = ids
            .into_iter()
            .zip(rows)
            .filter(|(_, r)| r.as_ref().is_some_and(|r| r.state == State::Active))
            .map(|(i, _)| i)
            .collect();
        if !back.is_empty() {
            let _ = self.q.unstage(&back);
        }
    }

    /// Skallets `undo` (lib.rs 352–359).
    fn undo(&self) {
        self.fjell.undo().unwrap();
        self.restore_undone();
    }

    /// `Core::restore` (core.rs 135–150).
    fn restore(&self, ids: &[AssetId]) {
        self.fjell.restore_assets(ids).unwrap();
        self.restore_undone();
    }

    fn state(&self, id: AssetId) -> State {
        self.fjell.with_store(|s| s.asset_rows(&[id])).unwrap()[0]
            .as_ref()
            .unwrap()
            .state
    }

    fn path_of(&self, id: AssetId) -> PathBuf {
        let SourceId(rel) = self.fjell.sources_for(&[id])[0].1.clone();
        self.lib.join(rel)
    }

    /// `Core::empty` (core.rs 196–259), standardmodus (Papirkurv). Gir antall `Deleted`.
    fn empty(&self, ids: Vec<AssetId>, bin: Arc<FakeRecycleBin>) -> usize {
        let intent = self.fjell.begin_purge(ids).unwrap();
        let deleter = FsDeleter::new(
            &self.lib,
            Some(self.q.clone()),
            bin,
            DeleteMode::Recycle,
            None,
        )
        .unwrap();
        let ticket = self.fjell.confirm_purge(&intent).unwrap();
        let sources = self.fjell.sources_for(ticket.ids());
        let res = deleter.delete_detailed(&ticket, &sources).unwrap();
        let n = res
            .iter()
            .filter(|(_, r)| *r == DeleteResult::Deleted)
            .count();
        let outcomes = res
            .into_iter()
            .map(|(id, r)| {
                let o = if r == DeleteResult::Deleted {
                    fjell_core::model::DeleteOutcome::Deleted
                } else {
                    fjell_core::model::DeleteOutcome::Failed
                };
                (id, o)
            })
            .collect();
        let _ = self.fjell.finish_purge(ticket, outcomes);
        n
    }

    fn bin(&self) -> Arc<FakeRecycleBin> {
        Arc::new(FakeRecycleBin {
            bin_dir: Some(self.root.join("papirkurv")),
            ..Default::default()
        })
    }
}

/// F1 (høy). Gjenoppretting ved navnekollisjon gir `navn (gjenopprettet).jpg`, men resultatet av
/// `unstage` kastes (core.rs 191) og indeksens `source_id` peker fortsatt på den opprinnelige
/// stien, der det nå ligger en ANNEN fil. Neste trash + «Tøm» av det samme kortet flytter og sletter
/// den nye filen, som aldri har vært foreslått.
#[test]
#[ignore = "BUG: gjenoppretting ved kollisjon oppdaterer ikke indeksen; neste Tøm sletter en fil som aldri ble foreslått"]
fn f1_kollisjon_ved_angre_gjor_at_tom_sletter_en_uforeslatt_fil() {
    let e = Env::new("f1");
    let (_, assets) = e.trash_all_cards();
    let x = assets[0];
    let orig = e.path_of(x);
    assert!(!orig.exists(), "oppsett: filen skal ligge i karantene");
    // Kameraet teller fra 0001 igjen / brukeren kopierer inn et nytt bilde med samme navn.
    let nytt = b"NYTT-BILDE-som-aldri-er-skannet-eller-foreslatt";
    fs::write(&orig, nytt).unwrap();

    e.undo(); // indeks: aktiv. Fil: «IMG_0001 (gjenopprettet).jpg». Indeksen peker fortsatt på `orig`.
    assert_eq!(e.state(x), State::Active);

    // Brukeren tar samme kort på nytt og tømmer.
    let (_, again) = e.trash_all_cards();
    assert_eq!(again, vec![x], "oppsett: samme kort er ventende igjen");
    e.empty(vec![x], e.bin());

    let still_there = fs::read(&orig).ok().as_deref() == Some(nytt.as_slice());
    assert!(
        still_there,
        "BRUDD: det nye bildet på {} ble flyttet til karantene og slettet ved «Tøm» selv om det aldri \
         var foreslått; forslaget gjaldt et annet innhold",
        orig.display()
    );
}

/// F2 (høy). `FsDeleter::path_for` (delete.rs 124–129) faller tilbake til opprinnelig sti når id-en
/// ikke ligger i karantenen. Gjenopprettes bildet mellom `confirm_purge` og slettingen (parallelt
/// skallkall; `Core::empty` kaller ikke `check_ticket` per fil og holder ingen lås), slettes den
/// gjenopprettede, AKTIVE filen. `finish_purge` svarer `PurgeStale`, så indeksen sier `active`.
#[test]
fn f2_tom_sletter_fil_som_ble_gjenopprettet_etter_bekreftelsen() {
    let e = Env::new("f2");
    let (_, assets) = e.trash_all_cards();
    let x = assets[0];
    let orig = e.path_of(x);

    let intent = e.fjell.begin_purge(vec![x]).unwrap();
    let deleter = FsDeleter::new(
        &e.lib,
        Some(e.q.clone()),
        e.bin(),
        DeleteMode::Recycle,
        None,
    )
    .unwrap();
    let ticket = e.fjell.confirm_purge(&intent).unwrap();
    let sources = e.fjell.sources_for(ticket.ids());

    // En annen tråd rekker «Gjenopprett» her (vinduet varer hele sletteløkken).
    e.restore(&[x]);
    assert_eq!(e.state(x), State::Active);
    assert!(orig.is_file(), "oppsett: filen er tilbake på plass");

    let res = deleter.delete_detailed(&ticket, &sources).unwrap();
    let _ = e.fjell.finish_purge(ticket, Vec::new());

    assert_eq!(e.state(x), State::Active, "indeksen sier fortsatt aktiv");
    assert!(
        orig.is_file(),
        "BRUDD: aktiv, gjenopprettet fil ble slettet av «Tøm» (resultat {res:?}); indeksen sier `active`"
    );
}

/// F3 (middels). Manifestets `stored` brukes uvalidert (`quarantine.rs` `locate`: `d.join(&e.stored)`).
/// Et manifest med `..` (skadet, synkkonflikt eller endret av andre på en delt disk) får «Tøm» til
/// å slette en fil utenfor karantenen og utenfor biblioteket. `original` valideres (`resolve`),
/// `stored` ikke.
#[test]
#[ignore = "BUG: manifestfeltet `stored` valideres ikke; Tøm kan slette en fil utenfor karantenen/biblioteket"]
fn f3_manifest_stored_med_dotdot_sletter_fil_utenfor_biblioteket() {
    let e = Env::new("f3");
    let (batch, assets) = e.trash_all_cards();
    let x = assets[0];
    let viktig = e.root.join("viktig.txt");
    fs::write(&viktig, b"ligger utenfor biblioteket").unwrap();

    let mf = e
        .lib
        .join(QUARANTINE_DIR)
        .join(batch.0.to_string())
        .join("manifest.json");
    let raw = fs::read_to_string(&mf).unwrap();
    let stored = format!("\"{}.jpg\"", x.0);
    assert!(
        raw.contains(&stored),
        "oppsett: fant ikke stored i manifestet"
    );
    fs::write(&mf, raw.replace(&stored, "\"../../../viktig.txt\"")).unwrap();

    let intent = e.fjell.begin_purge(vec![x]).unwrap();
    let ticket = e.fjell.confirm_purge(&intent).unwrap();
    let deleter = FsDeleter::new(
        &e.lib,
        Some(e.q.clone()),
        e.bin(),
        DeleteMode::Permanent,
        PermanentAck::new(true, 1),
    )
    .unwrap();
    let res = deleter
        .delete_detailed(&ticket, &e.fjell.sources_for(ticket.ids()))
        .unwrap();
    assert!(
        viktig.is_file(),
        "BRUDD: fil utenfor biblioteket ble permanent slettet via manifestets `stored` ({res:?})"
    );
}

/// F4 (middels). Angre av en «Gjenopprett» setter bildet til `trashed` i indeksen, men skallets
/// `undo` kaller bare `restore_undone` (lib.rs 352–359): filen flyttes ikke tilbake til
/// karantenen. Bildet står i «Nylig slettet» mens filen ligger synlig i biblioteket, og «Tøm»
/// sletter den derfra. Angre gjenoppretter altså ikke eksakt tilstand, og indeks og karantene
/// stemmer ikke (oppstartsavstemmingen svarer bare `NotMoved` og retter ingenting).
#[test]
#[ignore = "BUG: angre av Gjenopprett gir `trashed` i indeksen uten at filen legges tilbake i karantenen"]
fn f4_angre_av_gjenopprett_legger_ikke_filen_tilbake_i_karantenen() {
    let e = Env::new("f4");
    let (_, assets) = e.trash_all_cards();
    let x = assets[0];
    assert!(e.q.locate(x).is_some(), "oppsett: i karantene etter trash");
    e.restore(&[x]);
    assert!(e.path_of(x).is_file());
    e.undo(); // angrer gjenopprettingen
    assert_eq!(
        e.state(x),
        State::Trashed,
        "oppsett: indeksen er trashed igjen"
    );
    assert!(
        e.q.locate(x).is_some() && !e.path_of(x).exists(),
        "BRUDD: bildet er `trashed` i indeksen, men filen ligger i biblioteket og ikke i karantenen"
    );
}

/// F5 (middels). Skysperren ser bare på bibliotekstien som tekst (`cloud_provider`, shell lib.rs
/// 236–243). En undermappe som er symlenke/junction til en synkmappe følges av `FsSource::walk`
/// (`is_dir` følger lenker), og `stage` flytter filen UT av synkmappen med rename. Synkklienten
/// ser det som sletting og sletter i skyen og på andre enheter.
#[cfg(unix)]
#[test]
#[ignore = "BUG: symlenket synkmappe inne i biblioteket omgår skysperren; trash flytter filen ut av synkmappen"]
fn f5_symlenket_synkmappe_i_biblioteket_omgar_skysperren() {
    let root = tmp("f5");
    let sky = root.join("OneDrive").join("Bilder");
    put(&sky, "ferie.jpg", b"synket bilde");
    let lib = root.join("bibliotek");
    put(&lib, "lokal.jpg", b"lokalt bilde");
    std::os::unix::fs::symlink(&sky, lib.join("album")).unwrap();
    assert!(cloud_provider(&sky.to_string_lossy()).is_some());
    // Skallet sjekker bare denne strengen:
    assert!(cloud_provider(&lib.to_string_lossy()).is_none());

    use fjell_core::model::AssetSource;
    let listed = FsSource::new(&lib)
        .without_decoding()
        .list_changes(None)
        .unwrap();
    let ids: Vec<String> = listed
        .changed
        .iter()
        .map(|m| m.source_id.0.clone())
        .collect();
    let via_link = ids.iter().any(|s| s == "album/ferie.jpg");

    let q = FsQuarantine::in_library(&lib);
    let r = q.stage(
        BatchId(1),
        &[(AssetId(1), SourceId("album/ferie.jpg".into()))],
    );
    assert!(
        !via_link && sky.join("ferie.jpg").is_file(),
        "BRUDD: fil i synkmappe ble listet (via lenke: {via_link}) og flyttet ut av synkmappen \
         ({r:?}); skysperren slo aldri inn"
    );
}
