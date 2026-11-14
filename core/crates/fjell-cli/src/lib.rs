//! `fjell-cli` som bibliotek: filsystemadapter og kommandoer (testsele, ARKITEKTUR del 5).
//!
//! Ligger utenfor `core/crates/fjell-core` og dermed utenfor lagreglene (plattformadapter).
//! Grensesnitt:
//! - `fsource::FsSource`: omslag rundt `fjell_fs::FsSource` uten bildedekoding (som før M5).
//! - `cmd::{scan, list}`: kommandoene, skriver til en `Write`.
//! - `perf::{SynthSource, run}`: ytelsesmåling med syntetiske bilder (`fjell-cli perf`).
//! - `calibrate::{run, evaluate, parse_labels}`: kalibrering mot etikettert korpus (`fjell-cli calibrate`).
//! - `flow::{index, trash, undo, purge}`: fullflyt mot ekte kjerne (`Fjell`), purge bare i temp-kopi.
//! - `v1::run`: `protect`, `series`, `dashboard`, `people`, `rules` (V1; ingen sletting av bilder).

pub mod calibrate;
pub mod cmd;
pub mod crashtest;
pub mod flow;
pub mod fsource;
pub mod perf;
pub mod v1;
