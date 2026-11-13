//! Skallets egen lille konfigurasjon i app-data: sist valgte mappe og indeksfil per mappe.
//!
//! Grensesnitt: `last_library`, `save_library`, `db_path`, `language`, `set_language`, og for flere
//! mapper (docs/mvp/FLERE-MAPPER.md): `extra_roots`, `add_extra_root`, `set_extra_roots`.
//! Ingenting her berører bildefiler.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
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
    /// Ekstra mapper per bibliotek, nøklet på indeksfilens navn (`db_path`). `library` over er
    /// alltid første rot. Gamle filer uten feltet leses som «ingen ekstra mapper».
    #[serde(default)]
    libraries: BTreeMap<String, LibraryRoots>,
}

/// Ekstra røtter for ett bibliotek. Nøkler deles aldri ut på nytt (`next_key` bare øker), så en
/// gammel indeksrad `/r2/…` aldri kan peke inn i en mappe som ble lagt til senere.
#[derive(Serialize, Deserialize, Default, Clone)]
struct LibraryRoots {
    #[serde(default)]
    next_key: u32,
    #[serde(default)]
    roots: Vec<RootEntry>,
}

#[derive(Serialize, Deserialize, Clone)]
struct RootEntry {
    key: u32,
    path: String,
}

/// Første nøkkel for en ekstra rot (første rot har alltid nøkkel 1).
const FIRST_EXTRA_KEY: u32 = 2;

fn library_key(data_dir: &Path, library: &Path) -> String {
    db_path(data_dir, library)
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// Ekstra røtter (nøkkel ≥ 2, sti) for biblioteket med `library` som første rot, i lagret
/// rekkefølge. Oppføringer med ugyldig eller duplisert nøkkel (hånd-redigert fil) hoppes over.
pub fn extra_roots(data_dir: &Path, library: &Path) -> Vec<(u32, PathBuf)> {
    let c = read(data_dir);
    let mut out: Vec<(u32, PathBuf)> = Vec::new();
    if let Some(l) = c.libraries.get(&library_key(data_dir, library)) {
        for r in &l.roots {
            if r.key >= FIRST_EXTRA_KEY && !out.iter().any(|(k, _)| *k == r.key) {
                out.push((r.key, PathBuf::from(&r.path)));
            }
        }
    }
    out
}

/// Legger til en ekstra rot og returnerer den nye nøkkelen (aldri brukt før i dette biblioteket).
pub fn add_extra_root(data_dir: &Path, library: &Path, root: &Path) -> std::io::Result<u32> {
    let mut c = read(data_dir);
    let l = c
        .libraries
        .entry(library_key(data_dir, library))
        .or_default();
    let used = l.roots.iter().map(|r| r.key + 1).max().unwrap_or(0);
    let key = l.next_key.max(used).max(FIRST_EXTRA_KEY);
    l.next_key = key + 1;
    l.roots.push(RootEntry {
        key,
        path: root.to_string_lossy().into_owned(),
    });
    write(data_dir, &c)?;
    Ok(key)
}

/// Beholder bare de ekstra røttene med nøkkel i `keep` (fjerning). `next_key` røres aldri.
pub fn retain_extra_roots(data_dir: &Path, library: &Path, keep: &[u32]) -> std::io::Result<()> {
    let mut c = read(data_dir);
    let Some(l) = c.libraries.get_mut(&library_key(data_dir, library)) else {
        return Ok(());
    };
    // Tellerverket sikres før oppføringene forsvinner (også for filer skrevet uten `next_key`).
    let used = l.roots.iter().map(|r| r.key + 1).max().unwrap_or(0);
    l.next_key = l.next_key.max(used);
    l.roots.retain(|r| keep.contains(&r.key));
    write(data_dir, &c)
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

/// FNV-1a over tekst (stabil på tvers av Rust-versjoner). Samme funksjon som den gamle hashingen.
fn fnv1a(text: &str) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in text.bytes() {
        h = (h ^ u64::from(b)).wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

fn index_file(data_dir: &Path, text: &str) -> PathBuf {
    data_dir.join(format!("index-{:016x}.sqlite", fnv1a(text)))
}

/// Egen indeksfil per mappe (FNV-1a av kanonisk sti).
pub fn db_path(data_dir: &Path, library: &Path) -> PathBuf {
    // Samme mappe under ulike stavemåter (`lib`, `lib/.`, symlenke) må gi samme indeks, ellers deler
    // to indekser én karantene (KR-3). Frakoblet mappe: leksikalsk normalisering (uten `.` og sluttskråstrek).
    let norm = std::fs::canonicalize(library).unwrap_or_else(|_| library.components().collect());
    index_file(data_dir, &norm.to_string_lossy())
}

fn same_dir(a: &Path, b: &Path) -> bool {
    match (std::fs::canonicalize(a), std::fs::canonicalize(b)) {
        (Ok(x), Ok(y)) => x == y,
        _ => a.components().eq(b.components()),
    }
}

fn trim_sep(s: &str) -> &str {
    let t = s.trim_end_matches(['/', '\\']);
    if t.is_empty() {
        s
    } else {
        t
    }
}

/// Kandidater for GAMMEL indeksfil (før KR-3: hash av stien slik den ble gitt, ikke kanonisert), i
/// prioritert rekkefølge: rå som lagret i `shell.json` (bare hvis den peker på samme mappe), rå som
/// gitt, så samme uten avsluttende skråstrek, så uten Windows' `\\?\`-prefiks. Uten duplikater og
/// uten den nye (kanoniske) filen. Rene strengoperasjoner: virker likt på alle plattformer.
pub fn legacy_index_candidates(
    data_dir: &Path,
    library: &Path,
    saved: Option<&Path>,
) -> Vec<PathBuf> {
    let given = library.to_string_lossy().into_owned();
    let saved = saved
        .filter(|s| same_dir(s, library))
        .map(|s| s.to_string_lossy().into_owned());
    let raws: Vec<&str> = saved
        .iter()
        .map(String::as_str)
        .chain([given.as_str()])
        .collect();
    let mut strings: Vec<String> = raws.iter().map(|r| (*r).to_string()).collect();
    strings.extend(raws.iter().map(|r| trim_sep(r).to_string()));
    let stripped: Vec<String> = strings
        .iter()
        .filter_map(|r| r.strip_prefix(r"\\?\").map(str::to_string))
        .collect();
    strings.extend(stripped);
    let new = db_path(data_dir, library);
    let mut out: Vec<PathBuf> = Vec::new();
    for s in strings {
        let f = index_file(data_dir, &s);
        if f != new && !out.contains(&f) {
            out.push(f);
        }
    }
    out
}

/// Indeksfilen som finnes nå for mappen, uten å flytte noe: ny hvis den finnes, ellers første gamle
/// kandidat som finnes, ellers den nye (opprettes ved åpning).
pub fn locate_index(data_dir: &Path, library: &Path, saved: Option<&Path>) -> PathBuf {
    let new = db_path(data_dir, library);
    if new.exists() {
        return new;
    }
    legacy_index_candidates(data_dir, library, saved)
        .into_iter()
        .find(|c| c.exists())
        .unwrap_or(new)
}

fn with_suffix(p: &Path, suffix: &str) -> PathBuf {
    let mut o = p.as_os_str().to_os_string();
    o.push(suffix);
    PathBuf::from(o)
}

/// Flytter `old` (+ `-wal`, `-shm`) til `new`: wal, shm, hovedfil sist. Overskriver aldri. Feiler ett
/// trinn, rulles de flyttede filene tilbake (best effort) og feilen returneres. Sletter aldri noe.
fn move_index(
    old: &Path,
    new: &Path,
    rename: &dyn Fn(&Path, &Path) -> std::io::Result<()>,
) -> std::io::Result<()> {
    // Rester under det nye navnet (f.eks. en gammel `-wal`) ville blitt koblet til feil hovedfil.
    for suffix in ["-wal", "-shm", ""] {
        let dst = with_suffix(new, suffix);
        if dst.exists() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::AlreadyExists,
                format!("{} finnes allerede", dst.display()),
            ));
        }
    }
    let mut done: Vec<(PathBuf, PathBuf)> = Vec::new();
    for suffix in ["-wal", "-shm", ""] {
        let (src, dst) = (with_suffix(old, suffix), with_suffix(new, suffix));
        if !src.exists() {
            continue;
        }
        if let Err(e) = rename(&src, &dst) {
            for (s, d) in done.iter().rev() {
                let _ = fs::rename(d, s);
            }
            return Err(e);
        }
        done.push((src, dst));
    }
    Ok(())
}

/// Trygg migrering av indeks fra gammel til kanonisk navn (KR-3). Kall FØR `Store::open`, og bare når
/// ingen Store har filen åpen (`in_use` = filen en åpen kjerne bruker nå). Finnes den nye, brukes den
/// og gammel røres ikke. Mislykkes flyttingen, brukes gammel fil direkte (ikke-fatalt, logges).
/// Returnerer indeksfilen som skal åpnes. Sletter aldri en indeks.
pub fn adopt_legacy_index(
    data_dir: &Path,
    library: &Path,
    saved: Option<&Path>,
    in_use: Option<&Path>,
    rename: &dyn Fn(&Path, &Path) -> std::io::Result<()>,
) -> PathBuf {
    let new = db_path(data_dir, library);
    if new.exists() {
        return new;
    }
    let Some(old) = legacy_index_candidates(data_dir, library, saved)
        .into_iter()
        .find(|c| c.exists())
    else {
        return new;
    };
    if in_use == Some(old.as_path()) {
        return old;
    }
    match move_index(&old, &new, rename) {
        Ok(()) => new,
        Err(e) => {
            eprintln!(
                "fjell: kunne ikke flytte indeksen {} til {} ({e}); bruker den gamle filen",
                old.display(),
                new.display()
            );
            old
        }
    }
}
