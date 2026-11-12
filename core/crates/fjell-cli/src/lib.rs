//! `fjell-cli` som bibliotek: filsystemadapter og kommandoer (testsele, ARKITEKTUR del 5).
//!
//! Ligger utenfor `core/crates/fjell-core` og dermed utenfor lagreglene (plattformadapter).
//! Grensesnitt:
//! - `fsource::FsSource`: `AssetSource` over en mappe, kun lesing.
//! - `cmd::{scan, list}`: kommandoene, skriver til en `Write`.
//! - `flow::{index, trash, undo, purge}`: fullflyt mot ekte kjerne (`Fjell`), purge bare i temp-kopi.

pub mod cmd;
pub mod flow;
pub mod fsource;
