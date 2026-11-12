//! Golden-korpus og deterministisk generator (PLAN-KJERNE del 2, K0).
//! Grensesnitt: `corpus::generate(out, seed)` (golden), `blurset::generate_blur(out, seed)` (utvidet
//! uskarphetskorpus for K8; `FJELL_BLURSET=1 fjell-gen-golden` lager begge); moduler: `rng`, `ops`,
//! `synth`, `textures`, `manifest`.
//! Frittstående crate; fjell-core er bare dev-avhengighet for golden-testene i `tests/`. Generert data ligger i
//! `core/testdata/generated/` (gitignorert) og lages ved behov.

// Pikselløkker med indekser er klarere enn iteratorer her.
#![allow(clippy::needless_range_loop, clippy::manual_is_multiple_of)]

pub mod blurset;
pub mod corpus;
pub mod manifest;
pub mod ops;
pub mod rng;
pub mod synth;
pub mod textures;
