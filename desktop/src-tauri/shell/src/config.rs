//! Skallets egen lille konfigurasjon i app-data: sist valgte mappe og indeksfil per mappe.
//!
//! Grensesnitt: `last_library`, `save_library`, `db_path`. Ingenting her berører bildefiler.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

const FILE: &str = "shell.json";

#[derive(Serialize, Deserialize, Default)]
struct Config {
    library: Option<String>,
}

pub fn last_library(data_dir: &Path) -> Option<PathBuf> {
    let raw = fs::read(data_dir.join(FILE)).ok()?;
    let c: Config = serde_json::from_slice(&raw).ok()?;
    c.library.map(PathBuf::from)
}

pub fn save_library(data_dir: &Path, library: &Path) -> std::io::Result<()> {
    fs::create_dir_all(data_dir)?;
    let c = Config {
        library: Some(library.to_string_lossy().into_owned()),
    };
    let tmp = data_dir.join(format!("{FILE}.tmp"));
    fs::write(
        &tmp,
        serde_json::to_vec_pretty(&c).map_err(std::io::Error::other)?,
    )?;
    fs::rename(tmp, data_dir.join(FILE))
}

/// Egen indeksfil per mappe (FNV-1a av stien; stabil på tvers av Rust-versjoner).
pub fn db_path(data_dir: &Path, library: &Path) -> PathBuf {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in library.to_string_lossy().bytes() {
        h = (h ^ u64::from(b)).wrapping_mul(0x0000_0100_0000_01b3);
    }
    data_dir.join(format!("index-{h:016x}.sqlite"))
}
