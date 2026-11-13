//! Skallets egen lille konfigurasjon i app-data: sist valgte mappe og indeksfil per mappe.
//!
//! Grensesnitt: `last_library`, `save_library`, `db_path`, `language`, `set_language`. Ingenting her berører bildefiler.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

const FILE: &str = "shell.json";

#[derive(Serialize, Deserialize, Default)]
struct Config {
    library: Option<String>,
    /// Onboarding er vist (første gang). Settes når første skann startes; vises aldri på nytt.
    #[serde(default)]
    onboarded: bool,
    /// «Sjekk automatisk ved oppstart» (beta-kanalen). Standard av; gamle filer uten feltet leses som av.
    #[serde(default)]
    auto_update: bool,
    /// Språkvalg: `system`, `nb` eller `en`. `None` = aldri valgt (frontend faller da tilbake på sitt eget valg).
    #[serde(default)]
    language: Option<String>,
}

/// Språkvalg frontend kan lagre. `pseudo` er et testlokale og er ikke et brukervalg.
pub const LANGUAGES: [&str; 3] = ["system", "nb", "en"];

fn read(data_dir: &Path) -> Config {
    fs::read(data_dir.join(FILE))
        .ok()
        .and_then(|raw| serde_json::from_slice(&raw).ok())
        .unwrap_or_default()
}

pub fn last_library(data_dir: &Path) -> Option<PathBuf> {
    read(data_dir).library.map(PathBuf::from)
}

/// Første gang = onboarding er ikke fullført og ingen mappe er valgt.
pub fn first_run(data_dir: &Path) -> bool {
    let c = read(data_dir);
    !c.onboarded && c.library.is_none()
}

pub fn save_library(data_dir: &Path, library: &Path) -> std::io::Result<()> {
    let mut c = read(data_dir);
    c.library = Some(library.to_string_lossy().into_owned());
    write(data_dir, &c)
}

pub fn auto_update(data_dir: &Path) -> bool {
    read(data_dir).auto_update
}

pub fn set_auto_update(data_dir: &Path, on: bool) -> std::io::Result<()> {
    let mut c = read(data_dir);
    c.auto_update = on;
    write(data_dir, &c)
}

pub fn set_onboarded(data_dir: &Path) -> std::io::Result<()> {
    let mut c = read(data_dir);
    c.onboarded = true;
    write(data_dir, &c)
}

/// Lagret språkvalg. Ukjent verdi i filen (hånd-redigert, nyere versjon) regnes som ikke valgt.
pub fn language(data_dir: &Path) -> Option<String> {
    read(data_dir)
        .language
        .filter(|l| LANGUAGES.contains(&l.as_str()))
}

pub fn set_language(data_dir: &Path, language: &str) -> std::io::Result<()> {
    let mut c = read(data_dir);
    c.language = Some(language.to_string());
    write(data_dir, &c)
}

fn write(data_dir: &Path, c: &Config) -> std::io::Result<()> {
    fs::create_dir_all(data_dir)?;
    let tmp = data_dir.join(format!("{FILE}.tmp"));
    fs::write(
        &tmp,
        serde_json::to_vec_pretty(c).map_err(std::io::Error::other)?,
    )?;
    fs::rename(tmp, data_dir.join(FILE))
}

/// Egen indeksfil per mappe (FNV-1a av stien; stabil på tvers av Rust-versjoner).
pub fn db_path(data_dir: &Path, library: &Path) -> PathBuf {
    // Samme mappe under ulike stavemåter (`lib`, `lib/.`, symlenke) må gi samme indeks, ellers deler
    // to indekser én karantene (KR-3). Frakoblet mappe: leksikalsk normalisering (uten `.` og sluttskråstrek).
    let norm = std::fs::canonicalize(library).unwrap_or_else(|_| library.components().collect());
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in norm.to_string_lossy().bytes() {
        h = (h ^ u64::from(b)).wrapping_mul(0x0000_0100_0000_01b3);
    }
    data_dir.join(format!("index-{h:016x}.sqlite"))
}
