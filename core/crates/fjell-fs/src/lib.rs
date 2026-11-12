//! `fjell-fs`: desktop-adapter (ARKITEKTUR beslutning 20-22). Ligger utenfor `fjell-core` og
//! lagreglene; kjernen kjenner ikke plattformen.
//!
//! Grensesnitt:
//! - `FsSource`: `AssetSource` over mappe/ekstern disk/UNC (kun lesing), miniatyrer (`thumb`),
//!   skyplassholdere (`locality`).
//! - `Quarantine`/`FsQuarantine`: flytting til `.fjell-karantene` med manifest og avstemming.
//! - `RecycleBin`/`FsDeleter`: «Tøm» via Papirkurv (standard) eller permanent med `PermanentAck`.
//! - `path`: lange stier og UNC. `windows` (cfg(windows)): Papirkurv-implementasjon mot Windows.

pub mod delete;
pub mod drives;
pub mod locality;
pub mod manifest;
pub mod path;
pub mod quarantine;
pub mod source;
pub mod thumb;
pub mod thumbcache;
#[cfg(windows)]
pub mod windows;

pub use delete::{
    DeleteError, DeleteMode, DeleteResult, FakeRecycleBin, FsDeleter, PermanentAck, RecycleBin,
    RecycleError,
};
pub use quarantine::{FsQuarantine, Quarantine, QuarantineError, Reconciled, QUARANTINE_DIR};
pub use source::FsSource;
