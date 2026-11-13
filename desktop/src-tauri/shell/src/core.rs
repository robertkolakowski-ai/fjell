//! Ekte kjerne for ett bibliotek: `Fjell` + `FsSource` + indeks i app-data + karantene + «Tøm».
//!
//! Avhenger av: `fjell-core::api` (`Fjell`), `fjell-fs` (`FsQuarantine`, `FsDeleter`, `RecycleBin`).
//! Grensesnitt: `Core::open_roots`, `flow`, `flow_live`, `stage_trashed`,
//! `undo`, `restore`, `reconcile`, `empty`, `thumbnail` (kun lesing), `roots`, `cloud_block`, `retire`.
//!
//! Flere røtter (docs/mvp/FLERE-MAPPER.md): ett bibliotek = én indeks og en ordnet liste røtter, hver
//! med egen karantene (`<rot>/.fjell-karantene`). Alt som flytter eller sletter filer rutes på
//! rotprefikset i indeksens sti (`fjell_fs::RootSet`): en id fra rot A slås aldri opp i karantene B,
//! og en sti uten kjent rot flyttes og slettes aldri.
//!
//! Regler (CLAUDE.md, ARKITEKTUR 17/20/22):
//! - Trash er indeks + flytting til karantene (rename); undo flytter tilbake. Aldri sletting.
//! - «Tøm» går bare via `begin_purge` → `confirm_purge` → Papirkurv (standard) → `finish_purge`.
//!   Permanent krever `PermanentAck` (avkrysning + riktig antall), og `Unsupported` volum faller
//!   aldri stille tilbake til permanent.
//! - Rekkefølge: indeksen først, filflytting etterpå. Brudd midt imellom rettes av `reconcile`.
//! - Invariant (F1/F2/F4): «Tøm» sletter bare filer som på Tøm-tidspunktet er i «Nylig slettet» OG som
//!   karantenen faktisk inneholder. Derfor (1) holder hver sammensatt overgang (trash + flytting,
//!   angre, gjenoppretting, «Tøm», avstemming) `flow`-låsen fra indeksendring til filflytting er
//!   ferdig, (2) følger indeksen filen når karantenen gjenoppretter til et annet navn (`relocate`),
//!   og (3) legges filen tilbake i karantenen når en gjenoppretting angres (`restage_trashed`).

use crate::dto::{EmptyDto, ErrorDto, PermanentAckDto, RestoreDto, RootDto, TrashedDto};
use crate::thumb;
use fjell_core::api::{Fjell, FjellApi};
use fjell_core::model::{
    AssetId, AssetSource, BatchId, Clock, ConsequenceCode, DeleteOutcome, SourceId, State, Thumb,
};
use fjell_core::store::Store;
use fjell_fs::locality::LocalProbe;
use fjell_fs::path::{os_path, resolve};
use fjell_fs::roots::{Root, PRIMARY_KEY};
use fjell_fs::{
    DeleteMode, DeleteResult, FsDeleter, PermanentAck, Reconciled, RecycleBin, RootSet,
};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

pub(crate) struct Core {
    pub fjell: Arc<Fjell>,
    pub library: PathBuf,
    /// Indeksfilen denne kjernen har åpen (kan være gammelt navn etter mislykket migrering).
    pub db: PathBuf,
    /// Alle røtter (første = `library`) med karantene per rot.
    roots: RootSet,
    source: Arc<dyn AssetSource>,
    /// Samme sjekk som kilden bruker for skyplassholdere; brukes bare på metadata (aldri innhold).
    probe: LocalProbe,
    /// Skyleverandør per rot, slått opp ved åpning (også via lenkemål, se `root_cloud`). Brukes av
    /// `state()`-pollingen, som ikke skal røre filsystemet.
    cloud: BTreeMap<u32, Option<&'static str>>,
    /// Én om gangen av overgangene som endrer både indeks og karantene (se invarianten øverst).
    flow: Mutex<()>,
    /// Satt (under `flow`) når skallet har byttet ut denne kjernen (nytt bibliotek, rot lagt til
    /// eller fjernet). En pågående handling som fikk tak i den gamle kjernen avvises da i stedet for
    /// å flytte filer etter en rotliste som ikke gjelder lenger.
    retired: AtomicBool,
}

/// Mappen i en `SourceId` (alt før siste `/`).
fn folder(s: &SourceId) -> &str {
    s.0.rsplit_once('/').map_or("", |x| x.0)
}

/// Siste sti-ledd (Windows- og Unix-skille). Aldri hele stien til frontend.
fn file_name(source: &str) -> String {
    source
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or(source)
        .to_string()
}

fn not_found() -> ErrorDto {
    ErrorDto::new("not_found", "")
}

fn internal(e: impl std::fmt::Display) -> ErrorDto {
    ErrorDto::new("internal", e.to_string())
}

impl Core {
    /// Åpner indeksen på `db` (må ligge utenfor alle røtter) og avstemmer karantenen i hver rot.
    /// `roots` er bibliotekets røtter (første har nøkkel 1 og eier indeksen); `source` må dekke de
    /// samme røttene med samme nøkler. Første rot må finnes; øvrige kan være frakoblet.
    /// Overlappende røtter avvises (`root_overlap`). `probe` er plassholdersjekken (standard i
    /// produksjon; injiseres i tester på ikke-Windows).
    pub fn open_roots(
        roots: &[(u32, PathBuf)],
        db: &Path,
        source: Arc<dyn AssetSource>,
        clock: Arc<dyn Clock>,
        probe: LocalProbe,
    ) -> Result<Core, ErrorDto> {
        let set = RootSet::new(roots).map_err(|e| ErrorDto::new("root_overlap", e.to_string()))?;
        let library = set.roots()[0].path.clone();
        if !library.is_dir() {
            return Err(ErrorDto::new("not_a_folder", library.display().to_string()));
        }
        let canon_db = std::fs::canonicalize(db.parent().unwrap_or(db)).ok();
        for r in set.roots() {
            let inside = db.starts_with(&r.path)
                || matches!(
                    (&canon_db, std::fs::canonicalize(&r.path)),
                    (Some(d), Ok(p)) if d.starts_with(&p)
                );
            if inside {
                return Err(ErrorDto::bad_request(
                    "indeksen må ligge utenfor biblioteket",
                ));
            }
        }
        let store = Store::open(db).map_err(internal)?;
        let cloud = set
            .roots()
            .iter()
            .map(|r| (r.key, Core::root_cloud(r)))
            .collect();
        let core = Core {
            fjell: Arc::new(Fjell::new(store, source.clone(), clock)),
            library,
            db: db.to_path_buf(),
            roots: set,
            source,
            probe,
            cloud,
            flow: Mutex::new(()),
            retired: AtomicBool::new(false),
        };
        core.reconcile();
        Ok(core)
    }

    /// Låsen for sammensatte overganger. Tas FØR indeksen endres og holdes til filene er flyttet.
    pub fn flow(&self) -> MutexGuard<'_, ()> {
        self.flow.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Som `flow`, men avviser (`unavailable`) hvis kjernen er byttet ut mens kalleren ventet.
    pub fn flow_live(&self) -> Result<MutexGuard<'_, ()>, ErrorDto> {
        let g = self.flow();
        if self.retired.load(Ordering::SeqCst) {
            return Err(ErrorDto::new("unavailable", "biblioteket ble endret"));
        }
        Ok(g)
    }

    /// Pensjonerer kjernen: alle senere overganger avvises. Kalleren holder `flow` (argumentet er
    /// beviset), så ingen overgang er underveis når flagget settes.
    pub fn retire(&self, _flow: &MutexGuard<'_, ()>) {
        self.retired.store(true, Ordering::SeqCst);
    }

    pub fn roots(&self) -> &[Root] {
        self.roots.roots()
    }

    /// Skyleverandør for roten: kjente mappenavn i stien brukeren valgte ELLER i stien den faktisk
    /// peker på (kanonisert). En rot som er en lenke (symlenke, junction) inn i en skymappe er
    /// like skysynket som målet (RT-FM-3). Frakoblet rot kan ikke kanoniseres: bare teksten gjelder.
    fn root_cloud(root: &Root) -> Option<&'static str> {
        fjell_fs::cloud_provider(&root.path.to_string_lossy())
            .or_else(|| {
                let real = std::fs::canonicalize(os_path(&root.path)).ok()?;
                fjell_fs::cloud_provider(&real.to_string_lossy())
            })
            .map(|p| p.code())
    }

    /// Som `root_cloud`, men fra oppslaget gjort ved åpning (ingen filsystemtilgang).
    fn root_cloud_cached(&self, root: &Root) -> Option<&'static str> {
        self.cloud.get(&root.key).copied().flatten()
    }

    /// Leverandørkode hvis ALLE røtter er skysynket (da er all papirkurv sperret).
    pub fn cloud_all(&self) -> Option<&'static str> {
        let codes: Vec<Option<&'static str>> = self
            .roots()
            .iter()
            .map(|r| self.root_cloud_cached(r))
            .collect();
        match codes.first() {
            Some(Some(first)) if codes.iter().all(|c| c.is_some()) => Some(first),
            _ => None,
        }
    }

    /// Skysperre per rot: leverandørkoden hvis minst ett av bildene ligger i en skysynket rot.
    /// Bilder i andre røtter berøres ikke av sperren. Slås opp på nytt her (ikke fra åpningen), så
    /// en rot som var frakoblet ved åpning eller har fått lenkemålet byttet også sperres.
    pub fn cloud_block(&self, assets: &[AssetId]) -> Option<&'static str> {
        self.fjell
            .sources_for(assets)
            .iter()
            .filter_map(|(_, sid)| self.roots.route(sid))
            .find_map(|(root, _)| {
                self.root_cloud_cached(root)
                    .or_else(|| Core::root_cloud(root))
            })
    }

    /// Høyeste rotnøkkel noen rad i indeksen har (uansett tilstand), minst `PRIMARY_KEY`. Nye
    /// røtter må få en høyere nøkkel: går `shell.json` tapt, vet bare indeksen hvilke nøkler som
    /// er brukt, og en gjenbrukt nøkkel ville latt en gammel rad `/r2/…` i «Nylig slettet» peke
    /// inn i en ny mappe (RT-FM-2). Kan indeksen ikke leses, gis feil (ingen nøkkel deles ut).
    pub fn max_root_key(&self) -> Result<u32, ErrorDto> {
        self.fjell.with_store(|s| {
            let conn = s
                .reader()
                .map_err(internal)?
                .ok_or_else(|| internal("indeksen kan ikke leses"))?;
            let mut st = conn
                .prepare("SELECT source_id FROM asset WHERE source_id LIKE '/r%'")
                .map_err(internal)?;
            let rows = st
                .query_map([], |r| r.get::<_, String>(0))
                .map_err(internal)?;
            let mut max = PRIMARY_KEY;
            for sid in rows {
                let sid = SourceId(sid.map_err(internal)?);
                if let Some((key, _)) = fjell_fs::roots::split(&sid) {
                    max = max.max(key);
                }
            }
            Ok(max)
        })
    }

    /// Antall i «Nylig slettet» med sti i en av røttene `keys`. Brukes før røtter tas ut av
    /// biblioteket (de må da ikke ha bilder i papirkurven).
    pub fn trashed_in(&self, keys: &[u32]) -> u64 {
        self.fjell
            .sources_for(&self.trashed_ids())
            .iter()
            .filter_map(|(_, sid)| fjell_fs::roots::split(sid))
            .filter(|(key, _)| keys.contains(key))
            .count() as u64
    }

    /// Røttene uten filsystemtilgang og uten tall (til `StateDto`).
    pub fn root_dtos(&self) -> Vec<RootDto> {
        self.roots()
            .iter()
            .map(|r| RootDto {
                key: r.key,
                path: r.path.to_string_lossy().into_owned(),
                primary: r.key == PRIMARY_KEY,
                cloud_sync: self.root_cloud_cached(r),
                quarantine_dir: r
                    .path
                    .join(fjell_fs::QUARANTINE_DIR)
                    .to_string_lossy()
                    .into_owned(),
                online: None,
                count: None,
                bytes: None,
                videos: None,
                video_bytes: None,
                trashed: None,
            })
            .collect()
    }

    /// Røttene med tall fra indeksen (aktive bilder/video og «Nylig slettet») og om mappen er
    /// tilgjengelig nå. Én gjennomlesing av indeksen; ingen bildefiler åpnes.
    pub fn root_stats(&self) -> Vec<RootDto> {
        #[derive(Default, Clone, Copy)]
        struct Sum {
            count: u64,
            bytes: u64,
            videos: u64,
            video_bytes: u64,
            trashed: u64,
        }
        let sums: Option<BTreeMap<u32, Sum>> = self.fjell.with_store(|s| {
            let conn = s.reader().ok().flatten()?;
            let mut st = conn
                .prepare("SELECT source_id, bytes, state, kind FROM asset WHERE state IN (0, 1)")
                .ok()?;
            let rows = st
                .query_map([], |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        r.get::<_, i64>(1)?,
                        r.get::<_, i64>(2)?,
                        r.get::<_, String>(3)?,
                    ))
                })
                .ok()?;
            let mut out: BTreeMap<u32, Sum> = BTreeMap::new();
            for (sid, bytes, state, kind) in rows.filter_map(|r| r.ok()) {
                let Some((key, _)) = fjell_fs::roots::split(&SourceId(sid)) else {
                    continue;
                };
                let e = out.entry(key).or_default();
                let bytes = bytes.max(0) as u64;
                if state == 1 {
                    e.trashed += 1;
                } else {
                    e.count += 1;
                    e.bytes += bytes;
                    if kind == "video" {
                        e.videos += 1;
                        e.video_bytes += bytes;
                    }
                }
            }
            Some(out)
        });
        let mut dtos = self.root_dtos();
        for (d, r) in dtos.iter_mut().zip(self.roots()) {
            d.online = Some(os_path(&r.path).is_dir());
            if let Some(sum) = sums
                .as_ref()
                .map(|m| m.get(&r.key).copied().unwrap_or_default())
            {
                d.count = Some(sum.count);
                d.bytes = Some(sum.bytes);
                d.videos = Some(sum.videos);
                d.video_bytes = Some(sum.video_bytes);
                d.trashed = Some(sum.trashed);
            }
        }
        dtos
    }

    /// Alle id-er i «Nylig slettet». Leser indeksen med egen lesetilkobling; faller tilbake til
    /// det som ligger i karantene hvis lesing ikke er mulig.
    fn trashed_ids(&self) -> Vec<AssetId> {
        let from_db = self.fjell.with_store(|s| {
            let conn = s.reader().ok().flatten()?;
            let mut st = conn
                .prepare("SELECT id FROM asset WHERE state = 1 ORDER BY id")
                .ok()?;
            let rows = st.query_map([], |r| r.get::<_, i64>(0)).ok()?;
            Some(
                rows.filter_map(|r| r.ok())
                    .map(|i| AssetId(i as u64))
                    .collect(),
            )
        });
        from_db.unwrap_or_else(|| self.staged_still_trashed())
    }

    fn staged_still_trashed(&self) -> Vec<AssetId> {
        let ids: Vec<AssetId> = self.roots.staged().into_iter().map(|s| s.1).collect();
        let rows = self
            .fjell
            .with_store(|s| s.asset_rows(&ids))
            .unwrap_or_default();
        ids.into_iter()
            .zip(rows)
            .filter(|(_, r)| r.as_ref().is_some_and(|r| r.state == State::Trashed))
            .map(|(i, _)| i)
            .collect()
    }

    /// Karantenemappen (`<bibliotek>/.fjell-karantene`). Vises i Innstillinger; ingenting leses fra frontend.
    pub fn quarantine_dir(&self) -> PathBuf {
        self.library.join(fjell_fs::QUARANTINE_DIR)
    }

    /// Alle bilder i «Nylig slettet», eldste først. `now` brukes bare til `expired`.
    pub fn trashed(&self, now: i64) -> Vec<TrashedDto> {
        let ids = self.trashed_ids();
        let rows = self
            .fjell
            .with_store(|s| s.asset_rows(&ids))
            .unwrap_or_default();
        let sources = self.fjell.sources_for(&ids);
        let mut out: Vec<TrashedDto> = rows
            .into_iter()
            .flatten()
            .filter(|r| r.state == State::Trashed)
            .map(|r| {
                let name = sources
                    .iter()
                    .find(|(id, _)| *id == r.id)
                    .map(|(_, src)| file_name(&src.0))
                    .unwrap_or_default();
                let expires_at = r.expires_at.unwrap_or(i64::MAX);
                TrashedDto {
                    asset: r.id.0,
                    name,
                    bytes: r.bytes,
                    trashed_at: r.trashed_at.unwrap_or(0),
                    expires_at,
                    expired: expires_at <= now,
                }
            })
            .collect();
        out.sort_by_key(|t| (t.trashed_at, t.asset));
        out
    }

    /// Miniatyr for ett bilde (`id` = `AssetId`, det samme som `CardDto.asset`/`TrashedDto.asset`).
    ///
    /// Id-oversettelse: UI-ets kort-id (`CardDto.id`, `CardId`) er IKKE en `AssetId`; UI sender
    /// `card.asset` (og `card.keeper`). Papirkurvrader bruker `TrashedDto.asset` direkte.
    /// Papirkurv: miniatyrbufferen er nøklet på (rot, kilde-sti, mtime, størrelse) og treffer ikke
    /// etter flytting til karantene, så trashede bilder leses fra karantenens lagrede sti og
    /// dekodes uten buffer. Oppslaget er det samme som «Tøm» bruker (`FsQuarantine::locate_for`:
    /// manifestets `original` må være stien indeksen har, `stored` er validert, symlenket
    /// batchmappe hoppes over). Ikke flyttet (annet volum) leses fra originalstien via kilden.
    /// Skyplassholdere leses aldri: bare metadata sjekkes. Filer åpnes bare for lesing; ingenting
    /// flyttes, slettes eller skrives til indeks/manifest herfra.
    ///
    /// Låsing (valg): `flow` tas IKKE. «Tøm» holder `flow` gjennom hele slettingen (kan ta lang
    /// tid mot Papirkurv), og en ren lesing skal ikke vente på den. Indeksoppslaget tar bare den
    /// korte kjernelåsen (`with_store`), som ingen holder under filflytting. Uten `flow` kan
    /// oppslaget bli foreldet mens filen leses (gjenoppretting med nytt navn, angre, «Tøm»).
    /// Derfor leses indeksen på nytt etterpå; er (tilstand, sti) endret, forkastes resultatet og
    /// det prøves én gang til. Verste utfall av et kappløp er en forbigående `not_found` (UI
    /// laster på nytt), aldri et bilde fra en fil utenfor biblioteket og aldri en endring på disk,
    /// så Tøm-invarianten berøres ikke.
    pub fn thumbnail(&self, id: AssetId) -> Result<Thumb, ErrorDto> {
        let mut seen = self.thumb_lookup(id)?;
        for _ in 0..2 {
            let (state, sid) = &seen;
            let r = match state {
                State::Active => self.thumb_original(sid),
                State::Trashed => match self.roots.locate_for(id, sid) {
                    Some(p) => self.thumb_quarantined(sid, &p),
                    None => self.thumb_original(sid),
                },
                State::Purged => Err(not_found()),
            };
            let after = match self.thumb_lookup(id) {
                Ok(a) => a,
                Err(_) => return r,
            };
            if after == seen {
                return r;
            }
            seen = after;
        }
        // Endret seg to ganger på rad: heller «prøv igjen» enn et mulig feil bilde.
        Err(ErrorDto::new("unavailable", "endret under lesing"))
    }

    /// Tilstand og kilde-sti for `id`, lest under den korte kjernelåsen (ikke `flow`).
    fn thumb_lookup(&self, id: AssetId) -> Result<(State, SourceId), ErrorDto> {
        let (row, src) = self
            .fjell
            .with_store(|s| (s.asset_row(id), s.source_ids(&[id])));
        let row = row.map_err(internal)?.ok_or_else(not_found)?;
        let sid = src
            .map_err(internal)?
            .into_iter()
            .next()
            .map(|(_, s)| s)
            .ok_or_else(not_found)?;
        Ok((row.state, sid))
    }

    fn thumb_original(&self, sid: &SourceId) -> Result<Thumb, ErrorDto> {
        let (root, rel) = self.roots.route(sid).ok_or_else(not_found)?;
        let p = resolve(&root.path, &rel).map_err(thumb::map_error)?;
        self.check_contained(root, &p)?;
        self.check_local(&p)?;
        self.source.thumbnail(sid).map_err(thumb::map_error)
    }

    fn thumb_quarantined(&self, sid: &SourceId, p: &Path) -> Result<Thumb, ErrorDto> {
        let (root, _) = self.roots.route(sid).ok_or_else(not_found)?;
        self.check_contained(root, p)?;
        self.check_local(p)?;
        fjell_fs::thumb::make_thumb(&os_path(p)).map_err(thumb::map_error)
    }

    /// Filen må være en vanlig fil (ikke symlenke), og mappen den ligger i må etter kanonisering
    /// ligge i bildets EGEN rot (karantenen ligger også der). En symlenket fil, undermappe eller
    /// karantenemappe kunne ellers få miniatyrlesingen til å lese utenfor biblioteket. Bare
    /// metadata leses her. Avviste filer gir `not_found` (stien røpes ikke).
    fn check_contained(&self, root: &Root, p: &Path) -> Result<(), ErrorDto> {
        let md = std::fs::symlink_metadata(os_path(p)).map_err(|e| match e.kind() {
            std::io::ErrorKind::NotFound => not_found(),
            _ => ErrorDto::new("unavailable", e.to_string()),
        })?;
        let lib = std::fs::canonicalize(os_path(&root.path));
        let dir = p.parent().map(|d| std::fs::canonicalize(os_path(d)));
        match (lib, dir) {
            (Ok(l), Some(Ok(d))) if md.is_file() && d.starts_with(&l) => Ok(()),
            _ => Err(not_found()),
        }
    }

    /// `not_found`/`cloud_placeholder` fra metadata alene.
    fn check_local(&self, p: &Path) -> Result<(), ErrorDto> {
        let md = std::fs::metadata(os_path(p)).map_err(|e| match e.kind() {
            std::io::ErrorKind::NotFound => not_found(),
            _ => ErrorDto::new("unavailable", e.to_string()),
        })?;
        if (self.probe)(p, &md) {
            Ok(())
        } else {
            Err(ErrorDto::new("cloud_placeholder", ""))
        }
    }

    /// Gjenoppretter `ids` (bare de som ligger i «Nylig slettet» telles). Indeksen først, så flyttes
    /// filene tilbake fra karantenen. Feil ved flytting lar filen ligge; `reconcile` retter det ved oppstart.
    pub fn restore(&self, ids: &[u64]) -> Result<RestoreDto, ErrorDto> {
        let _flow = self.flow_live()?;
        let trashed = self.trashed_ids();
        let wanted: Vec<AssetId> = ids
            .iter()
            .map(|i| AssetId(*i))
            .filter(|a| trashed.contains(a))
            .collect();
        if wanted.is_empty() {
            return Ok(RestoreDto::default());
        }
        self.fjell.restore_assets(&wanted).map_err(ErrorDto::from)?;
        self.unstage_active();
        Ok(RestoreDto {
            restored: wanted.len() as u64,
        })
    }

    pub fn restore_all(&self) -> Result<RestoreDto, ErrorDto> {
        let all: Vec<u64> = self.trashed_ids().into_iter().map(|a| a.0).collect();
        self.restore(&all)
    }

    /// Oppstart: filer i karantene som ikke lenger er `trashed` flyttes tilbake (avbrutt angre).
    pub fn reconcile(&self) {
        let _flow = self.flow();
        let ids = self.trashed_ids();
        let trashed = self.fjell.sources_for(&ids);
        for (id, r) in self.roots.reconcile(&trashed) {
            if let Reconciled::Restored(now) = r {
                self.relocate(id, &now);
            }
        }
    }

    /// Karantenen la filen på `now`. Er det et annet navn i samme mappe enn indeksen har
    /// (navnekollisjon), pekes indeksen dit; ellers ville neste trash av kortet flytte, og «Tøm»
    /// slette, den fremmede filen som ligger på den opprinnelige stien (F1). Mappen inkluderer
    /// rotprefikset, så indeksen kan aldri pekes over i en annen rot.
    fn relocate(&self, id: AssetId, now: &SourceId) {
        let old = self.fjell.sources_for(&[id]);
        let Some((_, old)) = old.first() else { return };
        if old != now && folder(old) == folder(now) {
            let _ = self.fjell.relocate_source(id, now);
        }
    }

    /// Etter vellykket trash: flytt filene til karantene (batch-id fra kjernen). Feil (annet volum,
    /// frakoblet) lar filen ligge: den er da «Merket, ikke flyttet» og originalen er urørt.
    /// Kalleren holder `flow` fra før indeksen ble endret.
    pub fn stage_trashed(&self, batch: BatchId, assets: &[AssetId]) {
        let items = self.fjell.sources_for(assets);
        let _ = self.roots.stage(batch, &items);
    }

    /// Angre siste batch (indeks), og bring karantenen i samsvar begge veier: angret trash flyttes
    /// tilbake, og angret gjenoppretting legges tilbake i karantenen (F4). Gir angret batch-id.
    pub fn undo(&self) -> Result<u64, ErrorDto> {
        let _flow = self.flow_live()?;
        let batch = self.fjell.undo().map_err(ErrorDto::from)?.batch;
        self.unstage_active();
        self.restage_trashed(batch);
        Ok(batch.0)
    }

    /// Alt som er `trashed` i indeksen, men ikke ligger i karantenen, flyttes dit (samme regler som
    /// `stage_trashed`: feil lar filen ligge urørt som «Merket, ikke flyttet»).
    fn restage_trashed(&self, batch: BatchId) {
        let staged = self.staged_own();
        let missing: Vec<AssetId> = self
            .trashed_ids()
            .into_iter()
            .filter(|a| !staged.contains(a))
            .collect();
        if !missing.is_empty() {
            self.stage_trashed(batch, &missing);
        }
    }

    /// Id-er som ligger i karantenen til den roten indeksens sti peker på. En manifestoppføring i
    /// en annen rots karantene (fremmed eller tuklet manifest) regnes ikke som «i karantene».
    fn staged_own(&self) -> Vec<AssetId> {
        let staged = self.roots.staged();
        let ids: Vec<AssetId> = staged.iter().map(|s| s.1).collect();
        let sources = self.fjell.sources_for(&ids);
        staged
            .into_iter()
            .filter(|(key, id, _)| {
                sources.iter().any(|(sid_id, sid)| {
                    sid_id == id && self.roots.route(sid).is_some_and(|(r, _)| r.key == *key)
                })
            })
            .map(|(_, id, _)| id)
            .collect()
    }

    /// Alt i karantene som ikke lenger er `trashed` flyttes tilbake. Kalleren holder `flow`.
    fn unstage_active(&self) {
        let ids = self.staged_own();
        let rows = self
            .fjell
            .with_store(|s| s.asset_rows(&ids))
            .unwrap_or_default();
        let back: Vec<AssetId> = ids
            .into_iter()
            .zip(rows)
            .filter(|(_, r)| r.as_ref().is_some_and(|r| r.state == State::Active))
            .map(|(i, _)| i)
            .collect();
        if !back.is_empty() {
            // Rutes på indeksens sti: filen hentes bare fra karantenen i sin egen rot.
            let items = self.fjell.sources_for(&back);
            for (id, r) in self.roots.unstage(&items) {
                if let Ok(now) = r {
                    self.relocate(id, &now);
                }
            }
        }
    }

    /// «Tøm» alt i «Nylig slettet». `confirmed` er konsekvenskoder brukeren har godtatt.
    pub fn empty(
        &self,
        bin: Arc<dyn RecycleBin>,
        confirmed: Vec<ConsequenceCode>,
        permanent: Option<PermanentAckDto>,
    ) -> Result<EmptyDto, ErrorDto> {
        let (mode, ack) = match permanent {
            None => (DeleteMode::Recycle, None),
            Some(p) => match PermanentAck::new(p.checked, p.count) {
                Some(a) => (DeleteMode::Permanent, Some((a, p.count))),
                None => return Err(ErrorDto::new("ack_required", "permanent")),
            },
        };
        // Låsen holdes fra billetten lages til siste fil er behandlet: ingen gjenoppretting eller
        // angre kan flytte en fil mellom bekreftelsen og slettingen (F2).
        let _flow = self.flow_live()?;
        let ids = self.trashed_ids();
        if ids.is_empty() {
            return Ok(EmptyDto::default());
        }
        let intent = self.fjell.begin_purge(ids).map_err(ErrorDto::from)?;
        if let Some((_, n)) = ack {
            if n != intent.ids.len() {
                return Err(ErrorDto::new("ack_mismatch", "permanent"));
            }
        }
        // Én sletter per rot, hver bundet til SIN rots karantene. Lages før bekreftelsen, så en
        // ugyldig permanent-bekreftelse stopper før billetten finnes.
        let mut deleters: Vec<(u32, FsDeleter)> = Vec::new();
        for r in self.roots() {
            let d = FsDeleter::new(
                &r.path,
                Some(r.quarantine.clone()),
                bin.clone(),
                mode,
                ack.map(|a| a.0),
            )?;
            deleters.push((r.key, d));
        }
        let before = self.fjell.trash_summary().freed_bytes;
        let ticket = self
            .fjell
            .confirm_purge_confirmed(&intent, confirmed)
            .map_err(ErrorDto::from)?;
        let sources: Vec<(AssetId, SourceId)> = self.fjell.sources_for(ticket.ids());
        // Hvert bilde går til sletteren for roten indeksens sti peker på, med stien relativt til
        // den roten. Uten kjent rot slettes ingenting (`Failed`: blir stående i «Nylig slettet»).
        let mut detailed: Vec<(AssetId, DeleteResult)> = Vec::new();
        let mut per: BTreeMap<u32, Vec<(AssetId, SourceId)>> = BTreeMap::new();
        for (id, sid) in &sources {
            match self.roots.route(sid) {
                Some((root, rel)) => per.entry(root.key).or_default().push((*id, rel)),
                None => detailed.push((*id, DeleteResult::Failed)),
            }
        }
        for (key, deleter) in &deleters {
            if let Some(items) = per.get(key) {
                detailed.extend(deleter.delete_detailed(&ticket, items)?);
            }
        }
        let mut out = EmptyDto::default();
        let outcomes: Vec<(AssetId, DeleteOutcome)> = detailed
            .into_iter()
            .map(|(id, r)| {
                let o = match r {
                    DeleteResult::Deleted => DeleteOutcome::Deleted,
                    DeleteResult::Cancelled => {
                        out.cancelled += 1;
                        DeleteOutcome::Cancelled
                    }
                    DeleteResult::Unsupported => {
                        out.unsupported += 1;
                        DeleteOutcome::Failed
                    }
                    DeleteResult::Unavailable => {
                        out.unavailable += 1;
                        DeleteOutcome::Failed
                    }
                    DeleteResult::Failed => DeleteOutcome::Failed,
                };
                (id, o)
            })
            .collect();
        let rep = self
            .fjell
            .finish_purge(ticket, outcomes)
            .map_err(ErrorDto::from)?;
        out.purged = rep.purged.len() as u64;
        out.kept_trashed = rep.kept_trashed.len() as u64;
        out.freed_bytes = self
            .fjell
            .trash_summary()
            .freed_bytes
            .saturating_sub(before);
        Ok(out)
    }
}
