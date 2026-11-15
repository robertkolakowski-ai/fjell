//! `fjell-models`: modellramme for V1 (øyne og ansikter). Ligger utenfor `fjell-core`,
//! som `fjell-fs`; kjernen kjenner ikke denne krate-en (håndheves av `tools/check-layers`).
//!
//! Avhenger bare av `fjell_core::model`. Se `docs/v1/MODELLER.md`.
//! Grensesnitt:
//! - `ModelManifest`: lokal JSON-beskrivelse (hash, størrelse, lisens, former, kalibrerte terskler).
//! - `load_verified`/`verify_bytes`: verifiserer hash, størrelse (≤ 30 MB) og lisens (tillatt-liste).
//! - `OnnxSession`/`SessionFactory`: kjøreport; `MockSession` til tester; ekte ONNX bak featuren `onnx`.
//! - `ModelEyeAnalyzer`/`ModelFaceAnalyzer`: implementerer `EyeAnalyzer`/`FaceAnalyzer`, svarer
//!   `Unavailable` (aldri «ingen funn») når modell mangler, feiler verifisering eller kjøremotor mangler.
//!
//! Ingen nettverkskode og ingen modellfiler (CLAUDE.md: on-device først).

pub mod adapters;
pub mod error;
pub mod loader;
pub mod manifest;
pub mod session;

pub use adapters::{preprocess, ModelEyeAnalyzer, ModelFaceAnalyzer};
pub use error::ModelError;
pub use loader::{load_verified, verify_bytes, VerifiedModel};
pub use manifest::{
    license_allowed, ModelManifest, Purpose, ALLOWED_LICENSES, EYE_MIN_CONFIDENCE,
    FACE_MIN_QUALITY, MAX_MODEL_BYTES,
};
pub use session::{MockFactory, MockSession, NoRuntime, OnnxSession, SessionFactory, Tensor};

/// Krate-ens versjon (fra Cargo).
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg(test)]
mod tests {
    use super::*;
    use fjell_core::model::{AnalyzeError, EyeAnalyzer, NullEyeAnalyzer};

    #[test]
    fn crate_compiles_and_sees_core_model() {
        assert!(!VERSION.is_empty());
        fn assert_analyzer<T: EyeAnalyzer>(_: &T) {}
        assert_analyzer(&NullEyeAnalyzer);
        let _ = AnalyzeError::Unavailable;
    }
}
