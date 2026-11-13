//! Kommandologikk for desktop-skallet, uavhengig av Tauri (testbar uten webkit/WebView2).
//!
//! Avhenger av: `fjell-core::api` (`FjellApi`, `Fjell`, `MemApi`), `fjell-fs` (kilde, karantene,
//! Papirkurv), `dto`, `core` (ett åpent bibliotek), `config` (sist valgte mappe).
//! Grensesnitt: `Shell::mock()` (bare tester/dev), `Shell::with_data_dir(dir, bin)` (ekte),
//! `set_library`, `library`, `state`, `start_scan`, `cancel_scan`, `cards`, `decide`,
//! `decide_confirmed`, `set_keeper`, `undo`, `trash_summary`, `empty`, `language`, `set_language`,
//! og flere mapper: `list_library_roots`, `add_library_root`, `remove_library_root`
//! (docs/mvp/FLERE-MAPPER.md; `set_library` er fortsatt «erstatt med én rot»).
//! Tauri-kommandoene i `fjell-desktop` er tynne omslag rundt disse.
//!
//! Sletting (beslutning 17): trash flytter til karantene med angre; «Tøm» går via Papirkurv
//! (standard) og permanent bare med `PermanentAckDto`. Frontend kaller aldri filsystemet.

mod config;
mod core;
pub mod dto;
#[cfg(test)]
mod redteam_flere_mapper;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_core;
#[cfg(test)]
mod tests_flow;
#[cfg(all(test, unix))]
mod tests_migrering;
#[cfg(test)]
mod tests_review;
#[cfg(test)]
mod tests_roots;
#[cfg(test)]
mod tests_thumb;
mod thumb;

use crate::core::Core;
use dto::{
    parse_consequence, parse_kind, CardPageDto, DecisionDto, EmptyDto, ErrorDto, PermanentAckDto,
    RestoreDto, RootDto, SettingsDto, StateDto, ThumbDto, TrashSummaryDto, TrashedDto,
};
use fjell_core::api::{
    Card, CardCursor, Decision, FjellApi, HomeSummary, KindSummary, MemApi, ScanConfig, ScanMode,
    ScanPhase, ScanProgress, Settings, TrashSummary,
};
use fjell_core::model::{
    AssetId, AssetSource, CardId, Clock, ConsequenceCode, GroupId, ProtectFlags, ReasonCode,
    SuggestionKind,
};
use fjell_core::store::Store;
use fjell_fs::roots::PRIMARY_KEY;
use fjell_fs::thumbcache::{ThumbCache, DEFAULT_MAX_BYTES};
use fjell_fs::{FsSource, MultiSource, RecycleBin, RecycleError};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, RwLock};
use std::time::{SystemTime, UNIX_EPOCH};

struct SysClock;
impl Clock for SysClock {
    fn now(&self) -> i64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_secs() as i64)
    }
}

/// Papirkurv for plattformer uten implementasjon (Linux/macOS i denne fasen): gir alltid
/// `Unsupported`, aldri permanent sletting.
pub struct NoRecycleBin;
impl RecycleBin for NoRecycleBin {
    fn recycle(&self, _path: &Path) -> Result<(), RecycleError> {
        Err(RecycleError::Unsupported)
    }
}

/// Standard Papirkurv for plattformen: Windows Papirkurv på Windows, ellers `NoRecycleBin`.
pub fn default_recycle_bin() -> Arc<dyn RecycleBin> {
    #[cfg(windows)]
    {
        Arc::new(fjell_fs::windows::WindowsRecycleBin)
    }
    #[cfg(not(windows))]
    {
        Arc::new(NoRecycleBin)
    }
}

/// Kilden skallet bruker i ekte modus for ÉN rot (se `production_roots` for hele biblioteket).
fn production_source(data_dir: &Path, lib: &Path) -> FsSource {
    let (data_dir_owned, lib_owned) = (data_dir.to_path_buf(), lib.to_path_buf());
    FsSource::new(lib)
        .with_known_ids(Box::new(move || known_ids(&data_dir_owned, &lib_owned)))
        // Miniatyrer bufres bare i appens datamappe (aldri i biblioteket) og kan slettes uten tap.
        .with_thumb_cache(ThumbCache::new(
            thumb_cache_dir(data_dir),
            DEFAULT_MAX_BYTES,
        ))
}

/// T11: indeksens aktive bilder, så fjernede filer oppdages også etter omstart. Lesefeil gir tom
/// liste (ingen fjerninger), aldri gjetting. Før første skann finnes ikke indeksen ennå.
/// Slått opp ved kall: følger indeksen som faktisk er i bruk (også gammel fil etter mislykket flytting).
fn known_ids(data_dir: &Path, lib: &Path) -> Vec<fjell_core::model::SourceId> {
    let saved = config::last_library(data_dir);
    let db = config::locate_index(data_dir, lib, saved.as_deref());
    Store::active_source_ids_at(&db).unwrap_or_default()
}

/// Kilden for hele biblioteket: én `FsSource` per rot bak `MultiSource`. `roots[0]` er første rot
/// (nøkkel 1, id-er uten prefiks, som før flere mapper fantes) og eier indeksen.
fn production_roots(data_dir: &Path, roots: &[(u32, PathBuf)]) -> Result<MultiSource, ErrorDto> {
    let lib = roots
        .first()
        .map(|r| r.1.clone())
        .ok_or_else(|| ErrorDto::bad_request("ingen mappe"))?;
    let data_dir_owned = data_dir.to_path_buf();
    let parts = roots
        .iter()
        // Hver rots egne «kjente id-er» settes av `MultiSource::with_known_ids` under.
        .map(|(key, path)| (*key, production_source(data_dir, path)))
        .collect();
    Ok(MultiSource::new(parts)
        .map_err(|e| ErrorDto::new("root_overlap", e.to_string()))?
        .with_known_ids(Arc::new(move || known_ids(&data_dir_owned, &lib))))
}

/// Miniatyrbufferen: `<app-data>/cache/thumbs`.
pub(crate) fn thumb_cache_dir(data_dir: &Path) -> PathBuf {
    data_dir.join("cache").join("thumbs")
}

type Parts = (Arc<dyn FjellApi>, Option<Arc<Core>>);

enum Inner {
    /// Ekte modus uten valgt mappe ennå.
    NoLibrary,
    /// `MemApi` (kun tester og dev).
    Mock(Arc<dyn FjellApi>, &'static str),
    Core(Arc<Core>),
}

/// Skallets tilstand.
pub struct Shell {
    inner: RwLock<Inner>,
    data_dir: Option<PathBuf>,
    bin: Arc<dyn RecycleBin>,
    clock: Arc<dyn Clock>,
    /// Brukeren har satt skann på pause (avbrutt; `resume_scan` starter på nytt og fortsetter via `mod_token`).
    paused: AtomicBool,
    /// Modus for siste skann, så `resume_scan` fortsetter likt.
    last_full: AtomicBool,
}

impl Shell {
    pub fn new(api: Arc<dyn FjellApi>, backend: &'static str) -> Shell {
        Shell {
            inner: RwLock::new(Inner::Mock(api, backend)),
            data_dir: None,
            bin: Arc::new(NoRecycleBin),
            clock: Arc::new(SysClock),
            paused: AtomicBool::new(false),
            last_full: AtomicBool::new(false),
        }
    }

    /// Mot `MemApi` med noen få seedede kort. Bare for tester og dev.
    pub fn mock() -> Shell {
        Shell::new(
            Arc::new(MemApi::new(seed_cards(), Arc::new(SysClock))),
            "mock",
        )
    }

    /// Ekte skall. Gjenåpner sist valgte mappe hvis den finnes; ellers venter det på `set_library`.
    pub fn with_data_dir(data_dir: impl Into<PathBuf>, bin: Arc<dyn RecycleBin>) -> Shell {
        let data_dir = data_dir.into();
        let sh = Shell {
            inner: RwLock::new(Inner::NoLibrary),
            data_dir: Some(data_dir.clone()),
            bin,
            clock: Arc::new(SysClock),
            paused: AtomicBool::new(false),
            last_full: AtomicBool::new(false),
        };
        if let Some(lib) = config::last_library(&data_dir) {
            let mut roots = vec![(PRIMARY_KEY, lib.clone())];
            roots.extend(config::extra_roots(&data_dir, &lib));
            let _ = sh.open_production(&roots, &[]);
        }
        sh
    }

    fn data_dir(&self) -> Result<&PathBuf, ErrorDto> {
        self.data_dir
            .as_ref()
            .ok_or_else(|| ErrorDto::bad_request("ingen app-data-mappe"))
    }

    /// Åpner biblioteket med produksjonskilden over `roots`. `dropped` = nøklene til røtter som
    /// tas ut av biblioteket med dette kallet: har noen av dem bilder i «Nylig slettet», avvises
    /// byttet (`root_has_trash`) og ingenting endres.
    fn open_production(&self, roots: &[(u32, PathBuf)], dropped: &[u32]) -> Result<(), ErrorDto> {
        let source = production_roots(self.data_dir()?, roots)?;
        self.open_roots_probe(
            roots,
            Arc::new(source),
            fjell_fs::locality::default_probe(),
            dropped,
        )
    }

    /// Velger biblioteksmappe (fra mappevelger i Tauri-limet) og husker valget. Biblioteket får
    /// nøyaktig ÉN rot («erstatt med én rot»): ekstra mapper som var lagt til for denne mappen tas
    /// ut. Har noen av dem bilder i «Nylig slettet», avvises kallet med `root_has_trash` (gjenopprett
    /// eller tøm først), og ingenting endres.
    pub fn set_library(&self, path: &str) -> Result<StateDto, ErrorDto> {
        let lib = PathBuf::from(path);
        let dir = self.data_dir()?.clone();
        let dropped: Vec<u32> = config::extra_roots(&dir, &lib)
            .iter()
            .map(|r| r.0)
            .collect();
        self.open_production(&[(PRIMARY_KEY, lib.clone())], &dropped)?;
        // Valget huskes best effort; mislykket lagring stopper ikke bruken.
        let _ = config::save_library(&dir, &lib);
        let _ = config::retain_extra_roots(&dir, &lib, &[]);
        Ok(self.state())
    }

    /// Bibliotekets røtter med tall (antall/størrelse fra indeksen) og om mappen er tilgjengelig.
    pub fn list_library_roots(&self) -> Result<Vec<RootDto>, ErrorDto> {
        Ok(self.core()?.root_stats())
    }

    /// Legger en mappe til i biblioteket. Avvises hvis den ikke er en mappe (`not_a_folder`),
    /// overlapper en eksisterende rot (`root_overlap`), eller allerede har en Fjell-karantene med
    /// innhold fra et annet bibliotek (`root_has_quarantine`: id-ene der hører til en annen indeks).
    /// Ingen filer røres. Bildene kommer med ved neste skanning.
    pub fn add_library_root(&self, path: &str) -> Result<Vec<RootDto>, ErrorDto> {
        let dir = self.data_dir()?.clone();
        let core = self.core()?;
        let new = PathBuf::from(path);
        if !new.is_dir() {
            return Err(ErrorDto::new("not_a_folder", new.display().to_string()));
        }
        let mut roots: Vec<(u32, PathBuf)> = core
            .roots()
            .iter()
            .map(|r| (r.key, r.path.clone()))
            .collect();
        if let Some((_, other)) = roots
            .iter()
            .find(|(_, p)| fjell_fs::roots::overlaps(p, &new))
        {
            return Err(ErrorDto::new(
                "root_overlap",
                format!(
                    "{} overlapper {} (en mappe kan ikke ligge inni en annen)",
                    new.display(),
                    other.display()
                ),
            ));
        }
        if !fjell_fs::FsQuarantine::in_library(&new).staged().is_empty()
            || has_quarantine_files(&new)
        {
            return Err(ErrorDto::new(
                "root_has_quarantine",
                new.join(fjell_fs::QUARANTINE_DIR).display().to_string(),
            ));
        }
        // Nøkkelen reserveres i config FØR åpning (aldri gjenbrukt, selv om åpningen feiler).
        let key = config::add_extra_root(&dir, &core.library, &new)
            .map_err(|e| ErrorDto::new("internal", e.to_string()))?;
        roots.push((key, new));
        let keep: Vec<u32> = roots.iter().map(|r| r.0).collect();
        if let Err(e) = self.open_production(&roots, &[]) {
            let before: Vec<u32> = keep.iter().copied().filter(|k| *k != key).collect();
            let _ = config::retain_extra_roots(&dir, &core.library, &before);
            return Err(e);
        }
        self.list_library_roots()
    }

    /// Tar en mappe ut av biblioteket (ingen filer røres). Første rot kan ikke fjernes
    /// (`root_is_primary`: den eier indeksen; bytt bibliotek med `set_library`). Har roten bilder i
    /// «Nylig slettet», avvises kallet med `root_has_trash` (antall i `detail`): gjenopprett eller
    /// tøm dem først, ellers ville de blitt liggende i rotens karantene uten vei tilbake.
    /// Rotens rader forsvinner fra indeksen ved neste skanning.
    pub fn remove_library_root(&self, key: u32) -> Result<Vec<RootDto>, ErrorDto> {
        let dir = self.data_dir()?.clone();
        let core = self.core()?;
        if key == PRIMARY_KEY {
            return Err(ErrorDto::new("root_is_primary", ""));
        }
        if !core.roots().iter().any(|r| r.key == key) {
            return Err(ErrorDto::new("not_found", ""));
        }
        let roots: Vec<(u32, PathBuf)> = core
            .roots()
            .iter()
            .filter(|r| r.key != key)
            .map(|r| (r.key, r.path.clone()))
            .collect();
        let keep: Vec<u32> = roots.iter().map(|r| r.0).collect();
        self.open_production(&roots, &[key])?;
        config::retain_extra_roots(&dir, &core.library, &keep)
            .map_err(|e| ErrorDto::new("internal", e.to_string()))?;
        self.list_library_roots()
    }

    /// Som `set_library`, men med injisert kilde (tester bruker en kilde uten bildedekoding).
    #[cfg(test)]
    pub(crate) fn open_library(
        &self,
        lib: &Path,
        source: Arc<dyn AssetSource>,
    ) -> Result<(), ErrorDto> {
        self.open_library_probe(lib, source, fjell_fs::locality::default_probe())
    }

    #[cfg(test)]
    pub(crate) fn open_library_probe(
        &self,
        lib: &Path,
        source: Arc<dyn AssetSource>,
        probe: fjell_fs::locality::LocalProbe,
    ) -> Result<(), ErrorDto> {
        self.open_roots_probe(&[(PRIMARY_KEY, lib.to_path_buf())], source, probe, &[])
    }

    /// Åpner biblioteket `roots` (første = biblioteksmappen som eier indeksen) og bytter ut den
    /// åpne kjernen. Den gamle kjernens `flow`-lås holdes gjennom hele byttet, så pågående
    /// trash/angre/«Tøm» fullføres først og ingen ny starter imens: (1) ny kjerne åpnes (avstemming
    /// per rot), (2) `dropped` sjekkes, (3) den gamle pensjoneres (senere kall på den avvises),
    /// (4) byttet. Feiler (1) eller (2), er ingenting endret.
    pub(crate) fn open_roots_probe(
        &self,
        roots: &[(u32, PathBuf)],
        source: Arc<dyn AssetSource>,
        probe: fjell_fs::locality::LocalProbe,
        dropped: &[u32],
    ) -> Result<(), ErrorDto> {
        let dir = self.data_dir()?;
        let lib = roots
            .first()
            .map(|r| r.1.clone())
            .ok_or_else(|| ErrorDto::bad_request("ingen mappe"))?;
        // Første oppstart: app-data-mappen finnes ikke ennå, og SQLite oppretter ikke mapper.
        std::fs::create_dir_all(dir).map_err(|e| {
            ErrorDto::new(
                "internal",
                format!("kan ikke opprette {}: {e}", dir.display()),
            )
        })?;
        // Migrering av indeks fra pre-KR-3-navn, før noen Store åpnes på filen.
        let old = match &*self.read() {
            Inner::Core(c) => Some(c.clone()),
            _ => None,
        };
        let flow = old.as_ref().map(|c| c.flow());
        let in_use = old.as_ref().map(|c| c.db.clone());
        let saved = config::last_library(dir);
        let db =
            config::adopt_legacy_index(dir, &lib, saved.as_deref(), in_use.as_deref(), &|a, b| {
                std::fs::rename(a, b)
            });
        let core = Core::open_roots(roots, &db, source, self.clock.clone(), probe)?;
        let n = core.trashed_in(dropped);
        if n > 0 {
            return Err(ErrorDto::new("root_has_trash", n.to_string()));
        }
        if let (Some(old), Some(flow)) = (&old, &flow) {
            old.fjell.cancel();
            let _ = old.fjell.wait_scan();
            old.retire(flow);
        }
        drop(flow);
        let mut g = self.inner.write().unwrap_or_else(|e| e.into_inner());
        *g = Inner::Core(Arc::new(core));
        Ok(())
    }

    /// Valgt biblioteksmappe (ekte modus).
    pub fn library(&self) -> Option<String> {
        match &*self.read() {
            Inner::Core(c) => Some(c.library.to_string_lossy().into_owned()),
            _ => None,
        }
    }

    fn read(&self) -> std::sync::RwLockReadGuard<'_, Inner> {
        self.inner.read().unwrap_or_else(|e| e.into_inner())
    }

    /// API og (i ekte modus) kjerne. `no_library` hvis ingen mappe er valgt.
    fn parts(&self) -> Result<Parts, ErrorDto> {
        match &*self.read() {
            Inner::NoLibrary => Err(ErrorDto::new("no_library", "")),
            Inner::Mock(a, _) => Ok((a.clone(), None)),
            Inner::Core(c) => Ok((c.fjell.clone(), Some(c.clone()))),
        }
    }

    pub fn state(&self) -> StateDto {
        let (api, backend) = match &*self.read() {
            Inner::NoLibrary => {
                return StateDto::from_core("core", &idle_summary(), &Default::default())
            }
            Inner::Mock(a, b) => (a.clone(), *b),
            Inner::Core(c) => (c.fjell.clone() as Arc<dyn FjellApi>, "core"),
        };
        let mut st = StateDto::from_core(backend, &api.home_summary(), &api.settings());
        st.scan_paused = self.paused.load(Ordering::SeqCst);
        if let Inner::Core(c) = &*self.read() {
            st.cloud_sync = c.cloud_all();
            st.roots = c.root_dtos();
        }
        st
    }

    /// Starter skanning og returnerer straks; fremdrift leses via `state()` (`scan_*`).
    /// Eksakte grupper er klare først (`exact_ready`), full analyse fortsetter etterpå.
    pub fn start_scan(&self, full: bool) -> Result<(), ErrorDto> {
        let mode = if full {
            ScanMode::Full
        } else {
            ScanMode::Quick
        };
        self.paused.store(false, Ordering::SeqCst);
        self.last_full.store(full, Ordering::SeqCst);
        self.parts()?
            .0
            .start_scan(ScanConfig {
                mode,
                priority: 1,
                deadline: None,
            })
            .map_err(ErrorDto::from)
    }

    /// Avbryter skann. Det som er funnet beholdes; en ny skanning fortsetter via `mod_token`.
    pub fn cancel_scan(&self) -> Result<(), ErrorDto> {
        self.paused.store(false, Ordering::SeqCst);
        self.parts()?.0.cancel();
        Ok(())
    }

    /// Pause = avbryt nå og husk det. Ingen filer røres; bildene som er analysert er lagret i indeksen.
    pub fn pause_scan(&self) -> Result<(), ErrorDto> {
        self.parts()?.0.cancel();
        self.paused.store(true, Ordering::SeqCst);
        Ok(())
    }

    /// Fortsett etter pause: ny skanning i samme modus. Ferdig analyserte bilder hoppes over.
    pub fn resume_scan(&self) -> Result<(), ErrorDto> {
        if !self.paused.load(Ordering::SeqCst) {
            return Ok(());
        }
        self.start_scan(self.last_full.load(Ordering::SeqCst))
    }

    pub fn cards(
        &self,
        kind: &str,
        limit: usize,
        cursor: Option<u64>,
    ) -> Result<CardPageDto, ErrorDto> {
        let kind = parse_kind(kind).ok_or_else(|| ErrorDto::bad_request("unknown kind"))?;
        let limit = limit.clamp(1, 200);
        let page = self.parts()?.0.cards(kind, limit, cursor.map(CardCursor))?;
        Ok(CardPageDto::from(&page))
    }

    /// Returnerer batch-id (ett angre). Mangler bekreftelse svarer `ack_required` (koder i `detail`).
    pub fn decide(&self, ids: &[u64], decision: DecisionDto) -> Result<u64, ErrorDto> {
        self.decide_confirmed(ids, decision, &[])
    }

    /// Som `decide`, med konsekvenskoder brukeren har bekreftet (fra en tidligere `ack_required`).
    pub fn decide_confirmed(
        &self,
        ids: &[u64],
        decision: DecisionDto,
        confirmed: &[String],
    ) -> Result<u64, ErrorDto> {
        let (api, core) = self.parts()?;
        // Skysynket bibliotek (alle røtter): papirkurv nektes før noe endres (Behold er trygt og tillatt).
        if let (DecisionDto::Trash, Some(c)) = (decision, &core) {
            if let Some(p) = c.cloud_all() {
                return Err(ErrorDto::new("cloud_synced_readonly", p));
            }
        }
        let codes = parse_codes(confirmed)?;
        let d = match decision {
            DecisionDto::Trash => Decision::Trash,
            DecisionDto::Keep => Decision::Keep,
        };
        // Asset-id-er hentes før beslutningen (kortene er ikke lenger ventende etterpå).
        let assets: Vec<AssetId> = match (&core, d) {
            (Some(c), Decision::Trash) => c
                .fjell
                .with_store(|s| s.suggestion_rows(ids))
                .map_err(|e| ErrorDto::new("internal", e.to_string()))?
                .into_iter()
                .flatten()
                .map(|r| r.asset)
                .collect(),
            _ => Vec::new(),
        };
        // Skysperre per rot: ligger ett av bildene i en skysynket rot, nektes hele kallet før noe
        // endres. Bilder i andre røtter kan sendes i et eget kall.
        if let Some(p) = core.as_ref().and_then(|c| c.cloud_block(&assets)) {
            return Err(ErrorDto::new("cloud_synced_readonly", p));
        }
        // Indeks + flytting til karantene er én overgang (ingen «Tøm»/gjenoppretting imellom).
        let _flow = core.as_ref().map(|c| c.flow_live()).transpose()?;
        let batch =
            api.decide_many_confirmed(ids.iter().map(|i| (CardId(*i), d)).collect(), codes)?;
        if let Some(c) = &core {
            if !assets.is_empty() {
                c.stage_trashed(batch, &assets);
            }
        }
        Ok(batch.0)
    }

    pub fn set_keeper(&self, group: u64, keeper: u64) -> Result<(), ErrorDto> {
        Ok(self
            .parts()?
            .0
            .set_keeper(GroupId(group), AssetId(keeper))?)
    }

    pub fn undo(&self) -> Result<u64, ErrorDto> {
        let (api, core) = self.parts()?;
        match core {
            Some(c) => c.undo(),
            None => Ok(api.undo()?.batch.0),
        }
    }

    pub fn trash_summary(&self) -> Result<TrashSummaryDto, ErrorDto> {
        let (api, _) = self.parts()?;
        Ok(TrashSummaryDto::from_core(
            &api.trash_summary(),
            &api.settings(),
        ))
    }

    /// Alle bilder i «Nylig slettet» (ekte modus). Mocken har ingen filer.
    pub fn trashed(&self) -> Result<Vec<TrashedDto>, ErrorDto> {
        let core = self.core()?;
        Ok(core.trashed(self.clock.now()))
    }

    /// Gjenoppretter valgte bilder. Umiddelbart og uten bekreftelse (PLAN-UX-I18N 3.6).
    pub fn restore(&self, ids: &[u64]) -> Result<RestoreDto, ErrorDto> {
        self.core()?.restore(ids)
    }

    pub fn restore_all(&self) -> Result<RestoreDto, ErrorDto> {
        self.core()?.restore_all()
    }

    /// Miniatyr (JPEG, base64) for `AssetId` (`CardDto.asset`/`TrashedDto.asset`). Kjører uten lås
    /// på skallet under dekoding. Mock-modus gir en deterministisk liten JPEG.
    pub fn thumbnail(&self, id: u64) -> Result<ThumbDto, ErrorDto> {
        match self.parts()? {
            (_, Some(core)) => thumb::encode(&core.thumbnail(AssetId(id))?),
            _ => thumb::mock(id),
        }
    }

    fn core(&self) -> Result<Arc<Core>, ErrorDto> {
        self.parts()?
            .1
            .ok_or_else(|| ErrorDto::new("unsupported", "mock"))
    }

    /// Innstillinger: papirkurvfrist, karantenemappe og standard for «Tøm».
    pub fn settings(&self) -> Result<SettingsDto, ErrorDto> {
        let (api, core) = match &*self.read() {
            Inner::NoLibrary => (None, None),
            Inner::Mock(a, _) => (Some(a.clone()), None),
            Inner::Core(c) => (Some(c.fjell.clone() as Arc<dyn FjellApi>), Some(c.clone())),
        };
        Ok(SettingsDto {
            trash_days: api.map_or(Settings::default().trash_days, |a| a.settings().trash_days),
            quarantine_dir: core.map(|c| c.quarantine_dir().to_string_lossy().into_owned()),
            empty_default: "recycle",
        })
    }

    /// Endrer papirkurvfristen (1 til 365 dager). Gjelder bare nye elementer (kjernen).
    pub fn set_trash_days(&self, days: u32) -> Result<SettingsDto, ErrorDto> {
        if !(1..=365).contains(&days) {
            return Err(ErrorDto::bad_request("trash_days"));
        }
        self.parts()?.0.set_settings(Settings { trash_days: days });
        self.settings()
    }

    /// Lagret språkvalg (`"system"`, `"nb"` eller `"en"`). `None` = aldri valgt (eller mock uten
    /// app-data): frontend bruker da sitt eget valg og kan migrere det med `set_language`.
    pub fn language(&self) -> Option<String> {
        self.data_dir.as_deref().and_then(config::language)
    }

    /// Lagrer språkvalget i skallets innstillinger. Ukjent verdi (også `pseudo`) gir `bad_request`;
    /// uten app-data (mock) `unsupported`. Returnerer lagret verdi.
    pub fn set_language(&self, language: &str) -> Result<String, ErrorDto> {
        if !config::LANGUAGES.contains(&language) {
            return Err(ErrorDto::bad_request("language"));
        }
        let dir = self
            .data_dir
            .as_ref()
            .ok_or_else(|| ErrorDto::new("unsupported", "mock"))?;
        config::set_language(dir, language)
            .map_err(|e| ErrorDto::new("internal", e.to_string()))?;
        Ok(language.to_string())
    }

    /// Første gang appen kjøres (ingen mappe valgt, onboarding ikke fullført). Alltid `false` uten app-data.
    pub fn first_run(&self) -> bool {
        match &self.data_dir {
            Some(d) => config::first_run(d),
            None => false,
        }
    }

    /// Onboarding er gjennomført (første skann startet). Vises ikke på nytt, også om appen lukkes midt i skann.
    pub fn complete_onboarding(&self) -> Result<(), ErrorDto> {
        if let Some(d) = &self.data_dir {
            config::set_onboarded(d).map_err(|e| ErrorDto::new("internal", e.to_string()))?;
        }
        Ok(())
    }

    /// «Sjekk automatisk ved oppstart». Standard `false`; alltid `false` uten app-data (mock).
    pub fn auto_update(&self) -> bool {
        match &self.data_dir {
            Some(d) => config::auto_update(d),
            None => false,
        }
    }

    /// Lagrer valget i `shell.json` og returnerer den lagrede verdien. Uten app-data (mock) lagres ingenting.
    pub fn set_auto_update(&self, on: bool) -> Result<bool, ErrorDto> {
        if let Some(d) = &self.data_dir {
            config::set_auto_update(d, on).map_err(|e| ErrorDto::new("internal", e.to_string()))?;
        }
        Ok(self.auto_update())
    }

    /// «Tøm»: Papirkurv som standard; permanent bare med `permanent` (avkrysning + riktig antall).
    /// Bare i ekte modus; mocken har ingen filer å tømme.
    pub fn empty(
        &self,
        confirmed: &[String],
        permanent: Option<PermanentAckDto>,
    ) -> Result<EmptyDto, ErrorDto> {
        let (_, core) = self.parts()?;
        let core = core.ok_or_else(|| ErrorDto::new("unsupported", "mock"))?;
        core.empty(self.bin.clone(), parse_codes(confirmed)?, permanent)
    }
}

fn parse_codes(codes: &[String]) -> Result<Vec<ConsequenceCode>, ErrorDto> {
    codes
        .iter()
        .map(|c| {
            parse_consequence(c).ok_or_else(|| ErrorDto::bad_request(format!("unknown code {c}")))
        })
        .collect()
}

fn idle_summary() -> HomeSummary {
    HomeSummary {
        per_kind: SuggestionKind::ALL
            .iter()
            .map(|k| KindSummary {
                kind: *k,
                count: 0,
                bytes: 0,
            })
            .collect(),
        reclaimable_bytes: 0,
        trash: TrashSummary::default(),
        scan: ScanProgress {
            phase: ScanPhase::Idle,
            done: 0,
            total: 0,
            found_exact: 0,
        },
    }
}

fn seed_cards() -> Vec<Card> {
    let mk = |n: u64, kind, reason, bytes| Card {
        id: CardId(n),
        kind,
        asset: AssetId(n),
        group: (kind == SuggestionKind::ExactDup).then_some(GroupId(1)),
        keeper: (kind == SuggestionKind::ExactDup).then_some(AssetId(100)),
        reason,
        confidence: 1.0,
        bytes,
        protect_flags: ProtectFlags::default(),
    };
    vec![
        mk(
            1,
            SuggestionKind::ExactDup,
            ReasonCode::ExactCopy,
            2_400_000,
        ),
        mk(
            2,
            SuggestionKind::ExactDup,
            ReasonCode::ExactCopy,
            3_100_000,
        ),
        mk(3, SuggestionKind::Blurry, ReasonCode::Blurry, 1_800_000),
        mk(
            4,
            SuggestionKind::Screenshot,
            ReasonCode::Screenshot,
            600_000,
        ),
    ]
}

/// Ligger det filer i mappens `.fjell-karantene` (også uten lesbart manifest)? Bare katalogoppslag.
fn has_quarantine_files(root: &Path) -> bool {
    let q = root.join(fjell_fs::QUARANTINE_DIR);
    if std::fs::symlink_metadata(&q).is_ok_and(|m| m.file_type().is_symlink()) {
        return true;
    }
    std::fs::read_dir(&q).is_ok_and(|rd| {
        rd.filter_map(|b| b.ok()).any(|b| {
            let p = b.path();
            !p.is_dir()
                || std::fs::read_dir(&p).is_ok_and(|files| {
                    files.filter_map(|f| f.ok()).any(|f| {
                        let n = f.file_name();
                        n != "manifest.json" && n != "README.txt"
                    })
                })
        })
    })
}
