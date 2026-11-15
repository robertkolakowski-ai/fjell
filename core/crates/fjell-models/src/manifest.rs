//! `ModelManifest`: beskrivelse av en modell, lest fra lokal JSON. Kilde-URL er bare metadata
//! (ingenting hentes). Parsing validerer form og felt; verifisering av selve filen skjer i `loader`.

use crate::error::ModelError;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Øvre grense for modellfil (byte). Modeller følger med installasjonsprogrammet.
pub const MAX_MODEL_BYTES: u64 = 30 * 1024 * 1024;

/// Permissive lisenser (eksakt SPDX-id, ingen uttrykk som «A OR B»).
pub const ALLOWED_LICENSES: [&str; 6] = [
    "Apache-2.0",
    "MIT",
    "BSD-2-Clause",
    "BSD-3-Clause",
    "0BSD",
    "CC0-1.0",
];

/// Er lisensen på tillatt-listen?
pub fn license_allowed(spdx: &str) -> bool {
    ALLOWED_LICENSES.contains(&spdx)
}

/// Hva modellen brukes til.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Purpose {
    Eye,
    Face,
}

/// Terskelnavn for øyemodell: under denne sikkerheten svarer adapteren `Unavailable`.
pub const EYE_MIN_CONFIDENCE: &str = "eye_min_confidence";
/// Terskelnavn for ansiktsmodell: ansikter med lavere kvalitet utelates.
pub const FACE_MIN_QUALITY: &str = "face_min_quality";

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModelManifest {
    pub name: String,
    pub purpose: Purpose,
    pub version: String,
    /// BLAKE3 over modellfilen, 64 små hex-tegn.
    pub blake3: String,
    pub bytes: u64,
    /// SPDX-id.
    pub license: String,
    /// Bare metadata. Appen kontakter aldri denne.
    #[serde(default)]
    pub source_url: Option<String>,
    /// Filnavn ved siden av manifestet (ingen stier).
    pub file: String,
    /// `[1, kanaler, høyde, bredde]`, kanaler er 1 eller 3.
    pub input_shape: Vec<usize>,
    /// Forventet utdataform; 0 betyr dynamisk dimensjon.
    pub output_shape: Vec<usize>,
    /// Embeddingdimensjon (ansikt: > 0, øye: 0).
    #[serde(default)]
    pub embedding_dim: usize,
    /// Normalisering av 0..1-verdier: `(v - mean) / std`.
    #[serde(default)]
    pub input_mean: f32,
    #[serde(default = "one")]
    pub input_std: f32,
    /// Kalibrerte terskler (se `EYE_MIN_CONFIDENCE`, `FACE_MIN_QUALITY`).
    #[serde(default)]
    pub thresholds: BTreeMap<String, f32>,
}

fn one() -> f32 {
    1.0
}

fn invalid(m: &str) -> ModelError {
    ModelError::ManifestInvalid(m.to_string())
}

impl ModelManifest {
    /// Parser og validerer. Feiler lukket.
    pub fn from_json(json: &str) -> Result<ModelManifest, ModelError> {
        let m: ModelManifest =
            serde_json::from_str(json).map_err(|e| ModelError::ManifestParse(e.to_string()))?;
        m.validate()?;
        Ok(m)
    }

    /// Strukturell validering (uten tilgang til modellfilen).
    pub fn validate(&self) -> Result<(), ModelError> {
        if self.name.trim().is_empty() || self.version.trim().is_empty() {
            return Err(invalid("name and version must be non-empty"));
        }
        if self.blake3.len() != 64
            || !self
                .blake3
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return Err(invalid("blake3 must be 64 lowercase hex characters"));
        }
        if self.bytes == 0 {
            return Err(invalid("bytes must be > 0"));
        }
        if self.bytes > MAX_MODEL_BYTES {
            return Err(ModelError::TooLarge {
                bytes: self.bytes,
                max: MAX_MODEL_BYTES,
            });
        }
        if self.file.is_empty()
            || self.file.contains(['/', '\\', ':'])
            || self.file == ".."
            || self.file == "."
        {
            return Err(invalid("file must be a plain file name"));
        }
        match self.input_shape.as_slice() {
            [1, c, h, w] if (*c == 1 || *c == 3) && *h > 0 && *w > 0 => {}
            _ => return Err(invalid("input_shape must be [1, 1|3, h, w] with h, w > 0")),
        }
        if self.output_shape.is_empty() {
            return Err(invalid("output_shape must not be empty"));
        }
        if !self.input_mean.is_finite() || !self.input_std.is_finite() || self.input_std <= 0.0 {
            return Err(invalid("input_mean/input_std must be finite and std > 0"));
        }
        if self.thresholds.values().any(|v| !v.is_finite()) {
            return Err(invalid("thresholds must be finite"));
        }
        match self.purpose {
            Purpose::Eye => {
                if self.embedding_dim != 0 {
                    return Err(invalid("eye model must have embedding_dim 0"));
                }
                if self.output_shape != [1, 3] {
                    return Err(invalid("eye output_shape must be [1, 3]"));
                }
                self.threshold_in_unit(EYE_MIN_CONFIDENCE)?;
            }
            Purpose::Face => {
                if self.embedding_dim == 0 {
                    return Err(invalid("face model needs embedding_dim > 0"));
                }
                if self.output_shape != [1, 0, 5 + self.embedding_dim] {
                    return Err(invalid(
                        "face output_shape must be [1, 0, 5 + embedding_dim]",
                    ));
                }
                self.threshold_in_unit(FACE_MIN_QUALITY)?;
            }
        }
        Ok(())
    }

    fn threshold_in_unit(&self, key: &str) -> Result<f32, ModelError> {
        match self.thresholds.get(key) {
            Some(v) if (0.0..=1.0).contains(v) => Ok(*v),
            Some(_) => Err(invalid("threshold must be within 0..=1")),
            None => Err(ModelError::ManifestInvalid(format!(
                "missing calibrated threshold `{key}`"
            ))),
        }
    }

    pub fn threshold(&self, key: &str) -> Option<f32> {
        self.thresholds.get(key).copied()
    }
}

#[cfg(test)]
pub(crate) mod testutil {
    pub fn eye_json(hash: &str, bytes: u64, license: &str) -> String {
        format!(
            r#"{{"name":"eye-x","purpose":"eye","version":"1","blake3":"{hash}","bytes":{bytes},
"license":"{license}","source_url":"https://example.invalid/m","file":"m.onnx",
"input_shape":[1,3,8,8],"output_shape":[1,3],"thresholds":{{"eye_min_confidence":0.6}}}}"#
        )
    }

    pub fn face_json(hash: &str, bytes: u64, license: &str, dim: usize) -> String {
        let out = 5 + dim;
        format!(
            r#"{{"name":"face-x","purpose":"face","version":"1","blake3":"{hash}","bytes":{bytes},
"license":"{license}","file":"f.onnx","input_shape":[1,3,8,8],"output_shape":[1,0,{out}],
"embedding_dim":{dim},"input_mean":0.5,"input_std":0.5,"thresholds":{{"face_min_quality":0.5}}}}"#
        )
    }

    pub const H: &str = "0000000000000000000000000000000000000000000000000000000000000000";
}

#[cfg(test)]
mod tests {
    use super::testutil::*;
    use super::*;

    fn bad(j: String) -> bool {
        matches!(
            ModelManifest::from_json(&j),
            Err(ModelError::ManifestInvalid(_))
        )
    }

    #[test]
    fn parses_eye_and_face() {
        let e = ModelManifest::from_json(&eye_json(H, 10, "MIT")).unwrap();
        assert_eq!(e.purpose, Purpose::Eye);
        assert_eq!(e.threshold(EYE_MIN_CONFIDENCE), Some(0.6));
        assert_eq!(e.source_url.as_deref(), Some("https://example.invalid/m"));
        let f = ModelManifest::from_json(&face_json(H, 10, "Apache-2.0", 4)).unwrap();
        assert_eq!(f.embedding_dim, 4);
        assert_eq!(f.input_std, 0.5);
    }

    #[test]
    fn rejects_garbage_and_unknown_fields() {
        assert!(matches!(
            ModelManifest::from_json("{"),
            Err(ModelError::ManifestParse(_))
        ));
        let j = eye_json(H, 10, "MIT").replace("\"name\"", "\"extra\":1,\"name\"");
        assert!(matches!(
            ModelManifest::from_json(&j),
            Err(ModelError::ManifestParse(_))
        ));
    }

    #[test]
    fn rejects_bad_hash_size_file_shape() {
        assert!(bad(eye_json("abc", 10, "MIT")));
        assert!(bad(eye_json(&"A".repeat(64), 10, "MIT")));
        assert!(bad(eye_json(H, 0, "MIT")));
        assert!(bad(eye_json(H, 10, "MIT").replace("m.onnx", "../m.onnx")));
        assert!(bad(eye_json(H, 10, "MIT").replace("[1,3,8,8]", "[1,4,8,8]")));
        assert!(bad(eye_json(H, 10, "MIT").replace("[1,3]", "[1,2]")));
        assert!(bad(eye_json(H, 10, "MIT").replace("0.6", "1.5")));
        assert!(bad(
            eye_json(H, 10, "MIT").replace("eye_min_confidence", "other")
        ));
        assert!(bad(face_json(H, 10, "MIT", 0)));
        assert!(bad(
            face_json(H, 10, "MIT", 4).replace("\"embedding_dim\":4", "\"embedding_dim\":5")
        ));
    }

    #[test]
    fn rejects_declared_size_over_limit() {
        assert!(matches!(
            ModelManifest::from_json(&eye_json(H, MAX_MODEL_BYTES + 1, "MIT")),
            Err(ModelError::TooLarge { .. })
        ));
        assert!(ModelManifest::from_json(&eye_json(H, MAX_MODEL_BYTES, "MIT")).is_ok());
    }

    #[test]
    fn license_allow_list_is_exact() {
        for l in ["Apache-2.0", "MIT", "BSD-3-Clause", "CC0-1.0"] {
            assert!(license_allowed(l));
        }
        for l in [
            "GPL-3.0-only",
            "CC-BY-NC-4.0",
            "mit",
            "MIT OR GPL-3.0-only",
            "",
            "Proprietary",
        ] {
            assert!(!license_allowed(l), "{l}");
        }
    }
}
