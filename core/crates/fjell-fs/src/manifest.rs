//! Karantenemanifest (`<karantene>/<batch>/manifest.json`): opprinnelig sti, id og størrelse per fil.
//!
//! Grensesnitt: `Manifest`/`Entry` (serde), `Manifest::load/save` (atomisk skriving via
//! tmp + rename), `scan_batches` (alle batcher under karantenemappen) og `README_TEXT`.
//! Manifestet skrives FØR filene flyttes (hensikt først); oppstartsavstemming bruker filenes
//! faktiske plassering som sannhet, så en avbrutt flytting mister aldri en fil (T26).

use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

pub const MANIFEST_FILE: &str = "manifest.json";
pub const README_FILE: &str = "README.txt";
pub const README_TEXT: &str =
    "Dette er Fjells karantene. Filene her er bilder du har flyttet til papirkurven i Fjell.\n\
Ikke slett eller endre mappen for hånd: åpne Fjell og bruk Angre eller Gjenopprett.\n\
manifest.json forteller hvor hver fil lå opprinnelig.\n";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entry {
    pub id: u64,
    /// Opprinnelig `source_id` (relativ sti med `/`).
    pub original: String,
    /// Filnavn inne i batchmappen.
    pub stored: String,
    pub bytes: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Manifest {
    pub batch: u64,
    pub entries: Vec<Entry>,
}

impl Manifest {
    pub fn load(batch_dir: &Path) -> io::Result<Manifest> {
        let raw = fs::read(batch_dir.join(MANIFEST_FILE))?;
        serde_json::from_slice(&raw).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
    }

    /// Atomisk: skriv til tmp, så rename over den gamle.
    pub fn save(&self, batch_dir: &Path) -> io::Result<()> {
        fs::create_dir_all(batch_dir)?;
        let tmp = batch_dir.join(format!("{MANIFEST_FILE}.tmp"));
        let json = serde_json::to_vec_pretty(self).map_err(io::Error::other)?;
        fs::write(&tmp, json)?;
        fs::rename(&tmp, batch_dir.join(MANIFEST_FILE))
    }
}

/// Alle batchmapper med lesbart manifest, sortert på sti (deterministisk).
pub fn scan_batches(quarantine_dir: &Path) -> Vec<(PathBuf, Manifest)> {
    let Ok(rd) = fs::read_dir(quarantine_dir) else {
        return Vec::new();
    };
    let mut dirs: Vec<PathBuf> = rd
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.is_dir())
        .collect();
    dirs.sort();
    dirs.into_iter()
        .filter_map(|d| Manifest::load(&d).ok().map(|m| (d, m)))
        .collect()
}
