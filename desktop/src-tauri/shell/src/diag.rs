//! Diagnose for skallet (`impl Shell`). Del av `fjell-shell`. Skall i kontraktfasen: tynne delegasjoner til
//! `FjellApi`; grenen som eier filen utvider dem (docs/v1/ROBUSTHET-KONTRAKT.md). Tauri-kommandoene
//! (`diag_preview`, `diag_export`) er allerede registrert i `fjell-desktop`.
//!
//! Avhenger av: `fjell-core::api` (`FjellApi`), `dto_robust`, `dto`. Personvern (docs/PERSONVERN.md): rapporten
//! har bare tall og koder; ingen stier, filnavn eller bilder, og skallet sender den aldri noe sted selv.
//! Hvor eksporten havner (utklippstavle, fil via Rust-dialog) er et åpent spørsmål for eier.

use crate::dto::ErrorDto;
use crate::dto_robust::DiagDto;
use crate::Shell;

impl Shell {
    /// Rapporten slik brukeren ser den før eksport.
    pub fn diag_preview(&self) -> Result<DiagDto, ErrorDto> {
        Ok(DiagDto::from(&self.api()?.diag_preview()?))
    }

    /// Rapporten til eksport (samme innhold som forhåndsvisningen).
    pub fn diag_export(&self) -> Result<DiagDto, ErrorDto> {
        Ok(DiagDto::from(&self.api()?.diag_export()?))
    }
}
