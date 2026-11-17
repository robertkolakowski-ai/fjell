//! Typer for robusthetsflaten i `api` (L4): sikkerhetskopi av indeksen, indeksering og diagnose.
//!
//! Avhenger av: `crate::model` (`robust`), `super::types` (`ScanPhase`). Alle typer bærer koder, tall og ID-er,
//! aldri tekst. Se docs/v1/ROBUSTHET-KONTRAKT.md.
//! Grensesnitt: `BackupEntry`, `RestoreReport`, `IndexingStatus`, `IndexingSettings` (`MAX_INDEX_THREADS`).

use super::ScanPhase;
use crate::model::{BackupId, BackupKind, IndexPriority, IndexState, IntegrityStatus};

/// Én sikkerhetskopi av indeksen slik UI leser den.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BackupEntry {
    pub id: BackupId,
    pub kind: BackupKind,
    /// Unix-sekunder.
    pub created_at: i64,
    pub bytes: u64,
    /// Antall bilder i indeksen da kopien ble tatt.
    pub asset_count: u64,
    /// Kopiens egen integritet (en ødelagt kopi kan ikke gjenopprettes).
    pub integrity: IntegrityStatus,
}

/// Resultat av en gjenoppretting. `safety` er kopien av indeksen slik den var rett før (`BackupKind::PreRestore`);
/// `backup_restore_undo(safety)` setter den tilbake.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RestoreReport {
    pub restored: BackupId,
    pub safety: BackupId,
    pub asset_count: u64,
}

/// Indekseringens status (bakgrunnsskanning). Tall, ingen tekst.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IndexingStatus {
    pub state: IndexState,
    pub phase: ScanPhase,
    pub done: u64,
    pub total: u64,
    /// Bilder per sekund (glidende snitt); 0 når ukjent.
    pub rate_per_sec: u32,
    /// Beregnet gjenstående tid; `None` når den ikke kan anslås.
    pub eta_secs: Option<u64>,
}

impl Default for IndexingStatus {
    fn default() -> Self {
        IndexingStatus {
            state: IndexState::Idle,
            phase: ScanPhase::Idle,
            done: 0,
            total: 0,
            rate_per_sec: 0,
            eta_secs: None,
        }
    }
}

/// Størst antall indekseringstråder brukeren kan velge.
pub const MAX_INDEX_THREADS: u32 = 64;

/// Innstillinger for indeksering (lagres i `setting`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IndexingSettings {
    /// Antall tråder; 0 = automatisk (kjernen velger). Høyst `MAX_INDEX_THREADS`.
    pub threads: u32,
    pub priority: IndexPriority,
    /// Pause på batteri og fortsett når strømmen er tilbake (standard på).
    pub pause_on_battery: bool,
}

impl Default for IndexingSettings {
    fn default() -> Self {
        IndexingSettings {
            threads: 0,
            priority: IndexPriority::Low,
            pause_on_battery: true,
        }
    }
}

impl IndexingSettings {
    /// Gyldig når `threads <= MAX_INDEX_THREADS`.
    pub fn is_valid(&self) -> bool {
        self.threads <= MAX_INDEX_THREADS
    }
}
