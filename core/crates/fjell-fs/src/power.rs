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
