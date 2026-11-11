//! Windows Papirkurv (`cfg(windows)`). ALT HER ER KUN KOMPILERT OG TESTET PÅ WINDOWS I CI-jobben `desktop`;
//! på Linux finnes `delete::FakeRecycleBin` og tester av alt rundt.
//!
//! Grensesnitt: `WindowsRecycleBin` (`impl RecycleBin`). Eneste modul med `unsafe`.
//! Valg og avvik fra ARKITEKTUR 22 (som sier `IFileOperation`): `SHFileOperationW` med
//! `FOF_ALLOWUNDO` er enklere å kalle uten COM og gir samme Papirkurv-semantikk per fil.
//! Sikkerhet mot stille permanent sletting:
//! - Bare faste disker (`DRIVE_FIXED`) forsøkes; nettverk, UNC og flyttbare gir `Unsupported`
//!   (de har ingen Papirkurv, og Windows ville ellers slettet permanent).
//! - `FOF_WANTNUKEWARNINGS` gjør at filer som ikke får plass i Papirkurv gir advarsel i stedet
//!   for stille sletting; avbrutt gir `Cancelled`.
//! Begrensning: `SHFileOperationW` støtter ikke `\\?\`; stier over MAX_PATH gir `Failed`.

#![allow(unsafe_code)]

use crate::delete::{RecycleBin, RecycleError};
use crate::path::windows_volume_root;
use std::os::windows::ffi::OsStrExt;
use std::path::Path;
use windows_sys::Win32::Storage::FileSystem::GetDriveTypeW;
use windows_sys::Win32::UI::Shell::{
    SHFileOperationW, FOF_ALLOWUNDO, FOF_NOCONFIRMATION, FOF_NOERRORUI, FOF_SILENT, FO_DELETE,
    SHFILEOPSTRUCTW,
};

/// Ikke eksportert av windows-sys: `shellapi.h` FOF_WANTNUKEWARNINGS.
const FOF_WANTNUKEWARNINGS: u32 = 0x4000;
const DRIVE_FIXED: u32 = 3;

pub struct WindowsRecycleBin;

fn wide(s: &std::ffi::OsStr) -> Vec<u16> {
    s.encode_wide().chain(std::iter::once(0)).collect()
}

fn has_recycle_bin(path: &Path) -> bool {
    let Some(root) = path.to_str().and_then(windows_volume_root) else {
        return false;
    };
    if root.starts_with(r"\\") {
        return false;
    }
    let w = wide(std::ffi::OsStr::new(&root));
    // SAFETY: `w` er nullterminert og lever gjennom kallet.
    unsafe { GetDriveTypeW(w.as_ptr()) == DRIVE_FIXED }
}

impl RecycleBin for WindowsRecycleBin {
    fn recycle(&self, path: &Path) -> Result<(), RecycleError> {
        if !has_recycle_bin(path) {
            return Err(RecycleError::Unsupported);
        }
        // pFrom er en liste med doble nullbyte på slutten.
        let mut from = wide(path.as_os_str());
        from.push(0);
        let mut op: SHFILEOPSTRUCTW = unsafe { std::mem::zeroed() };
        op.wFunc = FO_DELETE;
        op.pFrom = from.as_ptr();
        op.fFlags = (FOF_ALLOWUNDO
            | FOF_NOCONFIRMATION
            | FOF_NOERRORUI
            | FOF_SILENT
            | FOF_WANTNUKEWARNINGS) as u16;
        // SAFETY: `op` er fullt initialisert; `from` lever gjennom kallet og er dobbelt nullterminert.
        let rc = unsafe { SHFileOperationW(&mut op) };
        let aborted = op.fAnyOperationsAborted != 0;
        if aborted {
            Err(RecycleError::Cancelled)
        } else if rc != 0 {
            Err(RecycleError::Failed(format!("SHFileOperationW: {rc:#x}")))
        } else {
            Ok(())
        }
    }
}
