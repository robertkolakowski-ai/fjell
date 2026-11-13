//! `fjell-fs`: desktop-adapter (ARKITEKTUR beslutning 20-22). Ligger utenfor `fjell-core` og
//! lagreglene; kjernen kjenner ikke plattformen.
//!
//! Grensesnitt:
//! - `FsSource`: `AssetSource` over mappe/ekstern disk/UNC (kun lesing), miniatyrer (`thumb`),
//!   skyplassholdere (`locality`).
//! - `Quarantine`/`FsQuarantine`: flytting til `.fjell-karantene` med manifest og avstemming.
//! - `RecycleBin`/`FsDeleter`: «Tøm» via Papirkurv (standard) eller permanent med `PermanentAck`.
//! - `roots`: flere røtter i ett bibliotek (`MultiSource`, `RootSet`: karantene og ruting per rot).
//! - `cloud`: sikkerhetsnett som kjenner igjen skysynkede mappenavn (iCloud/OneDrive/Dropbox).
//! - `path`: lange stier og UNC. `windows` (cfg(windows)): Papirkurv-implementasjon mot Windows.

pub mod cloud;
pub mod delete;
pub mod drives;
pub mod locality;
pub mod manifest;
pub mod path;
pub mod quarantine;
pub mod roots;
pub mod source;
pub mod thumb;
pub mod thumbcache;
#[cfg(windows)]
pub mod wic;
#[cfg(windows)]
pub mod windows;

pub use cloud::{cloud_provider, CloudProvider};
pub use delete::{
    DeleteError, DeleteMode, DeleteResult, FakeRecycleBin, FsDeleter, PermanentAck, RecycleBin,
    RecycleError,
};
pub use quarantine::{FsQuarantine, Quarantine, QuarantineError, Reconciled, QUARANTINE_DIR};
pub use roots::{MultiSource, RootSet, RootsError};
pub use source::FsSource;
