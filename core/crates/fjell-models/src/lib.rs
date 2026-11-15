//! `fjell-models`: modelladapter for V1 (øyne og ansikter). Ligger utenfor `fjell-core`,
//! som `fjell-fs`; kjernen kjenner ikke denne krate-en (håndheves av `tools/check-layers`).
//!
//! Grensesnitt (fylles ut i egen fase, se `docs/v1/MODELLER.md`):
//! - Implementasjoner av `fjell_core::model::EyeAnalyzer` og `FaceAnalyzer`.
//! - Bruker bare `fjell_core::model`; ingen andre kjernemoduler.
//!
//! Foreløpig tom: ingen modellfiler, ingen runtime, ingen nettverk (CLAUDE.md: on-device).

/// Krate-ens versjon (fra Cargo).
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg(test)]
mod tests {
    use super::*;
    use fjell_core::model::{AnalyzeError, EyeAnalyzer, NullEyeAnalyzer};

    #[test]
    fn crate_compiles_and_sees_core_model() {
        assert!(!VERSION.is_empty());
        // Bekrefter at avhengigheten mot `fjell_core::model` virker (kjernen er L0 for denne krate-en).
        fn assert_analyzer<T: EyeAnalyzer>(_: &T) {}
        assert_analyzer(&NullEyeAnalyzer);
        let _ = AnalyzeError::Unavailable;
    }
}
