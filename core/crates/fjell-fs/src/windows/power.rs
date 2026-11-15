//! Strømstatus på Windows (`cfg(windows)`). Skall i kontraktfasen: grenen som eier filen fyller den
//! (`GetSystemPowerStatus`) og verifiserer den på `windows-latest`. Trait og tester ligger plattformnøytralt i
//! `crate::power`. Til den er fylt ut svarer den `Unknown` (indekseringen pauses aldri på batteri; trygt).
//!
//! Ikke verifisert: hele filen er ikke kompilert eller kjørt (kun Linux-miljø i kontraktfasen).

use crate::power::{PowerSource, PowerStatus};

/// Strømstatus fra Windows.
#[derive(Debug, Default)]
pub struct WindowsPower;

impl PowerSource for WindowsPower {
    fn status(&self) -> PowerStatus {
        PowerStatus::Unknown
    }
}
