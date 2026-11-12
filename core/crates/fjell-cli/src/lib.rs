//! `fjell-cli` som bibliotek: filsystemadapter og kommandoer (testsele, ARKITEKTUR del 5).
//!
//! Ligger utenfor `core/crates/fjell-core` og dermed utenfor lagreglene (plattformadapter).
//! Grensesnitt:
//! - `fsource::FsSource`: `AssetSource` over en mappe, kun lesing.
//! - `cmd::{scan, list}`: kommandoene, skriver til en `Write`.

pub mod cmd;
pub mod fsource;
