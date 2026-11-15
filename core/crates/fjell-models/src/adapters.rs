//! Adaptere som implementerer `EyeAnalyzer` og `FaceAnalyzer` over en verifisert modell.
//!
//! Semantikk: mangler modellen, feiler verifiseringen eller finnes ingen runtime, svarer adapteren
//! `AnalyzeError::Unavailable` (aldri et tomt funn). Feil under kjøring av ett bilde gir `Failed`.
//!
//! Utdatakontrakt (modellen pakkes/etterbehandles slik at den følger denne):
//! - Øye: tensor `[1, 3]` = (antall ansikter, antall med lukket øye, sikkerhet 0..1).
//! - Ansikt: tensor `[1, N, 5 + D]`, rad = (x, y, w, h normalisert 0..1, kvalitet 0..1, embedding\[D]).

use crate::error::ModelError;
use crate::loader::{load_verified, VerifiedModel};
use crate::manifest::{ModelManifest, Purpose, EYE_MIN_CONFIDENCE, FACE_MIN_QUALITY};
use crate::session::{OnnxSession, SessionFactory, Tensor};
use fjell_core::model::{
    AnalyzeError, Embedding, EyeAnalyzer, EyeReport, FaceAnalyzer, FaceBox, FaceObservation, Thumb,
    ThumbFmt,
};
use std::path::Path;

/// Konverterer en miniatyr til inn-tensor `[1, C, H, W]` (nærmeste nabo-skalering, `(v/255 - mean) / std`).
pub fn preprocess(thumb: &Thumb, manifest: &ModelManifest) -> Result<Tensor, ModelError> {
    let (c, h, w) = match manifest.input_shape.as_slice() {
        [1, c, h, w] => (*c, *h, *w),
        _ => return Err(ModelError::ManifestInvalid("input_shape".into())),
    };
    let (tw, th) = (thumb.w as usize, thumb.h as usize);
    let bpp = thumb.fmt.bytes_per_pixel();
    if tw == 0 || th == 0 || thumb.data.len() != tw * th * bpp {
        return Err(ModelError::Run("thumb has inconsistent size".into()));
    }
    let px = |x: usize, y: usize| -> [f32; 3] {
        let o = (y * tw + x) * bpp;
        match thumb.fmt {
            ThumbFmt::Gray8 => [f32::from(thumb.data[o]); 3],
            ThumbFmt::Rgba8 => [
                f32::from(thumb.data[o]),
                f32::from(thumb.data[o + 1]),
                f32::from(thumb.data[o + 2]),
            ],
        }
    };
    let mut data = vec![0.0f32; c * h * w];
    for y in 0..h {
        let sy = y * th / h;
        for x in 0..w {
            let sx = x * tw / w;
            let p = px(sx, sy);
            if c == 3 {
                for (ch, v) in p.iter().enumerate() {
                    data[ch * h * w + y * w + x] =
                        (v / 255.0 - manifest.input_mean) / manifest.input_std;
                }
            } else {
                let l = 0.299 * p[0] + 0.587 * p[1] + 0.114 * p[2];
                data[y * w + x] = (l / 255.0 - manifest.input_mean) / manifest.input_std;
            }
        }
    }
    Tensor::new(vec![1, c, h, w], data)
}

enum State {
    Ready {
        manifest: Box<ModelManifest>,
        session: Box<dyn OnnxSession>,
    },
    Unavailable(ModelError),
}

fn open(verified: Result<VerifiedModel, ModelError>, factory: &dyn SessionFactory) -> State {
    match verified.and_then(|m| factory.open(&m).map(|s| (m, s))) {
        Ok((m, session)) => State::Ready {
            manifest: Box::new(m.manifest().clone()),
            session,
        },
        Err(e) => State::Unavailable(e),
    }
}

fn run(state: &State, thumb: &Thumb) -> Result<(Tensor, ModelManifest), AnalyzeError> {
    let State::Ready { manifest, session } = state else {
        return Err(AnalyzeError::Unavailable);
    };
    let input = preprocess(thumb, manifest).map_err(|e| AnalyzeError::Failed(e.to_string()))?;
    let out = session
        .run(&input)
        .map_err(|e| AnalyzeError::Failed(e.to_string()))?;
    Ok((out, (**manifest).clone()))
}

/// Øyeanalysator over en verifisert modell.
pub struct ModelEyeAnalyzer(State);

impl ModelEyeAnalyzer {
    /// Laster og verifiserer `manifest_path`. Feil gir en analysator som svarer `Unavailable`.
    pub fn from_manifest_path(manifest_path: &Path, factory: &dyn SessionFactory) -> Self {
        Self(open(load_verified(manifest_path, Purpose::Eye), factory))
    }
    pub fn from_verified(model: VerifiedModel, factory: &dyn SessionFactory) -> Self {
        let r = if model.manifest().purpose == Purpose::Eye {
            Ok(model)
        } else {
            Err(ModelError::WrongPurpose)
        };
        Self(open(r, factory))
    }
    /// Hvorfor analysatoren er utilgjengelig, hvis den er det.
    pub fn unavailable_reason(&self) -> Option<&ModelError> {
        match &self.0 {
            State::Unavailable(e) => Some(e),
            State::Ready { .. } => None,
        }
    }
}

fn bad(m: impl Into<String>) -> AnalyzeError {
    AnalyzeError::Failed(ModelError::BadOutputValue(m.into()).to_string())
}

impl EyeAnalyzer for ModelEyeAnalyzer {
    fn analyze(&self, thumb: &Thumb) -> Result<EyeReport, AnalyzeError> {
        let (out, manifest) = run(&self.0, thumb)?;
        if out.shape != [1, 3] || out.data.len() != 3 {
            return Err(AnalyzeError::Failed(
                ModelError::BadOutputShape {
                    expected: vec![1, 3],
                    actual: out.shape,
                }
                .to_string(),
            ));
        }
        let (faces, closed, conf) = (out.data[0], out.data[1], out.data[2]);
        if !(faces.is_finite() && closed.is_finite() && conf.is_finite()) {
            return Err(bad("non-finite eye output"));
        }
        if !(0.0..=255.0).contains(&faces) || !(0.0..=faces.round()).contains(&closed) {
            return Err(bad("face/closed counts out of range"));
        }
        if !(0.0..=1.0).contains(&conf) {
            return Err(bad("confidence outside 0..=1"));
        }
        let min = manifest.threshold(EYE_MIN_CONFIDENCE).unwrap_or(1.0);
        // Usikker avgjørelse er «ikke analysert», aldri «ingen lukkede øyne».
        if faces.round() > 0.0 && conf < min {
            return Err(AnalyzeError::Unavailable);
        }
        Ok(EyeReport {
            faces: faces.round() as u8,
            closed: closed.round() as u8,
            confidence: conf,
        })
    }
}

/// Ansiktsanalysator over en verifisert modell.
pub struct ModelFaceAnalyzer(State);

impl ModelFaceAnalyzer {
    pub fn from_manifest_path(manifest_path: &Path, factory: &dyn SessionFactory) -> Self {
        Self(open(load_verified(manifest_path, Purpose::Face), factory))
    }
    pub fn from_verified(model: VerifiedModel, factory: &dyn SessionFactory) -> Self {
        let r = if model.manifest().purpose == Purpose::Face {
            Ok(model)
        } else {
            Err(ModelError::WrongPurpose)
        };
        Self(open(r, factory))
    }
    pub fn unavailable_reason(&self) -> Option<&ModelError> {
        match &self.0 {
            State::Unavailable(e) => Some(e),
            State::Ready { .. } => None,
        }
    }
}

impl FaceAnalyzer for ModelFaceAnalyzer {
    fn detect(&self, thumb: &Thumb) -> Result<Vec<FaceObservation>, AnalyzeError> {
        let (out, manifest) = run(&self.0, thumb)?;
        let d = manifest.embedding_dim;
        let row = 5 + d;
        let n = match out.shape.as_slice() {
            [1, n, r] if *r == row && out.data.len() == n * row => *n,
            _ => {
                return Err(AnalyzeError::Failed(
                    ModelError::BadOutputShape {
                        expected: manifest.output_shape.clone(),
                        actual: out.shape,
                    }
                    .to_string(),
                ))
            }
        };
        let min_q = manifest.threshold(FACE_MIN_QUALITY).unwrap_or(1.0);
        let mut faces = Vec::new();
        for r in out.data.chunks_exact(row).take(n) {
            if r.iter().any(|v| !v.is_finite()) {
                return Err(bad("non-finite face output"));
            }
            let q = r[4];
            if !(0.0..=1.0).contains(&q) {
                return Err(bad("quality outside 0..=1"));
            }
            if q < min_q {
                continue;
            }
            let pm = |v: f32| (v.clamp(0.0, 1.0) * 1000.0).round() as u16;
            faces.push(FaceObservation {
                bbox: FaceBox {
                    x: pm(r[0]),
                    y: pm(r[1]),
                    w: pm(r[2]),
                    h: pm(r[3]),
                },
                quality: q,
                embedding: Embedding(r[5..].to_vec()),
            });
        }
        Ok(faces)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::loader::verify_bytes;
    use crate::manifest::testutil::{eye_json, face_json};
    use crate::session::{MockFactory, MockSession, NoRuntime};
    use std::sync::Arc;

    const DATA: &[u8] = b"model-bytes";

    fn verified(json: impl FnOnce(&str, u64) -> String) -> VerifiedModel {
        let h = blake3::hash(DATA).to_hex().to_string();
        let m = ModelManifest::from_json(&json(&h, DATA.len() as u64)).unwrap();
        verify_bytes(m, DATA.to_vec()).unwrap()
    }

    fn eye_model() -> VerifiedModel {
        verified(|h, b| eye_json(h, b, "MIT"))
    }
    fn face_model(dim: usize) -> VerifiedModel {
        verified(|h, b| face_json(h, b, "MIT", dim))
    }

    fn thumb() -> Thumb {
        Thumb::new(4, 2, ThumbFmt::Rgba8, vec![200; 4 * 2 * 4]).unwrap()
    }

    fn eye_with(out: Result<Tensor, ModelError>) -> ModelEyeAnalyzer {
        let s = match out {
            Ok(t) => MockSession::returning(t),
            Err(e) => MockSession::failing(e),
        };
        ModelEyeAnalyzer::from_verified(eye_model(), &MockFactory(Arc::new(s)))
    }

    fn t(shape: &[usize], data: &[f32]) -> Tensor {
        Tensor::new(shape.to_vec(), data.to_vec()).unwrap()
    }

    #[test]
    fn eye_ok_report() {
        let a = eye_with(Ok(t(&[1, 3], &[2.0, 1.0, 0.9])));
        assert_eq!(
            a.analyze(&thumb()).unwrap(),
            EyeReport {
                faces: 2,
                closed: 1,
                confidence: 0.9
            }
        );
    }

    #[test]
    fn eye_unavailable_without_runtime_or_on_failed_verification() {
        let a = ModelEyeAnalyzer::from_verified(eye_model(), &NoRuntime);
        assert_eq!(a.analyze(&thumb()), Err(AnalyzeError::Unavailable));
        assert_eq!(
            a.unavailable_reason(),
            Some(&ModelError::RuntimeUnavailable)
        );

        let d = tempfile::tempdir().unwrap();
        let missing = ModelEyeAnalyzer::from_manifest_path(&d.path().join("x.json"), &NoRuntime);
        assert_eq!(missing.analyze(&thumb()), Err(AnalyzeError::Unavailable));

        // Hash-feil på disk gir Unavailable, ikke et tomt funn.
        let json = eye_json(&"1".repeat(64), DATA.len() as u64, "MIT");
        std::fs::write(d.path().join("manifest.json"), json).unwrap();
        std::fs::write(d.path().join("m.onnx"), DATA).unwrap();
        let sess = Arc::new(MockSession::returning(t(&[1, 3], &[0.0, 0.0, 1.0])));
        let a = ModelEyeAnalyzer::from_manifest_path(
            &d.path().join("manifest.json"),
            &MockFactory(sess),
        );
        assert_eq!(a.analyze(&thumb()), Err(AnalyzeError::Unavailable));
        assert_eq!(a.unavailable_reason(), Some(&ModelError::HashMismatch));
    }

    #[test]
    fn wrong_purpose_is_unavailable() {
        let a = ModelEyeAnalyzer::from_verified(face_model(4), &NoRuntime);
        assert_eq!(a.unavailable_reason(), Some(&ModelError::WrongPurpose));
        let f = ModelFaceAnalyzer::from_verified(eye_model(), &NoRuntime);
        assert_eq!(f.detect(&thumb()), Err(AnalyzeError::Unavailable));
    }

    #[test]
    fn eye_low_confidence_is_unavailable_not_no_finding() {
        let a = eye_with(Ok(t(&[1, 3], &[1.0, 0.0, 0.3])));
        assert_eq!(a.analyze(&thumb()), Err(AnalyzeError::Unavailable));
        // Ingen ansikter: ingenting å være usikker på.
        let b = eye_with(Ok(t(&[1, 3], &[0.0, 0.0, 0.1])));
        assert_eq!(b.analyze(&thumb()).unwrap().faces, 0);
    }

    #[test]
    fn eye_bad_shape_and_values_fail() {
        for out in [
            t(&[1, 2], &[1.0, 0.0]),
            t(&[3], &[1.0, 0.0, 0.9]),
            t(&[1, 3], &[1.0, 0.0, f32::NAN]),
            t(&[1, 3], &[1.0, 2.0, 0.9]),
            t(&[1, 3], &[-1.0, 0.0, 0.9]),
            t(&[1, 3], &[1.0, 0.0, 1.5]),
        ] {
            assert!(matches!(
                eye_with(Ok(out)).analyze(&thumb()),
                Err(AnalyzeError::Failed(_))
            ));
        }
    }

    #[test]
    fn eye_runtime_error_is_failed() {
        let a = eye_with(Err(ModelError::Run("boom".into())));
        assert!(matches!(a.analyze(&thumb()), Err(AnalyzeError::Failed(m)) if m.contains("boom")));
    }

    #[test]
    fn preprocess_shapes_and_normalizes() {
        let m = face_model(4);
        let x = preprocess(&thumb(), m.manifest()).unwrap();
        assert_eq!(x.shape, vec![1, 3, 8, 8]);
        // mean 0.5, std 0.5: 200/255 -> (0.784-0.5)/0.5
        let want = (200.0 / 255.0 - 0.5) / 0.5;
        assert!(x.data.iter().all(|v| (v - want).abs() < 1e-5));
        let g = Thumb::new(2, 2, ThumbFmt::Gray8, vec![255; 4]).unwrap();
        assert!(preprocess(&g, m.manifest()).is_ok());
    }

    #[test]
    fn face_returns_faces_and_filters_low_quality() {
        let rows = [
            0.1, 0.2, 0.3, 0.4, 0.9, 1.0, 2.0, 3.0, 4.0, //
            0.5, 0.5, 0.5, 0.5, 0.2, 9.0, 9.0, 9.0, 9.0,
        ];
        let s = Arc::new(MockSession::returning(t(&[1, 2, 9], &rows)));
        let a = ModelFaceAnalyzer::from_verified(face_model(4), &MockFactory(s.clone()));
        let f = a.detect(&thumb()).unwrap();
        assert_eq!(f.len(), 1);
        assert_eq!(
            f[0].bbox,
            FaceBox {
                x: 100,
                y: 200,
                w: 300,
                h: 400
            }
        );
        assert_eq!(f[0].embedding, Embedding(vec![1.0, 2.0, 3.0, 4.0]));
        assert_eq!(s.last_input().unwrap().shape, vec![1, 3, 8, 8]);
    }

    #[test]
    fn face_zero_faces_is_ok_but_wrong_shape_or_dim_fails() {
        let mk = |t: Tensor| {
            ModelFaceAnalyzer::from_verified(
                face_model(4),
                &MockFactory(Arc::new(MockSession::returning(t))),
            )
        };
        assert_eq!(mk(t(&[1, 0, 9], &[])).detect(&thumb()).unwrap(), vec![]);
        // Feil embeddingdimensjon (D=3) mot manifest (D=4).
        assert!(matches!(
            mk(t(&[1, 1, 8], &[0.0; 8])).detect(&thumb()),
            Err(AnalyzeError::Failed(_))
        ));
        assert!(matches!(
            mk(t(&[2, 9], &[0.0; 18])).detect(&thumb()),
            Err(AnalyzeError::Failed(_))
        ));
        let mut nan = [0.0f32; 9];
        nan[6] = f32::NAN;
        assert!(matches!(
            mk(t(&[1, 1, 9], &nan)).detect(&thumb()),
            Err(AnalyzeError::Failed(_))
        ));
        let mut badq = [0.0f32; 9];
        badq[4] = 2.0;
        assert!(matches!(
            mk(t(&[1, 1, 9], &badq)).detect(&thumb()),
            Err(AnalyzeError::Failed(_))
        ));
    }

    #[test]
    fn adapters_are_send_sync_trait_objects() {
        fn _eye(_: &dyn EyeAnalyzer) {}
        fn _face(_: &dyn FaceAnalyzer) {}
        let a = ModelEyeAnalyzer::from_verified(eye_model(), &NoRuntime);
        _eye(&a);
        let f = ModelFaceAnalyzer::from_verified(face_model(4), &NoRuntime);
        _face(&f);
    }
}
