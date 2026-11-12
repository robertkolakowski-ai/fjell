//! Kommandologikk for desktop-skallet, uavhengig av Tauri (testbar uten webkit/WebView2).
//!
//! Avhenger av: `fjell-core::api` (`FjellApi`, `Fjell`, `MemApi`), `fjell-fs` (kilde, karantene,
//! Papirkurv), `dto`, `core` (ett åpent bibliotek), `config` (sist valgte mappe).
//! Grensesnitt: `Shell::mock()` (bare tester/dev), `Shell::with_data_dir(dir, bin)` (ekte),
//! `set_library`, `library`, `state`, `start_scan`, `cancel_scan`, `cards`, `decide`,
//! `decide_confirmed`, `set_keeper`, `undo`, `trash_summary`, `empty`, `language`, `set_language`.
//! Tauri-kommandoene i `fjell-desktop` er tynne omslag rundt disse.
//!
//! Sletting (beslutning 17): trash flytter til karantene med angre; «Tøm» går via Papirkurv
//! (standard) og permanent bare med `PermanentAckDto`. Frontend kaller aldri filsystemet.

mod config;
mod core;
pub mod dto;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_core;

use crate::core::Core;
use dto::{
    parse_consequence, parse_kind, CardPageDto, DecisionDto, EmptyDto, ErrorDto, PermanentAckDto,
    RestoreDto, SettingsDto, StateDto, TrashSummaryDto, TrashedDto,
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
use fjell_fs::thumbcache::{ThumbCache, DEFAULT_MAX_BYTES};
use fjell_fs::{FsSource, RecycleBin, RecycleError};
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

/// Kilden skallet bruker i ekte modus (se `Shell::production_source`).
fn production_source(data_dir: &Path, lib: &Path) -> FsSource {
    let db = config::db_path(data_dir, lib);
    FsSource::new(lib)
        // T11: indeksens aktive bilder, så fjernede filer oppdages også etter omstart. Lesefeil gir
        // tom liste (ingen fjerninger), aldri gjetting. Før første skann finnes ikke indeksen ennå.
        .with_known_ids(Box::new(move || {
            Store::active_source_ids_at(&db).unwrap_or_default()
        }))
        // Miniatyrer bufres bare i appens datamappe (aldri i biblioteket) og kan slettes uten tap.
        .with_thumb_cache(ThumbCache::new(
            thumb_cache_dir(data_dir),
            DEFAULT_MAX_BYTES,
        ))
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
            let _ = sh.open_library(&lib, Arc::new(production_source(&data_dir, &lib)));
        }
        sh
    }

    /// Velger biblioteksmappe (fra mappevelger i Tauri-limet) og husker valget.
    pub fn set_library(&self, path: &str) -> Result<StateDto, ErrorDto> {
        let lib = PathBuf::from(path);
        let dir = self
            .data_dir
            .as_ref()
            .ok_or_else(|| ErrorDto::bad_request("ingen app-data-mappe"))?;
        self.open_library(&lib, Arc::new(production_source(dir, &lib)))?;
        if let Some(d) = &self.data_dir {
            // Valget huskes best effort; mislykket lagring stopper ikke bruken.
            let _ = config::save_library(d, &lib);
        }
        Ok(self.state())
    }

    /// Som `set_library`, men med injisert kilde (tester bruker en kilde uten bildedekoding).
    pub(crate) fn open_library(
        &self,
        lib: &Path,
        source: Arc<dyn AssetSource>,
    ) -> Result<(), ErrorDto> {
        let dir = self
            .data_dir
            .as_ref()
            .ok_or_else(|| ErrorDto::bad_request("ingen app-data-mappe"))?;
        let lib = lib.to_path_buf();
        // Første oppstart: app-data-mappen finnes ikke ennå, og SQLite oppretter ikke mapper.
        std::fs::create_dir_all(dir).map_err(|e| {
            ErrorDto::new(
                "internal",
                format!("kan ikke opprette {}: {e}", dir.display()),
            )
        })?;
        let core = Core::open(
            &lib,
            &config::db_path(dir, &lib),
            source,
            self.clock.clone(),
        )?;
        let mut g = self.inner.write().unwrap_or_else(|e| e.into_inner());
        if let Inner::Core(old) = &*g {
            old.fjell.cancel();
            let _ = old.fjell.wait_scan();
        }
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
        st.cloud_sync = self.cloud_sync();
        st
    }

    /// Skyleverandør for gjeldende bibliotek (kun ekte modus), etter kjente mappenavn.
    fn cloud_sync(&self) -> Option<&'static str> {
        match &*self.read() {
            Inner::Core(c) => {
                fjell_fs::cloud_provider(&c.library.to_string_lossy()).map(|p| p.code())
            }
            _ => None,
        }
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
        // Skysynket bibliotek: papirkurv nektes før noe endres (Behold er trygt og tillatt).
        if matches!(decision, DecisionDto::Trash) {
            if let Some(p) = self.cloud_sync() {
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
        let batch =
            api.decide_many_confirmed(ids.iter().map(|i| (CardId(*i), d)).collect(), codes)?;
        if let Some(c) = core {
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
        let batch = api.undo()?.batch.0;
        if let Some(c) = core {
            c.restore_undone();
        }
        Ok(batch)
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
