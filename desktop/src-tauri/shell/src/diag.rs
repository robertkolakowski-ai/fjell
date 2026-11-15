//! Diagnose for skallet (`impl Shell`): lokal, rullerende feillogg, panikkhook og «Eksporter diagnose».
//! Del av `fjell-shell`. Spesifikasjon og personvernregler: `docs/v1/DIAGNOSE.md`, `docs/PERSONVERN.md`.
//!
//! Avhenger av: `fjell-core::api` (`FjellApi`), `fjell-core::model` (`DiagReport`, feilkoder), `dto_robust`, `dto`.
//! Kjernen har ingen loggkode; alt ligger her i skallet.
//!
//! Redaksjon ved konstruksjon, ikke ved rensing: loggposter er bare `(tidsstempel, kategori, kode, tall, versjon)`.
//! `kode` må være en kjent feilkode (tillatelsesliste); alt annet, også panikkmeldinger og feilmeldinger som kan
//! inneholde stier, blir `other`/`panic`. Rapporten går dessuten gjennom `redact_report`, som bare slipper
//! gjennom linjer på formen `nøkkel=verdi` med et smalt tegnsett (ingen skråstreker, kolon eller mellomrom).
//! Mappenavn (aldri stier) kommer bare med når brukeren har samtykket (`FolderNamesConsent`), og er av som standard.
//!
//! Ingenting sendes noe sted: ingen opplasting, ingen e-postklient. Eksporten er en tekstfil i appens egen
//! diagnosemappe (`<app-data>/diagnose/`), og stien vises så brukeren selv kan sende den.

// Modulen er privat i `lib.rs` (delt fil), og logg, hook og valg kalles først fra Tauri-limet i `fjell-desktop`
// (se integrasjonsnotatene i docs/v1/DIAGNOSE.md). Til da brukes de bare av testene.
#![allow(dead_code)]

use crate::dto::ErrorDto;
use crate::dto_robust::DiagDto;
use crate::Shell;
use fjell_core::api::FjellApi;
use fjell_core::model::{DiagMetric, DiagReport, ROBUST_ERROR_CODES};
use serde::Serialize;
use std::collections::BTreeMap;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

/// Størrelsestak per loggfil og antall filer (1 MB × 3).
pub const LOG_MAX_BYTES: u64 = 1_000_000;
pub const LOG_FILES: usize = 3;
/// Antall siste loggposter i rapporten.
pub const REPORT_LOG_ENTRIES: usize = 50;
/// Rapportformat for skallets del (kjernens `DiagReport::FORMAT` står i `format=`).
pub const SHELL_FORMAT: u32 = 1;

/// Feilkoder skallet selv bruker. Sammen med `ROBUST_ERROR_CODES` er dette hele tillatelseslisten for `code`.
const SHELL_CODES: [&str; 14] = [
    "no_library",
    "core_error",
    "internal",
    "bad_request",
    "unsupported",
    "not_found",
    "root_overlap",
    "root_has_trash",
    "root_has_quarantine",
    "root_is_primary",
    "not_a_folder",
    "io_error",
    "panic",
    "other",
];

fn allowed_code(raw: &str) -> Option<&'static str> {
    ROBUST_ERROR_CODES
        .iter()
        .chain(SHELL_CODES.iter())
        .find(|c| **c == raw)
        .copied()
}

/// Kjent kode eller `other`. Aldri fritekst: en melding som kan inneholde sti, filnavn eller navn gir `other`.
pub fn known_code(raw: &str) -> &'static str {
    allowed_code(raw).unwrap_or("other")
}

/// Kategorier for loggposter.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Category {
    Panic,
    Core,
    Scan,
    Index,
    Store,
    Shell,
}

impl Category {
    pub const ALL: [Category; 6] = [
        Self::Panic,
        Self::Core,
        Self::Scan,
        Self::Index,
        Self::Store,
        Self::Shell,
    ];
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Panic => "panic",
            Self::Core => "core",
            Self::Scan => "scan",
            Self::Index => "index",
            Self::Store => "store",
            Self::Shell => "shell",
        }
    }
    fn parse(s: &str) -> Option<Category> {
        Self::ALL.into_iter().find(|c| c.as_str() == s)
    }
}

/// Én loggpost. Feltene er koder og tall; det finnes ingen plass til tekst.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LogRecord {
    /// Unix-sekunder.
    pub ts: i64,
    pub category: Category,
    /// Alltid en kjent kode (`known_code`).
    pub code: &'static str,
    /// Et tall (f.eks. antall, linjenummer for panikk); 0 hvis ikke aktuelt.
    pub n: u64,
    /// Kjernens versjon.
    pub version: String,
}

impl LogRecord {
    /// Lager en post. `raw_code` kan være hva som helst (også en feilmelding med sti): det blir bare en kjent kode.
    pub fn new(ts: i64, category: Category, raw_code: &str, n: u64) -> LogRecord {
        LogRecord {
            ts,
            category,
            code: known_code(raw_code),
            n,
            version: fjell_core::VERSION.to_string(),
        }
    }

    fn line(&self) -> String {
        format!(
            "ts={} cat={} code={} n={} v={}",
            self.ts,
            self.category.as_str(),
            self.code,
            self.n,
            self.version
        )
    }

    /// Streng parsing: alt som ikke er nøyaktig på formen over (f.eks. en manipulert loggfil) forkastes.
    fn parse(line: &str) -> Option<LogRecord> {
        let mut it = line.split(' ');
        let ts = it.next()?.strip_prefix("ts=")?.parse::<i64>().ok()?;
        let category = Category::parse(it.next()?.strip_prefix("cat=")?)?;
        let code = allowed_code(it.next()?.strip_prefix("code=")?)?;
        let n = it.next()?.strip_prefix("n=")?.parse::<u64>().ok()?;
        let version = it.next()?.strip_prefix("v=")?;
        if it.next().is_some() || version.is_empty() || !valid_value(version) {
            return None;
        }
        Some(LogRecord {
            ts,
            category,
            code,
            n,
            version: version.to_string(),
        })
    }

    /// Posten slik den står i rapporten: `ts|kategori|kode|n|versjon`.
    fn report_value(&self) -> String {
        format!(
            "{}|{}|{}|{}|{}",
            self.ts,
            self.category.as_str(),
            self.code,
            self.n,
            self.version
        )
    }
}

static LOG_LOCK: Mutex<()> = Mutex::new(());

/// Rullerende loggfil: `fjell.log`, `fjell.log.1`, … Når gjeldende fil ville blitt større enn taket,
/// flyttes filene ett steg og den eldste forsvinner (bare loggen, aldri bilder).
pub struct RollingLog {
    dir: PathBuf,
    max_bytes: u64,
    files: usize,
}

impl RollingLog {
    pub fn new(dir: impl Into<PathBuf>, max_bytes: u64, files: usize) -> RollingLog {
        RollingLog {
            dir: dir.into(),
            max_bytes,
            files: files.max(1),
        }
    }

    /// Standardloggen i appens datamappe: `<app-data>/logs`.
    pub fn in_data_dir(data_dir: &Path) -> RollingLog {
        RollingLog::new(data_dir.join("logs"), LOG_MAX_BYTES, LOG_FILES)
    }

    fn path(&self, i: usize) -> PathBuf {
        if i == 0 {
            self.dir.join("fjell.log")
        } else {
            self.dir.join(format!("fjell.log.{i}"))
        }
    }

    pub fn append(&self, rec: &LogRecord) -> io::Result<()> {
        let _g = LOG_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        self.append_locked(rec)
    }

    fn append_locked(&self, rec: &LogRecord) -> io::Result<()> {
        fs::create_dir_all(&self.dir)?;
        let mut line = rec.line();
        line.push('\n');
        let cur = fs::metadata(self.path(0)).map_or(0, |m| m.len());
        if cur > 0 && cur + line.len() as u64 > self.max_bytes {
            let _ = fs::remove_file(self.path(self.files - 1));
            for i in (0..self.files - 1).rev() {
                if self.path(i).exists() {
                    fs::rename(self.path(i), self.path(i + 1))?;
                }
            }
        }
        let mut f = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(self.path(0))?;
        f.write_all(line.as_bytes())
    }

    /// De siste `n` gyldige postene, eldst først. Ugyldige linjer hoppes over.
    pub fn tail(&self, n: usize) -> Vec<LogRecord> {
        let _g = LOG_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut all = Vec::new();
        for i in (0..self.files).rev() {
            if let Ok(s) = fs::read_to_string(self.path(i)) {
                all.extend(s.lines().filter_map(LogRecord::parse));
            }
        }
        let skip = all.len().saturating_sub(n);
        all.split_off(skip)
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Miljø og ytelsestellere (prosessglobale, bare tall)

#[derive(Clone, Debug, Default)]
struct DiagEnv {
    os_version: Option<String>,
    scale_percent: Option<u32>,
}

static ENV: Mutex<DiagEnv> = Mutex::new(DiagEnv {
    os_version: None,
    scale_percent: None,
});

/// Settes av Tauri-limet ved oppstart: OS-versjon (f.eks. `10.0.22631`) og skjermskalering i prosent.
/// `os_version` slipper bare gjennom hvis den har tegnsettet til en versjon (siffer, punktum, bokstaver, `-`, `+`).
pub fn set_env(os_version: &str, scale_percent: u32) {
    let mut e = ENV.lock().unwrap_or_else(|e| e.into_inner());
    e.os_version =
        (!os_version.is_empty() && valid_value(os_version)).then(|| os_version.to_string());
    e.scale_percent = (scale_percent > 0 && scale_percent <= 1000).then_some(scale_percent);
}

const PERF_CODES: [&str; 6] = ["startup", "scan", "card_page", "decide", "thumb", "preview"];

#[derive(Clone, Copy, Default)]
struct Perf {
    count: u64,
    total_ms: u64,
    max_ms: u64,
}

static PERF: Mutex<BTreeMap<&'static str, Perf>> = Mutex::new(BTreeMap::new());

/// Registrerer én måling (millisekunder) for en kjent handling. Ukjente koder ignoreres.
pub fn perf_observe(code: &str, ms: u64) {
    let Some(code) = PERF_CODES.iter().find(|c| **c == code).copied() else {
        return;
    };
    let mut m = PERF.lock().unwrap_or_else(|e| e.into_inner());
    let p = m.entry(code).or_default();
    p.count += 1;
    p.total_ms = p.total_ms.saturating_add(ms);
    p.max_ms = p.max_ms.max(ms);
}

/// Nullstiller tellerne (tester).
pub fn perf_reset() {
    PERF.lock().unwrap_or_else(|e| e.into_inner()).clear();
}

fn linux_os_version() -> Option<String> {
    let s = fs::read_to_string("/proc/sys/kernel/osrelease").ok()?;
    let s = s.trim();
    (!s.is_empty() && valid_value(s)).then(|| s.to_string())
}

// ---------------------------------------------------------------------------------------------------------------
// Panikkhook

/// Installerer panikkhooken: skriver en redigert post (`cat=panic code=panic n=<linjenummer>`) til loggen og kaller
/// den forrige hooken. Panikkmeldingen og filstien i `Location` brukes aldri. Kalles én gang ved oppstart.
pub fn install_panic_hook(data_dir: &Path) {
    let log = RollingLog::in_data_dir(data_dir);
    let prev = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let line = info.location().map_or(0, |l| u64::from(l.line()));
        let rec = LogRecord::new(now(), Category::Panic, "panic", line);
        // `try_lock`: en panikk midt i en loggskriving på samme tråd skal ikke henge.
        if let Ok(_g) = LOG_LOCK.try_lock() {
            let _ = log.append_locked(&rec);
        }
        prev(info);
    }));
}

fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as i64)
}

// ---------------------------------------------------------------------------------------------------------------
// Rapport

/// Samtykke til å ta med mappenavn (siste stidel, aldri stier). Kan bare lages med avkrysning.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FolderNamesConsent(());

impl FolderNamesConsent {
    pub fn new(checked: bool) -> Option<FolderNamesConsent> {
        checked.then_some(FolderNamesConsent(()))
    }
}

/// Valg for rapporten. Standard: ingen mappenavn.
#[derive(Clone, Copy, Debug, Default)]
pub struct DiagOptions {
    pub folder_names: Option<FolderNamesConsent>,
}

fn valid_key(k: &str) -> bool {
    !k.is_empty()
        && k.len() <= 48
        && k.bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_' || b == b'.')
}

/// Tillatte tegn i en verdi: siffer, ASCII-bokstaver, `_ . , | + -`. Ingen skråstrek, kolon, mellomrom eller
/// tegn utenfor ASCII, så en sti kan ikke stå her.
fn valid_value(v: &str) -> bool {
    v.len() <= 96
        && v.bytes().all(|b| {
            b.is_ascii_alphanumeric() || matches!(b, b'_' | b'.' | b',' | b'|' | b'+' | b'-')
        })
}

/// Mappenavn til rapporten: bare siste stidel, uten skilletegn, kontrolltegn og kolon, høyst 40 tegn.
/// Bare brukt etter samtykke.
fn folder_label(path: &Path) -> String {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    name.chars()
        .filter(|c| !c.is_control() && !matches!(c, '/' | '\\' | ':' | '='))
        .take(40)
        .collect()
}

/// Siste sikkerhetsnett over hele rapporten: linjer som ikke er `nøkkel=verdi` med smalt tegnsett forkastes.
/// Unntak er `folder.N=` (bare med samtykke), der verdien er et mappenavn uten skilletegn.
pub fn redact_report(text: &str) -> String {
    let mut out = String::new();
    for line in text.lines() {
        let Some((k, v)) = line.split_once('=') else {
            continue;
        };
        let ok = valid_key(k)
            && if k.starts_with("folder.") {
                !v.is_empty()
                    && v.chars().count() <= 40
                    && !v
                        .chars()
                        .any(|c| c.is_control() || matches!(c, '/' | '\\' | ':' | '='))
            } else {
                valid_value(v)
            };
        if ok {
            out.push_str(line);
            out.push('\n');
        }
    }
    out
}

/// Alt rapporten trenger, uten filsystem og uten tilstand: lett å teste.
pub struct ReportInput<'a> {
    pub core: DiagReport,
    pub app_version: &'a str,
    pub os: &'a str,
    pub arch: &'a str,
    pub os_version: Option<&'a str>,
    pub scale_percent: Option<u32>,
    pub log: Vec<LogRecord>,
    /// `(kode, antall, sum ms, maks ms)`.
    pub perf: Vec<(&'static str, u64, u64, u64)>,
    /// Bare med samtykke.
    pub folders: Vec<String>,
}

/// Setter sammen den redigerte teksten. Samme tekst vises i forhåndsvisningen og lagres i filen.
pub fn build_text(i: &ReportInput<'_>) -> String {
    let mut t = String::new();
    t.push_str(&format!("diag_format={SHELL_FORMAT}\n"));
    t.push_str(&format!("app_version={}\n", i.app_version));
    t.push_str(&format!("core_version={}\n", fjell_core::VERSION));
    t.push_str(&format!("os={}\n", i.os));
    t.push_str(&format!("arch={}\n", i.arch));
    t.push_str(&format!(
        "os_version={}\n",
        i.os_version.unwrap_or("unknown")
    ));
    match i.scale_percent {
        Some(s) => t.push_str(&format!("scale_percent={s}\n")),
        None => t.push_str("scale_percent=unknown\n"),
    }
    t.push_str(&i.core.text);
    for (code, count, total, max) in &i.perf {
        t.push_str(&format!("perf.{code}.count={count}\n"));
        t.push_str(&format!("perf.{code}.total_ms={total}\n"));
        t.push_str(&format!("perf.{code}.max_ms={max}\n"));
    }
    t.push_str(&format!("log_entries={}\n", i.log.len()));
    for (n, r) in i.log.iter().enumerate() {
        t.push_str(&format!("log.{}={}\n", n + 1, r.report_value()));
    }
    for (n, f) in i.folders.iter().enumerate() {
        t.push_str(&format!("folder.{}={f}\n", n + 1));
    }
    redact_report(&t)
}

/// Resultat av eksport: rapporten og hvor filen ligger (så brukeren selv kan sende den).
#[derive(Serialize, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DiagSavedDto {
    pub report: DiagDto,
    /// Full sti til filen i appens diagnosemappe. Vises bare for brukeren; havner aldri i rapporten.
    pub saved_path: String,
}

/// Mappen eksporten lagres i.
pub fn export_dir(data_dir: &Path) -> PathBuf {
    data_dir.join("diagnose")
}

impl Shell {
    fn diag_core_report(&self) -> DiagReport {
        let Ok((api, _)) = self.parts() else {
            return DiagReport::empty();
        };
        let r = api.diag_preview().unwrap_or_else(|_| DiagReport::empty());
        if r.metrics.is_empty() {
            fallback_metrics(api.as_ref(), r)
        } else {
            r
        }
    }

    fn diag_text(&self, opts: DiagOptions) -> (DiagReport, String) {
        let (os_version, scale) = {
            let e = ENV.lock().unwrap_or_else(|e| e.into_inner());
            (e.os_version.clone(), e.scale_percent)
        };
        let os_version = os_version.or_else(linux_os_version);
        let log = self
            .data_dir
            .as_deref()
            .map(|d| RollingLog::in_data_dir(d).tail(REPORT_LOG_ENTRIES))
            .unwrap_or_default();
        let perf = PERF
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .iter()
            .map(|(c, p)| (*c, p.count, p.total_ms, p.max_ms))
            .collect();
        let folders = if opts.folder_names.is_some() {
            self.parts()
                .ok()
                .and_then(|(_, c)| c)
                .map(|c| c.roots().iter().map(|r| folder_label(&r.path)).collect())
                .unwrap_or_default()
        } else {
            Vec::new()
        };
        let core = self.diag_core_report();
        let text = build_text(&ReportInput {
            core: core.clone(),
            app_version: env!("CARGO_PKG_VERSION"),
            os: std::env::consts::OS,
            arch: std::env::consts::ARCH,
            os_version: os_version.as_deref(),
            scale_percent: scale,
            log,
            perf,
            folders,
        });
        (core, text)
    }

    fn diag_dto(&self, opts: DiagOptions) -> DiagDto {
        let (core, text) = self.diag_text(opts);
        let mut dto = DiagDto::from(&core);
        dto.text = text;
        dto
    }

    /// Rapporten slik brukeren ser den før noe lagres (standard: uten mappenavn).
    pub fn diag_preview(&self) -> Result<DiagDto, ErrorDto> {
        Ok(self.diag_dto(DiagOptions::default()))
    }

    /// Som `diag_preview`, med valg (mappenavn krever samtykke).
    pub fn diag_preview_with(&self, opts: DiagOptions) -> Result<DiagDto, ErrorDto> {
        Ok(self.diag_dto(opts))
    }

    /// Lagrer rapporten (samme tekst som forhåndsvisningen) i appens diagnosemappe. Ingenting sendes.
    pub fn diag_export(&self) -> Result<DiagDto, ErrorDto> {
        Ok(self.diag_export_with(DiagOptions::default())?.report)
    }

    /// Eksport med valg; returnerer også stien filen ble lagret på.
    pub fn diag_export_with(&self, opts: DiagOptions) -> Result<DiagSavedDto, ErrorDto> {
        let failed = || ErrorDto::new("core_error", "diag_failed");
        let dir = export_dir(self.data_dir.as_deref().ok_or_else(failed)?);
        let dto = self.diag_dto(opts);
        fs::create_dir_all(&dir).map_err(|_| failed())?;
        let mut ts = now();
        let path = loop {
            let p = dir.join(format!("fjell-diagnose-{ts}.txt"));
            if !p.exists() {
                break p;
            }
            ts += 1;
        };
        fs::write(&path, dto.text.as_bytes()).map_err(|_| failed())?;
        Ok(DiagSavedDto {
            report: dto,
            saved_path: path.to_string_lossy().into_owned(),
        })
    }

    /// Logger en feil fra skallet eller kjernen. `raw_code` kan være en vilkårlig feilmelding; bare kjente koder
    /// beholdes. Best effort: en loggfeil skal aldri gi brukeren en ny feil.
    pub fn diag_log(&self, category: Category, raw_code: &str, n: u64) {
        if let Some(d) = self.data_dir.as_deref() {
            let _ =
                RollingLog::in_data_dir(d).append(&LogRecord::new(now(), category, raw_code, n));
        }
    }
}

/// Når kjernen ikke har egen diagnose: tall fra det offentlige API-et (samme tall som Hjem og Regler viser).
fn fallback_metrics(api: &dyn FjellApi, base: DiagReport) -> DiagReport {
    let home = api.home_summary();
    let mut m = vec![
        (DiagMetric::AssetsTrashed, home.trash.count),
        (
            DiagMetric::Suggestions,
            home.per_kind.iter().map(|k| k.count).sum(),
        ),
    ];
    if let Ok(r) = api.rules_list() {
        m.push((DiagMetric::Rules, r.len() as u64));
    }
    if let Ok(p) = api.protections() {
        m.push((DiagMetric::Protections, p.len() as u64));
    }
    if let Ok(b) = api.backup_list() {
        m.push((DiagMetric::Backups, b.len() as u64));
    }
    DiagReport::new(base.integrity, base.indexing, m)
}

#[cfg(test)]
#[path = "diag_tests.rs"]
mod tests;
