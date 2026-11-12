//! Stihåndtering: lange stier (`\\?\`), UNC og volumrot. Ren strenglogikk etter Windows-regler,
//! slik at alt kan testes på Linux.
//!
//! Grensesnitt:
//! - `extended_str`: Windows-sti som streng til `\\?\`-form (UNC blir `\\?\UNC\server\share\..`).
//! - `strip_extended`: tilbake til visningsform.
//! - `windows_volume_root`: `C:\` eller `\\server\share\` for en Windows-sti.
//! - `os_path`: sti til bruk mot OS-et (utvidet på Windows, uendret ellers).
//! - `resolve`: `SourceId` (relativ, `/`) til sti under roten; avviser `..`, tomme ledd og absolutte.

use fjell_core::model::{SourceError, SourceId};
use std::path::{Path, PathBuf};

/// Over denne lengden (tegn) trenger Windows `\\?\` (MAX_PATH er 260, mapper 248).
pub const LONG_PATH_LIMIT: usize = 240;

/// Gjør en absolutt Windows-sti om til `\\?\`-form. `force` utvider uansett lengde.
/// Relative stier og stier som allerede er utvidet returneres uendret (`\\?\` kan ikke brukes på
/// relative stier). Skråstreker normaliseres til `\`, siden `\\?\` ikke gjør det selv.
pub fn extended_str(path: &str, force: bool) -> String {
    if path.starts_with(r"\\?\") || path.starts_with(r"\\.\") {
        return path.to_string();
    }
    if !force && path.len() < LONG_PATH_LIMIT {
        return path.to_string();
    }
    let p = path.replace('/', r"\");
    if let Some(unc) = p.strip_prefix(r"\\") {
        return format!(r"\\?\UNC\{unc}");
    }
    let b = p.as_bytes();
    if b.len() >= 3 && b[0].is_ascii_alphabetic() && b[1] == b':' && b[2] == b'\\' {
        return format!(r"\\?\{p}");
    }
    path.to_string()
}

/// Motsatt av `extended_str`, til visning og til `source_id`.
pub fn strip_extended(path: &str) -> String {
    if let Some(rest) = path.strip_prefix(r"\\?\UNC\") {
        format!(r"\\{rest}")
    } else if let Some(rest) = path.strip_prefix(r"\\?\") {
        rest.to_string()
    } else {
        path.to_string()
    }
}

/// Volumrot for en Windows-sti: `C:\` eller `\\server\share\`. `None` for relative stier.
pub fn windows_volume_root(path: &str) -> Option<String> {
    let p = strip_extended(path).replace('/', r"\");
    if let Some(unc) = p.strip_prefix(r"\\") {
        let mut parts = unc.split('\\').filter(|s| !s.is_empty());
        let (server, share) = (parts.next()?, parts.next()?);
        return Some(format!(r"\\{server}\{share}\"));
    }
    let b = p.as_bytes();
    if b.len() >= 2 && b[0].is_ascii_alphabetic() && b[1] == b':' {
        return Some(format!("{}:\\", p[..1].to_ascii_uppercase()));
    }
    None
}

/// Sti til bruk mot OS-et: utvidet ved behov på Windows, ellers uendret.
pub fn os_path(p: &Path) -> PathBuf {
    #[cfg(windows)]
    {
        if let Some(s) = p.to_str() {
            return PathBuf::from(extended_str(s, false));
        }
    }
    p.to_path_buf()
}

/// `SourceId` til sti under `root`. Avviser alt som kan forlate roten.
pub fn resolve(root: &Path, id: &SourceId) -> Result<PathBuf, SourceError> {
    if id.0.split('/').any(|c| c == ".." || c.is_empty()) || id.0.starts_with('/') {
        return Err(SourceError::NotFound);
    }
    Ok(root.join(&id.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_paths_untouched_long_paths_extended() {
        assert_eq!(extended_str(r"C:\a\b.jpg", false), r"C:\a\b.jpg");
        assert_eq!(extended_str(r"C:\a\b.jpg", true), r"\\?\C:\a\b.jpg");
        let long = format!(r"C:\{}\x.jpg", "d".repeat(260));
        assert!(extended_str(&long, false).starts_with(r"\\?\C:\"));
    }

    #[test]
    fn unc_and_slashes() {
        assert_eq!(
            extended_str(r"\\srv\share\a\b.jpg", true),
            r"\\?\UNC\srv\share\a\b.jpg"
        );
        assert_eq!(extended_str("C:/a/b.jpg", true), r"\\?\C:\a\b.jpg");
        // Allerede utvidet og relativ: uendret.
        assert_eq!(extended_str(r"\\?\C:\a", true), r"\\?\C:\a");
        assert_eq!(extended_str(r"a\b", true), r"a\b");
    }

    #[test]
    fn strip_roundtrip() {
        for p in [r"C:\a\b.jpg", r"\\srv\share\a.jpg"] {
            assert_eq!(strip_extended(&extended_str(p, true)), p);
        }
    }

    #[test]
    fn volume_roots() {
        assert_eq!(windows_volume_root(r"c:\x\y").as_deref(), Some(r"C:\"));
        assert_eq!(
            windows_volume_root(r"\\?\UNC\srv\share\x").as_deref(),
            Some(r"\\srv\share\")
        );
        assert_eq!(windows_volume_root(r"\\srv").as_deref(), None);
        assert_eq!(windows_volume_root("rel/x").as_deref(), None);
    }

    #[test]
    fn resolve_rejects_escapes() {
        let r = Path::new("/lib");
        for bad in ["../x", "a/../b", "/abs", "a//b", ""] {
            assert!(resolve(r, &SourceId(bad.into())).is_err(), "{bad}");
        }
        assert_eq!(
            resolve(r, &SourceId("a/b.jpg".into())).unwrap(),
            Path::new("/lib/a/b.jpg")
        );
    }
}
