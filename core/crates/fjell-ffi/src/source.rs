//! Plattformadapter over UniFFI (ingen logikk): `AssetSourceCallback` implementeres av Swift/Kotlin og
//! pakkes inn som `fjell_core::model::AssetSource`. Kun lesing; ingen skrivemetoder (beslutning 17).
//!
//! Avhenger av: `fjell_core::model`, `types`. Grensesnitt: `AssetSourceCallback`, `AssetMeta`,
//! `ChangeSet`, `Thumb`, `SourceFailure`, `CallbackSource` (til `lib.rs`). Filbytes leses i biter
//! (`read_chunk`) og settes sammen til en `Read` her.

use crate::types::{AssetKind, BackupState, ThumbFmt};
use fjell_core::model as m;
use std::io::Read;
use std::sync::Arc;

/// Feil fra plattformen. Må ha `From<UnexpectedUniFFICallbackError>` for UniFFI-callbacks.
#[derive(Clone, Debug, PartialEq, Eq, uniffi::Error)]
pub enum SourceFailure {
    Unavailable,
    Corrupt,
    NotFound,
    Failed { detail: String },
}

impl std::fmt::Display for SourceFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for SourceFailure {}

impl From<uniffi::UnexpectedUniFFICallbackError> for SourceFailure {
    fn from(e: uniffi::UnexpectedUniFFICallbackError) -> Self {
        SourceFailure::Failed {
            detail: e.to_string(),
        }
    }
}

impl From<SourceFailure> for m::SourceError {
    fn from(e: SourceFailure) -> Self {
        match e {
            SourceFailure::Unavailable => m::SourceError::Unavailable,
            SourceFailure::Corrupt => m::SourceError::Corrupt,
            SourceFailure::NotFound => m::SourceError::NotFound,
            SourceFailure::Failed { detail } => m::SourceError::Failed(detail),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct Album {
    pub id: u64,
    pub name: String,
    pub shared: bool,
}

#[derive(Clone, Debug, PartialEq, uniffi::Record)]
pub struct AssetMeta {
    pub source_id: String,
    pub kind: AssetKind,
    pub created_at: i64,
    pub mod_token: String,
    pub width: u32,
    pub height: u32,
    pub bytes: u64,
    pub mime: String,
    pub platform_screenshot_hint: bool,
    pub has_camera_exif: Option<bool>,
    pub is_local: bool,
    pub protect_flags: u8,
    pub albums: Vec<Album>,
    pub backup: Option<BackupState>,
}

impl From<AssetMeta> for m::AssetMeta {
    fn from(a: AssetMeta) -> Self {
        m::AssetMeta {
            source_id: m::SourceId(a.source_id),
            kind: a.kind.into(),
            created_at: a.created_at,
            mod_token: a.mod_token,
            width: a.width,
            height: a.height,
            bytes: a.bytes,
            mime: a.mime,
            platform_screenshot_hint: a.platform_screenshot_hint,
            has_camera_exif: a.has_camera_exif,
            is_local: a.is_local,
            protect_flags: m::ProtectFlags(a.protect_flags),
            albums: a
                .albums
                .into_iter()
                .map(|x| m::Album {
                    id: x.id,
                    name: x.name,
                    shared: x.shared,
                })
                .collect(),
            backup: a.backup.map(Into::into),
        }
    }
}

#[derive(Clone, Debug, PartialEq, uniffi::Record)]
pub struct ChangeSet {
    pub changed: Vec<AssetMeta>,
    pub removed: Vec<String>,
    pub next_token: String,
}

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct Thumb {
    pub w: u32,
    pub h: u32,
    pub fmt: ThumbFmt,
    pub data: Vec<u8>,
}

/// Lesegrensen som plattformen (Swift/Kotlin) implementerer.
#[uniffi::export(with_foreign)]
pub trait AssetSourceCallback: Send + Sync {
    /// Endringer siden `since` (`None` = alt), inkl. fjernede kilder.
    fn list_changes(&self, since: Option<String>) -> Result<ChangeSet, SourceFailure>;
    /// Miniatyr (lengste side ≤ 256 px, orientering brukt).
    fn thumbnail(&self, source_id: String) -> Result<Thumb, SourceFailure>;
    /// Opptil `max_len` bytes fra `offset`; tom liste betyr slutt på filen.
    fn read_chunk(
        &self,
        source_id: String,
        offset: u64,
        max_len: u32,
    ) -> Result<Vec<u8>, SourceFailure>;
}

const CHUNK: u32 = 256 * 1024;

/// `AssetSource` som delegerer til plattformens callback.
pub(crate) struct CallbackSource(pub Arc<dyn AssetSourceCallback>);

struct ChunkReader {
    cb: Arc<dyn AssetSourceCallback>,
    id: String,
    offset: u64,
}

impl Read for ChunkReader {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let want = (buf.len() as u32).min(CHUNK);
        if want == 0 {
            return Ok(0);
        }
        let got = self
            .cb
            .read_chunk(self.id.clone(), self.offset, want)
            .map_err(|e| std::io::Error::other(e.to_string()))?;
        let n = got.len().min(buf.len());
        buf[..n].copy_from_slice(&got[..n]);
        self.offset += n as u64;
        Ok(n)
    }
}

impl m::AssetSource for CallbackSource {
    fn list_changes(&self, since: Option<&m::ChangeToken>) -> Result<m::ChangeSet, m::SourceError> {
        let cs = self
            .0
            .list_changes(since.map(|t| t.0.clone()))
            .map_err(m::SourceError::from)?;
        Ok(m::ChangeSet {
            changed: cs.changed.into_iter().map(Into::into).collect(),
            removed: cs.removed.into_iter().map(m::SourceId).collect(),
            next_token: m::ChangeToken(cs.next_token),
        })
    }

    fn thumbnail(&self, id: &m::SourceId) -> Result<m::Thumb, m::SourceError> {
        let t = self
            .0
            .thumbnail(id.0.clone())
            .map_err(m::SourceError::from)?;
        // Validerer størrelse og datalengde; ugyldig miniatyr regnes som ødelagt.
        m::Thumb::new(t.w, t.h, t.fmt.into(), t.data).map_err(|_| m::SourceError::Corrupt)
    }

    fn read_bytes(&self, id: &m::SourceId) -> Result<Box<dyn Read + Send>, m::SourceError> {
        Ok(Box::new(ChunkReader {
            cb: self.0.clone(),
            id: id.0.clone(),
            offset: 0,
        }))
    }
}
