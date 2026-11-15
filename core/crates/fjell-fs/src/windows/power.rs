//! Strømstatus og bakgrunnsprioritet på Windows (`cfg(windows)`). Del av indekseringen
//! (docs/v1/INDEKSERING.md). Trait og tester ligger plattformnøytralt i `crate::power`; denne filen er bare det
//! tynne laget mot Win32.
//!
//! - `WindowsPower`: `GetSystemPowerStatus` (kernel32). Selve tolkningen (`classify_windows`) er ren og testet på
//!   Linux; kallet er ikke det.
//! - `WindowsBackgroundPriority`: `SetThreadPriority(THREAD_MODE_BACKGROUND_BEGIN)` på tråden som kaller. Det
//!   senker CPU-, disk- og minneprioriteten til akkurat denne tråden, så indekseringen ikke tar fra brukerens
//!   andre arbeid. Kalles først på hver indekseringstråd (rayon `start_handler`); tråden dør med poolen.
//!
//! **Ikke verifisert på Windows:** hele filen er skrevet uten å kunne kompileres eller kjøres her (kun Linux-miljø).
//! Verifiseres av CI-jobben på `windows-latest`, og manuelt på en bærbar PC (koble fra laderen under indeksering,
//! se prioriteten til trådene i Ressursmonitor). Feil fra Win32 gir `Unknown` / ingen endring (aldri pause, aldri panikk).

use crate::power::{classify_windows, BackgroundPriority, PowerSource, PowerStatus};
use windows_sys::Win32::System::Power::{GetSystemPowerStatus, SYSTEM_POWER_STATUS};
use windows_sys::Win32::System::Threading::{
    GetCurrentThread, SetThreadPriority, THREAD_MODE_BACKGROUND_BEGIN,
};

/// Strømstatus fra Windows.
#[derive(Debug, Default)]
pub struct WindowsPower;

impl PowerSource for WindowsPower {
    fn status(&self) -> PowerStatus {
        let mut s = SYSTEM_POWER_STATUS {
            ACLineStatus: 255,
            BatteryFlag: 255,
            BatteryLifePercent: 255,
            SystemStatusFlag: 0,
            BatteryLifeTime: 0,
            BatteryFullLifeTime: 0,
        };
        // SAFETY: `s` er en gyldig, utpekt `SYSTEM_POWER_STATUS` som Windows skriver til.
        let ok = unsafe { GetSystemPowerStatus(&mut s) };
        if ok == 0 {
            return PowerStatus::Unknown;
        }
        classify_windows(s.ACLineStatus, s.BatteryFlag)
    }
}

/// Lav bakgrunnsprioritet for tråden som kaller.
#[derive(Debug, Default)]
pub struct WindowsBackgroundPriority;

impl BackgroundPriority for WindowsBackgroundPriority {
    fn enter_current_thread(&self) {
        // SAFETY: `GetCurrentThread` gir en pseudohåndtak som alltid er gyldig for kallende tråd.
        // Feil (returnerer 0) ignoreres bevisst: prioriteten er en høflighet, ikke et krav.
        let _ = unsafe { SetThreadPriority(GetCurrentThread(), THREAD_MODE_BACKGROUND_BEGIN) };
    }
}
