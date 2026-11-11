//! «Tøm» på PC (ARKITEKTUR beslutning 17 og 22): `PlatformDelete` over filsystemet.
//!
//! Grensesnitt:
//! - `RecycleBin`: Windows Papirkurv bak en trait (`windows::WindowsRecycleBin`, `FakeRecycleBin` i tester).
//! - `DeleteMode`: `Recycle` (standard) eller `Permanent`. Permanent krever `PermanentAck`
//!   (avkrysning + antall som matcher billettens), ellers `DeleteError::AckRequired`, og ingenting kalles.
//! - `FsDeleter`: `impl PlatformDelete`; `delete_detailed` gir også `Unsupported`/`Unavailable`.
//!
//! Regler: `Unsupported` (volum uten Papirkurv) faller aldri stille tilbake til permanent; UI
//! tilbyr permanent med egen advarsel. Frakoblet rot sletter ingenting. Filer i karantene slettes
//! fra karantenestien, og manifestoppføringen fjernes etterpå.

use crate::path::{os_path, resolve};
use crate::quarantine::FsQuarantine;
use fjell_core::model::{AssetId, DeleteOutcome, PlatformDelete, PurgeTicket, SourceId};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeleteMode {
    Recycle,
    Permanent,
}

/// Brukerens ekstra bekreftelse for permanent sletting: avkrysning pluss knapp med antall.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PermanentAck {
    count: usize,
}

impl PermanentAck {
    /// `checked` = avkrysningen i arket; `count` = tallet på knappen. `None` uten avkrysning.
    pub fn new(checked: bool, count: usize) -> Option<PermanentAck> {
        (checked && count > 0).then_some(PermanentAck { count })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DeleteError {
    /// Permanent uten `PermanentAck`.
    AckRequired,
    /// Antallet i `PermanentAck` stemmer ikke med billetten.
    AckMismatch,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RecycleError {
    /// Volumet har ingen Papirkurv (nettverksstasjon, enkelte eksterne disker).
    Unsupported,
    Cancelled,
    Failed(String),
}

pub trait RecycleBin: Send + Sync {
    /// Flytt én fil til Papirkurv. Ingen systemdialog.
    fn recycle(&self, path: &Path) -> Result<(), RecycleError>;
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DeleteResult {
    Deleted,
    Cancelled,
    Failed,
    /// Ingen Papirkurv på volumet. Ikke slettet; UI kan tilby permanent.
    Unsupported,
    /// Roten er frakoblet. Ikke slettet.
    Unavailable,
}

/// Testdobbel: logger kall; stier som starter med et `unsupported`-prefiks gir `Unsupported`.
#[derive(Default)]
pub struct FakeRecycleBin {
    pub calls: Mutex<Vec<PathBuf>>,
    pub unsupported_under: Vec<PathBuf>,
    /// Hvis sann: flytter filen til `bin_dir` (etterligner Papirkurv); ellers bare logging.
    pub bin_dir: Option<PathBuf>,
}

impl RecycleBin for FakeRecycleBin {
    fn recycle(&self, path: &Path) -> Result<(), RecycleError> {
        if self.unsupported_under.iter().any(|p| path.starts_with(p)) {
            return Err(RecycleError::Unsupported);
        }
        self.calls.lock().unwrap().push(path.to_path_buf());
        if let Some(dir) = &self.bin_dir {
            let name = path.file_name().unwrap_or_default();
            fs::create_dir_all(dir).map_err(|e| RecycleError::Failed(e.to_string()))?;
            fs::rename(path, dir.join(name)).map_err(|e| RecycleError::Failed(e.to_string()))?;
        }
        Ok(())
    }
}

pub struct FsDeleter {
    library: PathBuf,
    quarantine: Option<Arc<FsQuarantine>>,
    bin: Arc<dyn RecycleBin>,
    mode: DeleteMode,
    ack: Option<PermanentAck>,
}

impl FsDeleter {
    /// `Permanent` uten `ack` gir `AckRequired`.
    pub fn new(
        library: impl Into<PathBuf>,
        quarantine: Option<Arc<FsQuarantine>>,
        bin: Arc<dyn RecycleBin>,
        mode: DeleteMode,
        ack: Option<PermanentAck>,
    ) -> Result<FsDeleter, DeleteError> {
        if mode == DeleteMode::Permanent && ack.is_none() {
            return Err(DeleteError::AckRequired);
        }
        Ok(FsDeleter {
            library: library.into(),
            quarantine,
            bin,
            mode,
            ack,
        })
    }

    fn path_for(&self, id: AssetId, sid: &SourceId) -> Option<PathBuf> {
        self.quarantine
            .as_ref()
            .and_then(|q| q.locate(id))
            .or_else(|| resolve(&self.library, sid).ok())
    }

    pub fn delete_detailed(
        &self,
        ticket: &PurgeTicket,
        ids: &[(AssetId, SourceId)],
    ) -> Result<Vec<(AssetId, DeleteResult)>, DeleteError> {
        if let (DeleteMode::Permanent, Some(a)) = (self.mode, self.ack) {
            if a.count != ticket.ids().len() {
                return Err(DeleteError::AckMismatch);
            }
        }
        let offline = !os_path(&self.library).is_dir();
        let mut out = Vec::with_capacity(ids.len());
        for (id, sid) in ids {
            let res = if !ticket.ids().contains(id) {
                DeleteResult::Failed // ikke dekket av billetten
            } else if offline {
                DeleteResult::Unavailable
            } else {
                self.delete_one(*id, sid)
            };
            out.push((*id, res));
        }
        Ok(out)
    }

    fn delete_one(&self, id: AssetId, sid: &SourceId) -> DeleteResult {
        let Some(p) = self.path_for(id, sid) else {
            return DeleteResult::Failed;
        };
        let p = os_path(&p);
        if !p.is_file() {
            return DeleteResult::Failed; // avstemmes til purged (external) ved neste skanning
        }
        let r = match self.mode {
            DeleteMode::Recycle => self.bin.recycle(&p),
            DeleteMode::Permanent => {
                fs::remove_file(&p).map_err(|e| RecycleError::Failed(e.to_string()))
            }
        };
        match r {
            Ok(()) => {
                if let Some(q) = &self.quarantine {
                    q.forget(id);
                }
                DeleteResult::Deleted
            }
            Err(RecycleError::Unsupported) => DeleteResult::Unsupported,
            Err(RecycleError::Cancelled) => DeleteResult::Cancelled,
            Err(RecycleError::Failed(_)) => DeleteResult::Failed,
        }
    }
}

impl PlatformDelete for FsDeleter {
    fn delete(
        &self,
        ticket: &PurgeTicket,
        ids: &[(AssetId, SourceId)],
    ) -> Vec<(AssetId, DeleteOutcome)> {
        let fail_all = || {
            ids.iter()
                .map(|(i, _)| (*i, DeleteOutcome::Failed))
                .collect()
        };
        match self.delete_detailed(ticket, ids) {
            Err(_) => fail_all(),
            Ok(v) => v
                .into_iter()
                .map(|(i, r)| {
                    let o = match r {
                        DeleteResult::Deleted => DeleteOutcome::Deleted,
                        DeleteResult::Cancelled => DeleteOutcome::Cancelled,
                        // Aldri stille fallback til permanent: ikke slettet.
                        _ => DeleteOutcome::Failed,
                    };
                    (i, o)
                })
                .collect(),
        }
    }
}
