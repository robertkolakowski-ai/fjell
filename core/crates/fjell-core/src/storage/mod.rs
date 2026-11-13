//! Modul `storage` (L2): lagringsoversikt, ren leseside over indeksen. Se docs/mvp/LAGRING.md.
//!
//! Avhenger av: store, model. Bare lesing: ingen skriving, ingen dekoding, ingen filtilgang. Alt regnes ut
//! fra `asset.bytes`, så video og bilder som ikke er lastet ned (`Unavailable`) teller med fullt ut.
//! Grensesnitt:
//! - `summarize(&Store, &StorageOptions)` og `summarize_on(&Connection, ..)`: hele `StorageSummary`.
//! - `largest_files_on(&Connection, n)`: bare de største filene.
//!
//! Effektivitet: ett strømmende pass over aktive rader (ingen tabell i minne; bare mapper, topp-N-haug og
//! tellere), pluss to lette pass for forslagskategorier og papirkurv. Alle summer er `u64` med metning.

mod scan;
#[cfg(test)]
mod tests;

use crate::model::{AssetId, SuggestionKind};
use crate::store::{Store, StoreError};
use rusqlite::Connection;

/// Gruppering for visning. `ScreenshotHint` hører til `Other` (skjermbilder vises under forslag).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum StorageKind {
    Photo,
    Video,
    Other,
}

impl StorageKind {
    pub const ALL: [StorageKind; 3] = [Self::Photo, Self::Video, Self::Other];
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KindUsage {
    pub kind: StorageKind,
    pub count: u64,
    pub bytes: u64,
}

/// Bruk per rot. Rot-nummeret er indeksen i skallets liste over valgte mapper (0 i dag).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RootUsage {
    pub root: u32,
    pub count: u64,
    pub bytes: u64,
}

/// Mappe, rot-relativ med `/`. Hierarkisk: en mappe teller alt under seg, så en forelder og barna
/// overlapper (ikke summer på tvers av `depth`). Filer rett i roten inngår bare i `RootUsage`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FolderUsage {
    pub root: u32,
    pub path: String,
    /// 1 = rett under roten.
    pub depth: u32,
    pub count: u64,
    pub bytes: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileUsage {
    pub id: AssetId,
    pub root: u32,
    /// Rot-relativ sti (`source_id`).
    pub path: String,
    pub bytes: u64,
    pub kind: StorageKind,
}

/// Plass bak en forslagskategori: ventende, synlige forslag på aktive bilder (samme regel som kortene,
/// så hvert bilde teller i én kategori og kategoriene kan summeres).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Reclaimable {
    pub kind: SuggestionKind,
    pub count: u64,
    pub bytes: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StorageSummary {
    /// Aktive bilder (ikke i papirkurv, ikke purget).
    pub total_count: u64,
    pub total_bytes: u64,
    /// Alltid tre rader i rekkefølgen Foto, Video, Annet (også med 0).
    pub per_kind: Vec<KindUsage>,
    /// Stigende rot-nummer; én rot i dag, tom for tom indeks.
    pub per_root: Vec<RootUsage>,
    /// Størst først; lik størrelse sorteres på rot, så sti.
    pub per_folder: Vec<FolderUsage>,
    /// Størst først; lik størrelse sorteres på sti, så id.
    pub largest_files: Vec<FileUsage>,
    /// Alltid fire rader i rekkefølgen til `SuggestionKind::ALL`.
    pub reclaimable_by_category: Vec<Reclaimable>,
    pub reclaimable_total_bytes: u64,
    pub trash_count: u64,
    pub trash_bytes: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StorageOptions {
    /// Antall mapper i `per_folder`.
    pub folder_limit: usize,
    /// Antall nivåer under roten det aggregeres for.
    pub folder_depth: usize,
    /// Antall filer i `largest_files`.
    pub file_limit: usize,
}

impl Default for StorageOptions {
    fn default() -> Self {
        StorageOptions {
            folder_limit: 20,
            folder_depth: 3,
            file_limit: 20,
        }
    }
}

/// Oversikt over indeksen til `store` (egen tilkobling hvis du har `Store::reader`, se `Fjell::storage_summary`).
pub fn summarize(store: &Store, opts: &StorageOptions) -> Result<StorageSummary, StoreError> {
    summarize_on(store.read_conn(), opts)
}

pub fn summarize_on(
    conn: &Connection,
    opts: &StorageOptions,
) -> Result<StorageSummary, StoreError> {
    scan::summarize(conn, opts)
}

/// De `n` største aktive filene.
pub fn largest_files_on(conn: &Connection, n: usize) -> Result<Vec<FileUsage>, StoreError> {
    let opts = StorageOptions {
        folder_limit: 0,
        folder_depth: 0,
        file_limit: n,
    };
    Ok(scan::scan_active(conn, &opts)?.largest)
}
