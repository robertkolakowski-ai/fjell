//! Stasjonsoppdagelse: type og tilgjengelighet per volum. Klassifiseringen er ren logikk (testbar på
//! Linux); selve Windows-kallene (`GetDriveTypeW`, `GetLogicalDrives`) ligger bak `cfg(windows)`.
//!
//! Grensesnitt: `DriveKind`, `classify_drive_type`, `volume_root_of`, `drive_status_with` (injiserbar),
//! og på Windows `drive_status` og `list_drives`.
//! Regler: frakoblet disk (ingen media, nettverk nede, ukjent rot) gir `DriveStatus::Unavailable`,
//! aldri «tom» eller «slettet». Bare `Fixed` har Papirkurv.

use crate::path::windows_volume_root;
use std::path::Path;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DriveKind {
    Fixed,
    Removable,
    Network,
    Optical,
    Ram,
    Unknown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DriveStatus {
    Online(DriveKind),
    Unavailable,
}

/// `GetDriveTypeW`-verdi til type. 0 (ukjent) og 1 (rot finnes ikke) gir `None`.
pub fn classify_drive_type(t: u32) -> Option<DriveKind> {
    match t {
        2 => Some(DriveKind::Removable),
        3 => Some(DriveKind::Fixed),
        4 => Some(DriveKind::Network),
        5 => Some(DriveKind::Optical),
        6 => Some(DriveKind::Ram),
        _ => None,
    }
}

impl DriveKind {
    /// Papirkurv finnes bare på faste disker.
    pub fn has_recycle_bin(self) -> bool {
        self == DriveKind::Fixed
    }
}

/// Volumrot (`C:\` eller `\\server\share\`) for en Windows-sti; `None` for relative stier.
pub fn volume_root_of(path: &Path) -> Option<String> {
    windows_volume_root(path.to_str()?)
}

/// Status for en rot, med injiserte OS-kall: `drive_type(root)` og `reachable(root)`.
/// Nettverksrot uten svar og flyttbar uten media er `Unavailable`.
pub fn drive_status_with(
    root: &str,
    drive_type: impl Fn(&str) -> u32,
    reachable: impl Fn(&str) -> bool,
) -> DriveStatus {
    let kind = if root.starts_with(r"\\") {
        Some(DriveKind::Network) // UNC rapporteres ikke av GetDriveTypeW på alle systemer
    } else {
        classify_drive_type(drive_type(root))
    };
    match kind {
        Some(k) if reachable(root) => DriveStatus::Online(k),
        _ => DriveStatus::Unavailable,
    }
}

#[cfg(windows)]
#[allow(unsafe_code)] // FFI mot GetDriveTypeW/GetLogicalDrives; ellers forbudt i denne craten
mod sys {
    use super::*;
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::{GetDriveTypeW, GetLogicalDrives};

    fn wide(s: &str) -> Vec<u16> {
        std::ffi::OsStr::new(s)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect()
    }

    pub fn raw_drive_type(root: &str) -> u32 {
        let w = wide(root);
        // SAFETY: `w` er nullterminert og lever gjennom kallet.
        unsafe { GetDriveTypeW(w.as_ptr()) }
    }

    /// Alle stasjonsbokstaver Windows kjenner, med status.
    pub fn list_drives() -> Vec<(String, DriveStatus)> {
        // SAFETY: ingen argumenter.
        let mask = unsafe { GetLogicalDrives() };
        (0..26u8)
            .filter(|i| mask & (1 << i) != 0)
            .map(|i| {
                let root = format!("{}:\\", (b'A' + i) as char);
                let st = drive_status(&root);
                (root, st)
            })
            .collect()
    }

    pub fn drive_status(root: &str) -> DriveStatus {
        drive_status_with(root, raw_drive_type, |r| std::fs::metadata(r).is_ok())
    }
}

#[cfg(windows)]
pub use sys::{drive_status, list_drives, raw_drive_type};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classify() {
        assert_eq!(classify_drive_type(3), Some(DriveKind::Fixed));
        assert_eq!(classify_drive_type(2), Some(DriveKind::Removable));
        assert_eq!(classify_drive_type(4), Some(DriveKind::Network));
        assert_eq!(classify_drive_type(1), None);
        assert_eq!(classify_drive_type(0), None);
        assert!(DriveKind::Fixed.has_recycle_bin());
        assert!(!DriveKind::Removable.has_recycle_bin());
        assert!(!DriveKind::Network.has_recycle_bin());
    }

    #[test]
    fn status_offline_is_unavailable() {
        let up = |_: &str| true;
        let down = |_: &str| false;
        assert_eq!(
            drive_status_with(r"E:\", |_| 2, down),
            DriveStatus::Unavailable
        );
        assert_eq!(
            drive_status_with(r"Z:\", |_| 1, up),
            DriveStatus::Unavailable
        );
        assert_eq!(
            drive_status_with(r"C:\", |_| 3, up),
            DriveStatus::Online(DriveKind::Fixed)
        );
        assert_eq!(
            drive_status_with(r"\\srv\share\", |_| 0, up),
            DriveStatus::Online(DriveKind::Network)
        );
        assert_eq!(
            drive_status_with(r"\\srv\share\", |_| 0, down),
            DriveStatus::Unavailable
        );
    }

    #[test]
    fn volume_root() {
        assert_eq!(
            volume_root_of(Path::new(r"d:\a\b")).as_deref(),
            Some(r"D:\")
        );
        assert_eq!(volume_root_of(Path::new("rel")), None);
    }
}
