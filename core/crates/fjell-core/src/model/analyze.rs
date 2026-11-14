//! Analysatorporter i `model` (L0) for V1: øyne og ansikter. Se docs/v1/V1-PLAN.md.
//!
//! Avhenger av: `super` (`Thumb`). Ingen I/O og ingen modellfiler i kjernen; ekte implementasjoner
//! (ONNX e.l.) ligger utenfor `core/crates/fjell-core` og injiseres. Standard er Null-implementasjonene.
//! Grensesnitt:
//! - `EyeAnalyzer`: lukkede øyne per bilde (brukes av serier for «beste bilde»).
//! - `FaceAnalyzer`: ansiktsboks og embedding per ansikt (brukes av personer; krever `face_index`).
//! - `NullEyeAnalyzer`/`NullFaceAnalyzer`: svarer alltid `AnalyzeError::Unavailable`, slik at kallere
//!   aldri lagrer «ingen lukkede øyne» for noe som ikke er analysert.
//!
//! Embeddings forlater aldri enheten (CLAUDE.md: on-device først).

use super::Thumb;
use std::fmt;

/// Feil fra en analysator.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AnalyzeError {
    /// Ingen modell er installert (Null-implementasjonen), eller bildet kan ikke analyseres.
    Unavailable,
    Failed(String),
}

impl fmt::Display for AnalyzeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AnalyzeError::Unavailable => write!(f, "analyzer unavailable"),
            AnalyzeError::Failed(m) => write!(f, "analyzer failed: {m}"),
        }
    }
}

impl std::error::Error for AnalyzeError {}

/// Resultat for ett bilde fra `EyeAnalyzer`.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct EyeReport {
    /// Antall ansikter funnet.
    pub faces: u8,
    /// Antall av dem med minst ett lukket øye.
    pub closed: u8,
    /// 0,0..=1,0, analysatorens sikkerhet for `closed`.
    pub confidence: f32,
}

/// Lukkede øyne per bilde.
pub trait EyeAnalyzer: Send + Sync {
    fn analyze(&self, thumb: &Thumb) -> Result<EyeReport, AnalyzeError>;
}

/// Standard: ingen analyse.
#[derive(Clone, Copy, Debug, Default)]
pub struct NullEyeAnalyzer;

impl EyeAnalyzer for NullEyeAnalyzer {
    fn analyze(&self, _thumb: &Thumb) -> Result<EyeReport, AnalyzeError> {
        Err(AnalyzeError::Unavailable)
    }
}

/// Ansiktsboks i promille av bildets bredde og høyde (0..=1000), uavhengig av oppløsning.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct FaceBox {
    pub x: u16,
    pub y: u16,
    pub w: u16,
    pub h: u16,
}

/// Ansiktsembedding (kun lokal). Lagres som little-endian f32 i `face_embedding.embedding`.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Embedding(pub Vec<f32>);

impl Embedding {
    pub fn to_le_bytes(&self) -> Vec<u8> {
        self.0.iter().flat_map(|v| v.to_le_bytes()).collect()
    }

    /// `None` hvis lengden ikke er et multiplum av 4.
    pub fn from_le_bytes(bytes: &[u8]) -> Option<Embedding> {
        if !bytes.len().is_multiple_of(4) {
            return None;
        }
        Some(Embedding(
            bytes
                .chunks_exact(4)
                .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
                .collect(),
        ))
    }
}

/// Ett ansikt funnet av `FaceAnalyzer`.
#[derive(Clone, Debug, PartialEq)]
pub struct FaceObservation {
    pub bbox: FaceBox,
    /// 0,0..=1,0.
    pub quality: f32,
    pub embedding: Embedding,
}

/// Ansiktsdeteksjon og embedding. Bare kalt når brukeren har slått på `face_index`.
pub trait FaceAnalyzer: Send + Sync {
    fn detect(&self, thumb: &Thumb) -> Result<Vec<FaceObservation>, AnalyzeError>;
}

/// Standard: ingen ansiktsanalyse.
#[derive(Clone, Copy, Debug, Default)]
pub struct NullFaceAnalyzer;

impl FaceAnalyzer for NullFaceAnalyzer {
    fn detect(&self, _thumb: &Thumb) -> Result<Vec<FaceObservation>, AnalyzeError> {
        Err(AnalyzeError::Unavailable)
    }
}
