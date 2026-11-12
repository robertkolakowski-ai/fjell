//! Windows Papirkurv (`cfg(windows)`). Kompileres og testes på Windows i CI-jobben `desktop`;
//! på Linux finnes `delete::FakeRecycleBin` og tester av alt rundt. Eneste modul med `unsafe`.
//!
//! Valg (ARKITEKTUR 22): `IFileOperation` via `windows`-craten. Den erstatter `SHFileOperationW`,
//! som ikke tar `\\?\` og dermed ga `Failed` for stier over MAX_PATH. Stien gjøres alltid om til
//! `\\?\`-form før `SHCreateItemFromParsingName`.
//! Sikkerhet mot stille permanent sletting:
//! - Bare faste disker (`DRIVE_FIXED`) forsøkes; nettverk, UNC og flyttbare gir `Unsupported`.
//! - `FOFX_RECYCLEONDELETE` får operasjonen til å feile når filen ikke kan til Papirkurv, i stedet
//!   for å slette permanent. `FOF_WANTNUKEWARNINGS` er satt som ekstra sikring.
//! - Etter kallet sjekkes at filen faktisk er borte fra opprinnelig sted; ellers `Failed`.
//! - Ingen fallback til `remove_file` her. Permanent går bare via `FsDeleter` med `PermanentAck`.
//!
//! Ikke verifisert på Linux: hele modulen. Se `tests/windows.rs` (kjøres på windows-latest).

#![allow(unsafe_code)]

use crate::delete::{RecycleBin, RecycleError};
use crate::drives::{drive_status, volume_root_of, DriveStatus};
use crate::path::extended_str;
use std::path::Path;
use windows::core::HSTRING;
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CoUninitialize, IBindCtx, CLSCTX_ALL,
    COINIT_APARTMENTTHREADED,
};
use windows::Win32::UI::Shell::{
    FileOperation, IFileOperation, IFileOperationProgressSink, IShellItem,
    SHCreateItemFromParsingName, FILEOPERATION_FLAGS,
};

// shellapi.h / shobjidl_core.h; rå verdi for å slippe forskjeller i konstanttype mellom crate-versjoner.
const FOF_SILENT: u32 = 0x0004;
const FOF_NOCONFIRMATION: u32 = 0x0010;
const FOF_ALLOWUNDO: u32 = 0x0040;
const FOF_NOERRORUI: u32 = 0x0400;
const FOF_WANTNUKEWARNINGS: u32 = 0x4000;
const FOFX_RECYCLEONDELETE: u32 = 0x0008_0000;

pub struct WindowsRecycleBin;

struct ComGuard(bool);
impl Drop for ComGuard {
    fn drop(&mut self) {
        if self.0 {
            // SAFETY: paret med vellykket `CoInitializeEx` på samme tråd.
            unsafe { CoUninitialize() };
        }
    }
}

fn fail(what: &str, e: windows::core::Error) -> RecycleError {
    RecycleError::Failed(format!("{what}: {:#x}", e.code().0))
}

fn recycle_item(path: &str) -> Result<(), RecycleError> {
    // Allerede initialisert med annen modell gir RPC_E_CHANGED_MODE; COM er da brukbart likevel.
    // SAFETY: standard COM-oppstart; guard avslutter på samme tråd.
    let _com = ComGuard(unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) }.is_ok());
    let ext = HSTRING::from(extended_str(path, true));
    // SAFETY: COM er initialisert; `ext` lever gjennom kallene; grensesnittene slippes før `_com`.
    unsafe {
        let op: IFileOperation =
            CoCreateInstance(&FileOperation, None, CLSCTX_ALL).map_err(|e| fail("CoCreate", e))?;
        op.SetOperationFlags(FILEOPERATION_FLAGS(
            FOF_ALLOWUNDO
                | FOF_NOCONFIRMATION
                | FOF_NOERRORUI
                | FOF_SILENT
                | FOF_WANTNUKEWARNINGS
                | FOFX_RECYCLEONDELETE,
        ))
        .map_err(|e| fail("SetOperationFlags", e))?;
        let item: IShellItem = SHCreateItemFromParsingName(&ext, None::<&IBindCtx>)
            .map_err(|e| fail("SHCreateItemFromParsingName", e))?;
        op.DeleteItem(&item, None::<&IFileOperationProgressSink>)
            .map_err(|e| fail("DeleteItem", e))?;
        op.PerformOperations()
            .map_err(|e| fail("PerformOperations", e))?;
        if op
            .GetAnyOperationsAborted()
            .map_err(|e| fail("Aborted", e))?
            .as_bool()
        {
            return Err(RecycleError::Cancelled);
        }
    }
    Ok(())
}

impl RecycleBin for WindowsRecycleBin {
    fn recycle(&self, path: &Path) -> Result<(), RecycleError> {
        let root = volume_root_of(path).ok_or(RecycleError::Unsupported)?;
        match drive_status(&root) {
            DriveStatus::Online(k) if k.has_recycle_bin() => {}
            DriveStatus::Online(_) => return Err(RecycleError::Unsupported),
            DriveStatus::Unavailable => {
                return Err(RecycleError::Failed("volumet er frakoblet".into()))
            }
        }
        let s = path.to_str().ok_or(RecycleError::Unsupported)?;
        recycle_item(s)?;
        // Aldri meld Deleted hvis filen fortsatt ligger der.
        if std::fs::symlink_metadata(crate::path::os_path(path)).is_ok() {
            return Err(RecycleError::Failed("filen ligger fortsatt igjen".into()));
        }
        Ok(())
    }
}
