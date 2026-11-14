//! Fjell-kjernen. Modulene mapper til modulgrafen i `docs/mvp/ARKITEKTUR.md` del 2.
//!
//! Lag (en modul kan bare bruke moduler i lavere lag; håndheves av `tools/check-layers`,
//! konfig i `tools/check-layers/layers.toml`):
//!
//! ```text
//! L4  api
//! L3  pipeline
//! L2  cluster   suggest   actions   rules   storage   series   people   suggest_rules   protect   dashboard
//! L1  store   hash   quality   screenshot
//! L0  model
//! ```

pub mod actions;
pub mod api;
pub mod cluster;
pub mod dashboard;
pub mod hash;
pub mod model;
pub mod people;
pub mod pipeline;
pub mod protect;
pub mod quality;
pub mod rules;
pub mod screenshot;
pub mod series;
pub mod storage;
pub mod store;
pub mod suggest;
pub mod suggest_rules;

/// Kjernens versjon (fra Cargo).
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
