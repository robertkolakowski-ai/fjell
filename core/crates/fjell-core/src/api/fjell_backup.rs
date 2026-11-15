//! Robusthetsflaten for `Fjell` (L4): sikkerhetskopi av indeksen og integritet (backup-delen). Del av `api`.
//! Indekseringsdelen (`pipeline::control`) og diagnosen legges i egne `impl`-blokker av sine grener.
//!
//! Avhenger av: `super::fjell` (`Fjell`), store (`Store::backup_*`, `check_file`, `restore_backup`), model.
//! Grensesnitt: `pub(super)` metoder på `Fjell` som `impl FjellApi for Fjell` (i `fjell_ops`) delegerer til,
//! pluss `Fjell::backup_auto_if_due` (kalles av skallet ved trygge tidspunkter).
//!
//! Regler (docs/v1/BACKUP.md): kopier tas via egen skrivebeskyttet tilkobling og holder aldri skrivelåsen
//! (en handling venter ikke på en kopi). Gjenoppretting krever `RestoreAck`, tar en `PreRestore`-kopi først og
//! avvises mens en skanning pågår (`index_busy`). Ingenting her rører originalbilder. Feil er koder.

use super::fjell::Fjell;
use super::{ApiError, BackupEntry, RestoreReport};
use crate::model::{BackupId, BackupKind, IntegrityStatus, RestoreAck};
use crate::pipeline::{GroupEngine, Phase};
use crate::store::Store;
use std::path::PathBuf;

/// En automatisk kopi når ingen er tatt siste uke («ukentlig»).
const WEEKLY_SECS: i64 = 7 * 24 * 3600;
/// Før en stor «Tøm» tas en ny kopi hvis den siste er eldre enn dette.
const BEFORE_PURGE_SECS: i64 = 3600;

/// `BackupMeta` kan ikke navngis utenfor `store` (modulen er privat), derfor et makro i stedet for en funksjon.
macro_rules! entry {
    ($m:expr) => {{
        let m = $m;
        BackupEntry {
            id: m.id,
            kind: m.kind,
            created_at: m.created_at,
            bytes: m.bytes,
            asset_count: m.asset_count,
            integrity: m.integrity,
        }
    }};
}

fn code(c: &str) -> ApiError {
    ApiError::Internal(c.to_string())
}

impl Fjell {
    /// Indeksfilen. Låsen på indeksen holdes bare mens stien leses.
    fn backup_db(&self) -> Result<PathBuf, ApiError> {
        self.actions()
            .store_mut()
            .db_path()
            .map(|p| p.to_path_buf())
            .ok_or_else(|| code("backup_failed"))
    }

    fn scan_busy(&self) -> bool {
        !matches!(self.sh.progress.snapshot().phase, Phase::Done | Phase::Idle)
    }

    pub(super) fn backup_make(&self) -> Result<BackupEntry, ApiError> {
        self.backup_take(BackupKind::Manual, "manual")
    }

    fn backup_take(&self, kind: BackupKind, reason: &str) -> Result<BackupEntry, ApiError> {
        let db = self.backup_db()?;
        let keep = self.actions().store_mut().backup_keep_auto();
        let now = self.sh.clock.now();
        let made =
            Store::backup_create_at(&db, kind, reason, now, keep).map_err(|e| code(e.code()))?;
        Ok(entry!(&made.meta))
    }

    pub(super) fn backup_entries(&self) -> Result<Vec<BackupEntry>, ApiError> {
        let Ok(db) = self.backup_db() else {
            return Ok(Vec::new()); // minnedatabase: ingen kopier
        };
        Ok(Store::backup_list_at(&db)
            .iter()
            .map(|m| entry!(m))
            .collect())
    }

    pub(super) fn backup_restore_ack(&self, ack: RestoreAck) -> Result<RestoreReport, ApiError> {
        self.backup_swap_in(ack.backup())
    }

    /// Angre: setter `safety` tilbake. Bare `PreRestore`-kopier godtas (ellers `backup_not_found`).
    pub(super) fn backup_undo(&self, safety: BackupId) -> Result<RestoreReport, ApiError> {
        let db = self.backup_db()?;
        let is_safety = Store::backup_list_at(&db)
            .iter()
            .any(|m| m.id == safety && m.kind == BackupKind::PreRestore);
        if !is_safety {
            return Err(code("backup_not_found"));
        }
        self.backup_swap_in(safety)
    }

    fn backup_swap_in(&self, id: BackupId) -> Result<RestoreReport, ApiError> {
        if self.scan_busy() {
            return Err(code("index_busy"));
        }
        let now = self.sh.clock.now();
        // Hele byttet skjer under skrivelåsen, så ingen handling skriver til en halvt byttet indeks.
        let done = self
            .actions()
            .store_mut()
            .restore_backup(id, now)
            .map_err(|e| code(e.code()))?;
        // Gruppebufferen hører til den gamle indeksen.
        *self.sh.engine.lock().unwrap_or_else(|e| e.into_inner()) = GroupEngine::new();
        let _ = self.refresh();
        Ok(RestoreReport {
            restored: done.restored.id,
            safety: done.safety.id,
            asset_count: done.restored.asset_count,
        })
    }

    /// Full integritetssjekk av indeksfilen via egen tilkobling (blokkerer ikke skriveren). Minnedatabase: `Ok`.
    pub(super) fn integrity_now(&self) -> Result<IntegrityStatus, ApiError> {
        match self.backup_db() {
            Ok(db) => Ok(Store::check_file(&db, true).status()),
            Err(_) => Ok(IntegrityStatus::Ok),
        }
    }

    /// Automatisk kopi ved et trygt tidspunkt, bare hvis det trengs: `weekly` (ingen kopi siste 7 døgn) eller
    /// `before_purge` (ingen siste time). Gir `None` når ingenting ble tatt. Kjøres av skallet i bakgrunnen
    /// (aldri i en UI-handling). Kopien er synlig i listen og rulleres bare blant `Auto`-kopier.
    pub fn backup_auto_if_due(&self, reason: &str) -> Result<Option<BackupEntry>, ApiError> {
        let window = match reason {
            "weekly" => WEEKLY_SECS,
            "before_purge" => BEFORE_PURGE_SECS,
            _ => return Err(code("backup_failed")),
        };
        let now = self.sh.clock.now();
        let recent = self
            .backup_entries()?
            .iter()
            .any(|b| b.kind != BackupKind::PreRestore && b.created_at > now - window);
        if recent {
            return Ok(None);
        }
        self.backup_take(BackupKind::Auto, reason).map(Some)
    }
}

#[cfg(test)]
mod tests;
