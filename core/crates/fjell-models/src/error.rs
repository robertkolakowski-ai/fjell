//! Typede feil for modellrammen. Alt feiler lukket: en feil betyr aldri «ingen funn».

use std::fmt;

/// Hvorfor en modell ikke kunne tas i bruk eller kjøres.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ModelError {
    /// Manifest- eller modellfil finnes ikke / kan ikke leses.
    Io(String),
    /// Manifestet er ikke gyldig JSON eller mangler felt.
    ManifestParse(String),
    /// Manifestet er gyldig JSON, men innholdet er ugyldig (f.eks. feil form, ugyldig hash-tekst).
    ManifestInvalid(String),
    /// Manifestets formål stemmer ikke med adapteren (øye vs. ansikt).
    WrongPurpose,
    /// Lisensen står ikke på tillatt-listen.
    LicenseNotAllowed(String),
    /// Størrelsen (deklarert eller faktisk) er over grensen.
    TooLarge { bytes: u64, max: u64 },
    /// Faktisk størrelse avviker fra manifestet.
    SizeMismatch { expected: u64, actual: u64 },
    /// BLAKE3-hash avviker fra manifestet.
    HashMismatch,
    /// Ingen kjøremotor er koblet inn (feature `onnx` av, eller ingen fabrikk).
    RuntimeUnavailable,
    /// Kjøremotoren kunne ikke åpne modellen.
    SessionOpen(String),
    /// Modellens utdata har feil form for kontrakten.
    BadOutputShape {
        expected: Vec<usize>,
        actual: Vec<usize>,
    },
    /// Utdata inneholder NaN/uendelig eller verdier utenfor gyldig område.
    BadOutputValue(String),
    /// Kjøringen feilet.
    Run(String),
}

impl fmt::Display for ModelError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(m) => write!(f, "model io error: {m}"),
            Self::ManifestParse(m) => write!(f, "manifest parse error: {m}"),
            Self::ManifestInvalid(m) => write!(f, "manifest invalid: {m}"),
            Self::WrongPurpose => write!(f, "model purpose does not match analyzer"),
            Self::LicenseNotAllowed(l) => write!(f, "license not allowed: {l}"),
            Self::TooLarge { bytes, max } => write!(f, "model too large: {bytes} > {max}"),
            Self::SizeMismatch { expected, actual } => {
                write!(f, "model size mismatch: expected {expected}, got {actual}")
            }
            Self::HashMismatch => write!(f, "model hash mismatch"),
            Self::RuntimeUnavailable => write!(f, "no inference runtime available"),
            Self::SessionOpen(m) => write!(f, "cannot open model session: {m}"),
            Self::BadOutputShape { expected, actual } => {
                write!(f, "bad output shape: expected {expected:?}, got {actual:?}")
            }
            Self::BadOutputValue(m) => write!(f, "bad output value: {m}"),
            Self::Run(m) => write!(f, "model run failed: {m}"),
        }
    }
}

impl std::error::Error for ModelError {}
