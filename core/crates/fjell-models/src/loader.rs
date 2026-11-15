//! Lasteport: verifiserer manifest, lisens, størrelse og BLAKE3-hash før noe kan kjøres.
//! Rekkefølge (billigst først): parse, formål, lisens, deklarert størrelse, faktisk størrelse (metadata),
//! lesing med tak, hash. Feiler lukket med typede feil.

use crate::error::ModelError;
use crate::manifest::{license_allowed, ModelManifest, Purpose, MAX_MODEL_BYTES};
use std::io::Read;
use std::path::Path;

/// Modell som har passert alle sjekker. Kan bare lages av `load_verified`/`verify_bytes`.
#[derive(Debug)]
pub struct VerifiedModel {
    manifest: ModelManifest,
    bytes: Vec<u8>,
}

impl VerifiedModel {
    pub fn manifest(&self) -> &ModelManifest {
        &self.manifest
    }
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}

/// Verifiserer manifest mot innlastede bytes (ingen filtilgang).
pub fn verify_bytes(manifest: ModelManifest, bytes: Vec<u8>) -> Result<VerifiedModel, ModelError> {
    manifest.validate()?;
    if !license_allowed(&manifest.license) {
        return Err(ModelError::LicenseNotAllowed(manifest.license.clone()));
    }
    let actual = bytes.len() as u64;
    if actual > MAX_MODEL_BYTES {
        return Err(ModelError::TooLarge {
            bytes: actual,
            max: MAX_MODEL_BYTES,
        });
    }
    if actual != manifest.bytes {
        return Err(ModelError::SizeMismatch {
            expected: manifest.bytes,
            actual,
        });
    }
    let hash = blake3::hash(&bytes).to_hex();
    if hash.as_str() != manifest.blake3 {
        return Err(ModelError::HashMismatch);
    }
    Ok(VerifiedModel { manifest, bytes })
}

/// Leser `manifest_path` og modellfilen ved siden av, og verifiserer begge mot `expected`.
pub fn load_verified(manifest_path: &Path, expected: Purpose) -> Result<VerifiedModel, ModelError> {
    let json = std::fs::read_to_string(manifest_path).map_err(|e| ModelError::Io(e.to_string()))?;
    let manifest = ModelManifest::from_json(&json)?;
    if manifest.purpose != expected {
        return Err(ModelError::WrongPurpose);
    }
    if !license_allowed(&manifest.license) {
        return Err(ModelError::LicenseNotAllowed(manifest.license.clone()));
    }
    let dir = manifest_path.parent().unwrap_or_else(|| Path::new("."));
    let model_path = dir.join(&manifest.file);
    let file = std::fs::File::open(&model_path).map_err(|e| ModelError::Io(e.to_string()))?;
    let len = file
        .metadata()
        .map_err(|e| ModelError::Io(e.to_string()))?
        .len();
    if len > MAX_MODEL_BYTES {
        return Err(ModelError::TooLarge {
            bytes: len,
            max: MAX_MODEL_BYTES,
        });
    }
    // Tak på lesingen: filen kan vokse etter metadata-sjekken.
    let mut bytes = Vec::with_capacity(len as usize);
    file.take(MAX_MODEL_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| ModelError::Io(e.to_string()))?;
    verify_bytes(manifest, bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::testutil::{eye_json, face_json};
    use std::fs;

    const DATA: &[u8] = b"not a real model, just bytes";

    fn hex(b: &[u8]) -> String {
        blake3::hash(b).to_hex().to_string()
    }

    fn setup(json: String, data: &[u8]) -> (tempfile::TempDir, std::path::PathBuf) {
        let d = tempfile::tempdir().unwrap();
        fs::write(d.path().join("m.onnx"), data).unwrap();
        fs::write(d.path().join("f.onnx"), data).unwrap();
        let p = d.path().join("manifest.json");
        fs::write(&p, json).unwrap();
        (d, p)
    }

    fn eye(license: &str, bytes: u64, hash: &str) -> String {
        eye_json(hash, bytes, license)
    }

    #[test]
    fn accepts_valid_model() {
        let (_d, p) = setup(eye("MIT", DATA.len() as u64, &hex(DATA)), DATA);
        let m = load_verified(&p, Purpose::Eye).unwrap();
        assert_eq!(m.bytes(), DATA);
        assert_eq!(m.manifest().name, "eye-x");
    }

    #[test]
    fn rejects_hash_mismatch() {
        let (_d, p) = setup(eye("MIT", DATA.len() as u64, &hex(b"other")), DATA);
        assert_eq!(
            load_verified(&p, Purpose::Eye).unwrap_err(),
            ModelError::HashMismatch
        );
    }

    #[test]
    fn rejects_size_mismatch() {
        let (_d, p) = setup(eye("MIT", DATA.len() as u64 + 1, &hex(DATA)), DATA);
        assert!(matches!(
            load_verified(&p, Purpose::Eye).unwrap_err(),
            ModelError::SizeMismatch { .. }
        ));
    }

    #[test]
    fn rejects_declared_oversize_before_reading_file() {
        let (_d, p) = setup(eye("MIT", MAX_MODEL_BYTES + 1, &hex(DATA)), DATA);
        assert!(matches!(
            load_verified(&p, Purpose::Eye).unwrap_err(),
            ModelError::TooLarge { .. }
        ));
    }

    #[test]
    fn rejects_actual_oversize_file() {
        let big = vec![0u8; MAX_MODEL_BYTES as usize + 1];
        let (_d, p) = setup(eye("MIT", 10, &hex(&big)), &big);
        assert!(matches!(
            load_verified(&p, Purpose::Eye).unwrap_err(),
            ModelError::TooLarge { .. }
        ));
    }

    #[test]
    fn rejects_disallowed_license_even_if_hash_ok() {
        for l in ["GPL-3.0-only", "CC-BY-NC-4.0", "Proprietary"] {
            let (_d, p) = setup(eye(l, DATA.len() as u64, &hex(DATA)), DATA);
            assert_eq!(
                load_verified(&p, Purpose::Eye).unwrap_err(),
                ModelError::LicenseNotAllowed(l.to_string())
            );
        }
    }

    #[test]
    fn rejects_wrong_purpose_and_missing_files() {
        let (_d, p) = setup(eye("MIT", DATA.len() as u64, &hex(DATA)), DATA);
        assert_eq!(
            load_verified(&p, Purpose::Face).unwrap_err(),
            ModelError::WrongPurpose
        );
        fs::remove_file(p.parent().unwrap().join("m.onnx")).unwrap();
        assert!(matches!(
            load_verified(&p, Purpose::Eye).unwrap_err(),
            ModelError::Io(_)
        ));
        assert!(matches!(
            load_verified(&p.with_file_name("nope.json"), Purpose::Eye).unwrap_err(),
            ModelError::Io(_)
        ));
    }

    #[test]
    fn verify_bytes_checks_everything() {
        let m =
            ModelManifest::from_json(&face_json(&hex(DATA), DATA.len() as u64, "BSD-3-Clause", 4))
                .unwrap();
        assert!(verify_bytes(m.clone(), DATA.to_vec()).is_ok());
        let mut tampered = DATA.to_vec();
        tampered[0] ^= 1;
        assert_eq!(
            verify_bytes(m, tampered).unwrap_err(),
            ModelError::HashMismatch
        );
    }
}
