//! Sikkerhetskopi av indeksen og integritetssjekk for skallet (`impl Shell`). Del av `fjell-shell`.
//! Skall i kontraktfasen: tynne delegasjoner til `FjellApi`; grenen som eier filen utvider dem
//! (docs/v1/ROBUSTHET-KONTRAKT.md). Tauri-kommandoene (`backup_create`, `backup_list`, `backup_restore`,
//! `backup_restore_undo`, `integrity_check`) er allerede registrert i `fjell-desktop`.
//!
//! Avhenger av: `fjell-core::api` (`FjellApi`), `dto_robust`, `dto`. Regler (CLAUDE.md): kopien inneholder bare
//! indeksen, aldri bilder. Gjenoppretting krever `RestoreAck` (avkrysning fra brukeren) og er angrebar
//! (`backup_restore_undo`). Ingen filsystem her: filarbeidet ligger i kjernen (`store::backup`).

use crate::dto::ErrorDto;
use crate::dto_robust::{BackupDto, RestoreReportDto};
use crate::Shell;
use fjell_core::model::{BackupId, RestoreAck};

impl Shell {
    pub fn backup_create(&self) -> Result<BackupDto, ErrorDto> {
        let e = self.api()?.backup_create()?;
        Ok(BackupDto::from(&e))
    }

    /// Kopiene, nyeste først.
    pub fn backups(&self) -> Result<Vec<BackupDto>, ErrorDto> {
        Ok(self
            .api()?
            .backup_list()?
            .iter()
            .map(BackupDto::from)
            .collect())
    }

    /// Gjenoppretter `id`. `acknowledged` er brukerens avkrysning; uten den sendes `backup_ack_required`
    /// og ingenting røres.
    pub fn backup_restore(
        &self,
        id: u64,
        acknowledged: bool,
    ) -> Result<RestoreReportDto, ErrorDto> {
        let ack = RestoreAck::new(BackupId(id), acknowledged)
            .ok_or_else(|| ErrorDto::new("core_error", "backup_ack_required"))?;
        let r = self.api()?.backup_restore(ack)?;
        Ok(RestoreReportDto::from(&r))
    }

    /// Angrer en gjenoppretting (`safety` fra `RestoreReportDto`).
    pub fn backup_restore_undo(&self, safety: u64) -> Result<RestoreReportDto, ErrorDto> {
        let r = self.api()?.backup_restore_undo(BackupId(safety))?;
        Ok(RestoreReportDto::from(&r))
    }

    /// Integritet som kode (`ok`, `degraded`, `corrupt`, `unreadable`). Uten bibliotek finnes ingen indeks å sjekke: `ok`.
    pub fn integrity(&self) -> Result<&'static str, ErrorDto> {
        match self.api() {
            Ok(api) => Ok(api.integrity_check()?.as_str()),
            Err(e) if e.code == "no_library" => Ok("ok"),
            Err(e) => Err(e),
        }
    }
}
