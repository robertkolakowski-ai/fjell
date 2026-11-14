//! `MemFaceAnalyzer`: deterministisk ansiktsanalysator for tester og golden-korpus (ingen modell, ingen I/O).
//!
//! Avhenger av: `crate::model` (`FaceAnalyzer`, `Thumb`, `FaceObservation`, `AnalyzeError`).
//! Grensesnitt: `MemFaceAnalyzer::new().with_faces(thumb_data, faces)` / `.failing(feil)`. Ukjent miniatyr gir
//! `Ok(vec![])` (ekte «ingen ansikter»); `NullFaceAnalyzer` svarer derimot `Unavailable`. Teller kall slik at
//! tester kan bevise at analysatoren ikke kjøres når ansiktsindeksen er av.

use crate::model::{AnalyzeError, FaceAnalyzer, FaceObservation, Thumb};
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};

#[derive(Debug, Default)]
pub struct MemFaceAnalyzer {
    by_data: HashMap<Vec<u8>, Vec<FaceObservation>>,
    error: Option<AnalyzeError>,
    calls: AtomicUsize,
}

impl MemFaceAnalyzer {
    pub fn new() -> Self {
        Self::default()
    }

    /// Miniatyrer med nøyaktig disse bytene gir disse ansiktene.
    pub fn with_faces(mut self, thumb_data: Vec<u8>, faces: Vec<FaceObservation>) -> Self {
        self.by_data.insert(thumb_data, faces);
        self
    }

    /// Alle kall feiler med `error`.
    pub fn failing(mut self, error: AnalyzeError) -> Self {
        self.error = Some(error);
        self
    }

    /// Antall `detect`-kall så langt.
    pub fn calls(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }
}

impl FaceAnalyzer for MemFaceAnalyzer {
    fn detect(&self, thumb: &Thumb) -> Result<Vec<FaceObservation>, AnalyzeError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        if let Some(e) = &self.error {
            return Err(e.clone());
        }
        Ok(self.by_data.get(&thumb.data).cloned().unwrap_or_default())
    }
}
