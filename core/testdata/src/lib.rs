//! Golden-korpus og deterministisk generator (PLAN-KJERNE del 2, K0).
//! Grensesnitt: `corpus::generate(out, seed)`; moduler: `rng`, `ops`, `synth`, `manifest`.
//! Frittstående crate; fjell-core er bare dev-avhengighet for golden-testene i `tests/`. Generert data ligger i
//! `core/testdata/generated/` (gitignorert) og lages ved behov.

// Pikselløkker med indekser er klarere enn iteratorer her.
#![allow(clippy::needless_range_loop, clippy::manual_is_multiple_of)]

pub mod corpus;
pub mod manifest;
pub mod ops;
pub mod rng;
pub mod synth;
