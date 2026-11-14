//! Koding av `RuleSpec` til og fra teksten i `rule.spec` (stabil og versjonert).
//!
//! Avhenger av: model. Ren funksjon, ingen I/O.
//! Grensesnitt: `encode(&RuleSpec) -> String`, `decode(&str) -> Result<RuleSpec, SpecError>`, `FORMAT_V1`.
//!
//! Format v1: `v1` etterfulgt av ett vilkår per `;`-felt, f.eks. `v1;screenshot;older=30;folder=%2FDCIM`.
//! Verdier med `%`, `;`, `=` eller kontrolltegn skrives som `%XX` (UTF-8-byte i hex). Rekkefølgen bevares.
//! Ukjent versjon eller vilkår er en feil (aldri «tolk som tom»), slik at en nyere regel ikke leses feil.

use crate::model::{RuleCondition, RuleSpec};

/// Versjonsmerke først i `rule.spec`.
pub const FORMAT_V1: &str = "v1";

/// Hvorfor en lagret spesifikasjon ikke kunne leses.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpecError {
    /// Tom tekst eller ukjent versjonsmerke.
    UnknownVersion,
    UnknownCondition,
    /// Manglende eller ugyldig tall.
    BadNumber,
    /// Ugyldig `%XX` eller ugyldig UTF-8 etter avkoding.
    BadEscape,
}

fn escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        if matches!(ch, '%' | ';' | '=') || ch.is_ascii_control() {
            out.push_str(&format!("%{:02X}", ch as u32));
        } else {
            out.push(ch);
        }
    }
    out
}

fn unescape(s: &str) -> Result<String, SpecError> {
    let bytes = s.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            let hex = s.get(i + 1..i + 3).ok_or(SpecError::BadEscape)?;
            out.push(u8::from_str_radix(hex, 16).map_err(|_| SpecError::BadEscape)?);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8(out).map_err(|_| SpecError::BadEscape)
}

/// Koder spesifikasjonen (alltid versjon 1).
pub fn encode(spec: &RuleSpec) -> String {
    let mut out = String::from(FORMAT_V1);
    for c in &spec.all_of {
        out.push(';');
        match c {
            RuleCondition::IsScreenshot => out.push_str("screenshot"),
            RuleCondition::IsBlurry => out.push_str("blurry"),
            RuleCondition::IsExactDuplicate => out.push_str("exact_dup"),
            RuleCondition::OlderThanDays(n) => out.push_str(&format!("older={n}")),
            RuleCondition::LargerThanBytes(n) => out.push_str(&format!("larger={n}")),
            RuleCondition::SmallerThanBytes(n) => out.push_str(&format!("smaller={n}")),
            RuleCondition::InAlbum(n) => out.push_str(&format!("album={n}")),
            RuleCondition::InFolder(p) => out.push_str(&format!("folder={}", escape(p))),
            RuleCondition::Mime(m) => out.push_str(&format!("mime={}", escape(m))),
        }
    }
    out
}

fn number<T: std::str::FromStr>(v: Option<&str>) -> Result<T, SpecError> {
    v.ok_or(SpecError::BadNumber)?
        .parse()
        .map_err(|_| SpecError::BadNumber)
}

/// Leser en kodet spesifikasjon.
pub fn decode(text: &str) -> Result<RuleSpec, SpecError> {
    let mut parts = text.split(';');
    if parts.next() != Some(FORMAT_V1) {
        return Err(SpecError::UnknownVersion);
    }
    let mut all_of = Vec::new();
    for part in parts {
        let (key, value) = match part.split_once('=') {
            Some((k, v)) => (k, Some(v)),
            None => (part, None),
        };
        let cond = match (key, value) {
            ("screenshot", None) => RuleCondition::IsScreenshot,
            ("blurry", None) => RuleCondition::IsBlurry,
            ("exact_dup", None) => RuleCondition::IsExactDuplicate,
            ("older", v) => RuleCondition::OlderThanDays(number(v)?),
            ("larger", v) => RuleCondition::LargerThanBytes(number(v)?),
            ("smaller", v) => RuleCondition::SmallerThanBytes(number(v)?),
            ("album", v) => RuleCondition::InAlbum(number(v)?),
            ("folder", Some(v)) => RuleCondition::InFolder(unescape(v)?),
            ("mime", Some(v)) => RuleCondition::Mime(unescape(v)?),
            _ => return Err(SpecError::UnknownCondition),
        };
        all_of.push(cond);
    }
    Ok(RuleSpec { all_of })
}
