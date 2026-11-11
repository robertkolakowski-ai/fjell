//! Fjell-kjernen. Modulene mapper til modulgrafen i `docs/mvp/ARKITEKTUR.md` del 2.
//!
//! Lag (en modul kan bare bruke moduler i lavere lag; håndheves av `tools/check-layers`,
//! konfig i `tools/check-layers/layers.toml`):
//!
//! ```text
//! L4  api
//! L3  pipeline
//! L2  cluster   suggest   actions
//! L1  store   hash   quality   screenshot
//! L0  model
//! ```

pub mod actions;
pub mod api;
pub mod cluster;
pub mod hash;
pub mod model;
pub mod pipeline;
pub mod quality;
pub mod screenshot;
pub mod store;
pub mod suggest;

/// Kjernens versjon (fra Cargo).
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
