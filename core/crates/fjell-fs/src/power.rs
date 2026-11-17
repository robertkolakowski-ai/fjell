//! Strømstatus (plattformnøytral trait). Brukes til «pause på batteri» i indekseringen
//! (docs/v1/ROBUSTHET-KONTRAKT.md). Kjernen (`fjell-core`) kjenner ingen plattform: skallet injiserer en
//! `PowerSource` og kjernen får bare svaret som kode.
//!
//! Grensesnitt: `PowerStatus`, `PowerSource`, `FakePower` (tester, og plattformer uten implementasjon).
//! Windows-implementasjonen ligger i `windows::power` (cfg(windows)); den er ikke verifisert på Linux.

use std::sync::atomic::{AtomicU8, Ordering};

/// Hva PC-en går på nå.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum PowerStatus {
    /// Nettstrøm.
    Ac,
    /// Batteri.
    Battery,
    /// Ukjent eller ingen batteri (stasjonær PC). Behandles som nettstrøm: indekseringen pauses ikke.
    #[default]
    Unknown,
}

impl PowerStatus {
    /// Skal indekseringen pauses når brukeren har valgt «pause på batteri»? Bare ved sikker kunnskap om batteri.
    pub fn is_battery(self) -> bool {
        self == PowerStatus::Battery
    }
}

/// Kilde for strømstatus. Må være billig å kalle (poll hvert sekund eller sjeldnere).
pub trait PowerSource: Send + Sync {
    fn status(&self) -> PowerStatus;
}

/// Tolker `GetSystemPowerStatus` (`ACLineStatus`: 0 = frakoblet, 1 = tilkoblet, 255 = ukjent; `BatteryFlag`: 128 =
/// ingen batteri i maskinen, 255 = ukjent). Ren funksjon, testet på Linux; selve Win32-kallet er det ikke.
/// Bare sikker kunnskap om batteri gir `Battery`: stasjonær PC og ukjent gir aldri pause.
pub fn classify_windows(ac_line_status: u8, battery_flag: u8) -> PowerStatus {
    if battery_flag == 128 {
        return PowerStatus::Unknown;
    }
    match ac_line_status {
        0 => PowerStatus::Battery,
        1 => PowerStatus::Ac,
        _ => PowerStatus::Unknown,
    }
}

/// Senker prioriteten til tråden som kaller (CPU, disk og minne), slik at indekseringen ikke tar fra annet arbeid.
/// Kalles først på hver indekseringstråd. Windows: `windows::power::WindowsBackgroundPriority`
/// (ikke verifisert på Windows). Andre plattformer: `NoPriority` (ingen virkning; kontrakten, åpent spørsmål 7).
pub trait BackgroundPriority: Send + Sync {
    fn enter_current_thread(&self);
}

/// Ingen endring av trådprioritet (Linux/macOS og tester).
#[derive(Debug, Default)]
pub struct NoPriority;

impl BackgroundPriority for NoPriority {
    fn enter_current_thread(&self) {}
}

/// Teller kall (tester).
#[derive(Debug, Default)]
pub struct FakePriority(std::sync::atomic::AtomicUsize);

impl FakePriority {
    pub fn calls(&self) -> usize {
        self.0.load(Ordering::SeqCst)
    }
}

impl BackgroundPriority for FakePriority {
    fn enter_current_thread(&self) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}

/// Fast eller styrbar strømstatus for tester og plattformer uten implementasjon.
#[derive(Debug, Default)]
pub struct FakePower(AtomicU8);

impl FakePower {
    pub fn new(status: PowerStatus) -> FakePower {
        let p = FakePower::default();
        p.set(status);
        p
    }

    pub fn set(&self, status: PowerStatus) {
        let v = match status {
            PowerStatus::Ac => 0,
            PowerStatus::Battery => 1,
            PowerStatus::Unknown => 2,
        };
        self.0.store(v, Ordering::SeqCst);
    }
}

impl PowerSource for FakePower {
    fn status(&self) -> PowerStatus {
        match self.0.load(Ordering::SeqCst) {
            0 => PowerStatus::Ac,
            1 => PowerStatus::Battery,
            _ => PowerStatus::Unknown,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn windows_status_is_only_battery_when_certain() {
        assert_eq!(classify_windows(0, 1), PowerStatus::Battery);
        assert_eq!(classify_windows(0, 0), PowerStatus::Battery);
        assert_eq!(classify_windows(1, 8), PowerStatus::Ac);
        assert_eq!(classify_windows(1, 0), PowerStatus::Ac);
        // Stasjonær PC (ingen batteri) og ukjent pauser aldri, selv om ACLineStatus skulle si frakoblet.
        assert_eq!(classify_windows(0, 128), PowerStatus::Unknown);
        assert_eq!(classify_windows(255, 255), PowerStatus::Unknown);
        assert_eq!(classify_windows(2, 0), PowerStatus::Unknown);
        assert!(!classify_windows(255, 0).is_battery());
    }

    #[test]
    fn priority_hooks_are_plain_traits() {
        let p = FakePriority::default();
        let h: &dyn BackgroundPriority = &p;
        h.enter_current_thread();
        h.enter_current_thread();
        assert_eq!(p.calls(), 2);
        NoPriority.enter_current_thread();
    }

    #[test]
    fn fake_power_follows_set_and_unknown_is_not_battery() {
        let p = FakePower::new(PowerStatus::Battery);
        assert!(p.status().is_battery());
        p.set(PowerStatus::Ac);
        assert_eq!(p.status(), PowerStatus::Ac);
        p.set(PowerStatus::Unknown);
        assert!(!p.status().is_battery());
        assert_eq!(FakePower::default().status(), PowerStatus::Ac);
    }
}
