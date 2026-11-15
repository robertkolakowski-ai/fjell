//! Indeksering (status, pause, fortsett, avbryt, innstillinger) for skallet (`impl Shell`). Del av `fjell-shell`.
//! Skall i kontraktfasen: tynne delegasjoner til `FjellApi`; grenen som eier filen utvider dem
//! (docs/v1/ROBUSTHET-KONTRAKT.md). Tauri-kommandoene (`indexing_status`, `indexing_pause`, `indexing_resume`,
//! `indexing_cancel`, `get_indexing_settings`, `set_indexing_settings`) er allerede registrert i `fjell-desktop`.
//!
//! Avhenger av: `fjell-core::api` (`FjellApi`), `dto_robust`, `dto`. Strømstatus (pause på batteri) injiseres
//! fra `fjell-fs::power`; skallet kobler den, kjernen kjenner ingen plattform.

use crate::dto::ErrorDto;
use crate::dto_robust::{IndexingSettingsDto, IndexingStatusDto};
use crate::Shell;

impl Shell {
    pub fn indexing_status(&self) -> Result<IndexingStatusDto, ErrorDto> {
        Ok(IndexingStatusDto::from(&self.api()?.indexing_status()?))
    }

    pub fn indexing_pause(&self) -> Result<(), ErrorDto> {
        self.api()?.indexing_pause().map_err(ErrorDto::from)
    }

    pub fn indexing_resume(&self) -> Result<(), ErrorDto> {
        self.api()?.indexing_resume().map_err(ErrorDto::from)
    }

    pub fn indexing_cancel(&self) -> Result<(), ErrorDto> {
        self.api()?.indexing_cancel().map_err(ErrorDto::from)
    }

    pub fn indexing_settings(&self) -> Result<IndexingSettingsDto, ErrorDto> {
        Ok(IndexingSettingsDto::from(&self.api()?.indexing_settings()?))
    }

    /// Lagrer innstillingene. Ukjent prioritetskode gir `indexing_invalid_settings`.
    pub fn set_indexing_settings(
        &self,
        s: &IndexingSettingsDto,
    ) -> Result<IndexingSettingsDto, ErrorDto> {
        let core = s
            .to_core()
            .ok_or_else(|| ErrorDto::new("core_error", "indexing_invalid_settings"))?;
        let api = self.api()?;
        api.indexing_settings_set(core)?;
        Ok(IndexingSettingsDto::from(&api.indexing_settings()?))
    }
}
