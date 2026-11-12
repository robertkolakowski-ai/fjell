//! Traits i `model` (L0): grensene mot plattform og tid. Se docs/mvp/ARKITEKTUR.md del 2.
//!
//! Avhenger av: `super` (typer). Ingen I/O i kjernen; adaptere ligger utenfor `core/`.
//! Grensesnitt:
//! - `AssetSource`: kun lesing (`list_changes`, `thumbnail`, `read_bytes`). Ingen skrivemetoder
//!   (originaler endres aldri, beslutning 17).
//! - `PlatformDelete`: egen trait for sletting; krever `PurgeTicket`. Kalles bare av UI.
//! - `Clock`: injisert tid (Unix-sekunder).

use super::{AssetId, ChangeSet, ChangeToken, PurgeTicket, SourceId, Thumb};
use std::fmt;
use std::io::Read;

/// Feil fra adapter. `Unavailable` (ikke lastet ned) skilles fra `Corrupt`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SourceError {
    Unavailable,
    Corrupt,
    NotFound,
    Failed(String),
}

impl fmt::Display for SourceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SourceError::Unavailable => write!(f, "source unavailable"),
            SourceError::Corrupt => write!(f, "source corrupt"),
            SourceError::NotFound => write!(f, "source not found"),
            SourceError::Failed(m) => write!(f, "source failed: {m}"),
        }
    }
}

impl std::error::Error for SourceError {}

/// Lesegrense mot plattformen (PhotoKit, filsystem, ...). Ingen skrivemetoder.
pub trait AssetSource: Send + Sync {
    /// Endringer siden `since` (`None` = alt). Melder også fjernede kilder.
    fn list_changes(&self, since: Option<&ChangeToken>) -> Result<ChangeSet, SourceError>;
    /// Miniatyr, lengste side 256 px, orientering brukt.
    fn thumbnail(&self, id: &SourceId) -> Result<Thumb, SourceError>;
    /// Strømmende lesing av filbytes.
    fn read_bytes(&self, id: &SourceId) -> Result<Box<dyn Read + Send>, SourceError>;
}

/// Utfall per element ved plattformens sletting.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeleteOutcome {
    Deleted,
    Cancelled,
    Failed,
}

/// Plattformens sletting. Bare UI kaller denne, og bare med gyldig `PurgeTicket` fra `api`.
pub trait PlatformDelete: Send + Sync {
    fn delete(
        &self,
        ticket: &PurgeTicket,
        ids: &[(AssetId, SourceId)],
    ) -> Vec<(AssetId, DeleteOutcome)>;
}

/// Tidskilde. Injiseres slik at utløp og angre kan testes.
pub trait Clock: Send + Sync {
    /// Unix-sekunder.
    fn now(&self) -> i64;
}
