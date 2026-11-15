//! Indeksering (status, pause, fortsett, avbryt, innstillinger) for skallet (`impl Shell`). Del av `fjell-shell`.
//! Skall i kontraktfasen: tynne delegasjoner til `FjellApi`; grenen som eier filen utvider dem
//! (docs/v1/ROBUSTHET-KONTRAKT.md). Tauri-kommandoene (`indexing_status`, `indexing_pause`, `indexing_resume`,
//! `indexing_cancel`, `get_indexing_settings`, `set_indexing_settings`) er allerede registrert i `fjell-desktop`.
//!
//! Avhenger av: `fjell-core::api` (`FjellApi`), `dto_robust`, `dto`. Strømstatus (pause på batteri) injiseres
//! fra `fjell-fs::power`; skallet kobler den, kjernen kjenner ingen plattform.

use crate::dto::ErrorDto;
use crate::dto_robust::{IndexingSettingsDto, IndexingStatusDto};
use crate::Shell;
use fjell_core::api::Fjell;
use fjell_core::pipeline::PowerProbe;
use fjell_fs::power::{BackgroundPriority, PowerSource};
use std::sync::Arc;

/// Gjør `fjell-fs::power::PowerSource` om til kjernens `PowerProbe` (kode: går PC-en på batteri?).
/// Bare sikker kunnskap om batteri teller (`PowerStatus::is_battery`); ukjent og stasjonær PC pauser aldri.
pub struct PowerAdapter(pub Arc<dyn PowerSource>);

impl PowerProbe for PowerAdapter {
    fn on_battery(&self) -> bool {
        self.0.status().is_battery()
    }
}

/// Kobler strømstatus og trådprioritet til kjernen. Kjernen kjenner ingen plattform: den får bare koden
/// (`on_battery`) og en krok som kjøres først på hver indekseringstråd (ved lav prioritet).
pub fn wire_platform(
    fjell: &Fjell,
    power: Arc<dyn PowerSource>,
    priority: Arc<dyn BackgroundPriority>,
) {
    fjell.set_power_probe(Arc::new(PowerAdapter(power)));
    fjell.set_worker_hook(Arc::new(move || priority.enter_current_thread()));
}

/// Plattformens standardkobling. Windows: `GetSystemPowerStatus` og bakgrunnsprioritet på trådene
/// (**ikke verifisert på Windows**, se `fjell-fs/src/windows/power.rs`). Andre plattformer: ukjent strøm
/// (pauser aldri) og ingen prioritetsendring.
pub fn wire_default_platform(fjell: &Fjell) {
    #[cfg(windows)]
    wire_platform(
        fjell,
        Arc::new(fjell_fs::windows::power::WindowsPower),
        Arc::new(fjell_fs::windows::power::WindowsBackgroundPriority),
    );
    #[cfg(not(windows))]
    wire_platform(
        fjell,
        Arc::new(fjell_fs::power::FakePower::default()),
        Arc::new(fjell_fs::power::NoPriority),
    );
}

impl Shell {
    pub fn indexing_status(&self) -> Result<IndexingStatusDto, ErrorDto> {
        Ok(IndexingStatusDto::from(&self.api()?.indexing_status()?))
    }

    pub fn indexing_pause(&self) -> Result<(), ErrorDto> {
        self.api()?.indexing_pause().map_err(ErrorDto::from)
    }

    pub fn indexing_resume(&self) -> Result<(), ErrorDto> {
        self.api()?.indexing_resume().map_err(ErrorDto::from)
    }

    pub fn indexing_cancel(&self) -> Result<(), ErrorDto> {
        self.api()?.indexing_cancel().map_err(ErrorDto::from)
    }

    pub fn indexing_settings(&self) -> Result<IndexingSettingsDto, ErrorDto> {
        Ok(IndexingSettingsDto::from(&self.api()?.indexing_settings()?))
    }

    /// Lagrer innstillingene. Ukjent prioritetskode gir `indexing_invalid_settings`.
    pub fn set_indexing_settings(
        &self,
        s: &IndexingSettingsDto,
    ) -> Result<IndexingSettingsDto, ErrorDto> {
        let core = s
            .to_core()
            .ok_or_else(|| ErrorDto::new("core_error", "indexing_invalid_settings"))?;
        let api = self.api()?;
        api.indexing_settings_set(core)?;
        Ok(IndexingSettingsDto::from(&api.indexing_settings()?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fjell_core::model::FakeClock;
    use fjell_core::store::Store;
    use fjell_fs::power::{FakePower, FakePriority, PowerStatus};

    #[test]
    fn adapter_only_reports_battery_when_certain() {
        let p = Arc::new(FakePower::new(PowerStatus::Unknown));
        let a = PowerAdapter(p.clone());
        assert!(!a.on_battery(), "ukjent pauser aldri");
        p.set(PowerStatus::Battery);
        assert!(a.on_battery());
        p.set(PowerStatus::Ac);
        assert!(!a.on_battery());
    }

    #[test]
    fn wiring_leaves_an_idle_core_idle_even_on_battery() {
        let f = Fjell::new(
            Store::open_in_memory().unwrap(),
            Arc::new(fjell_core::model::MemSource::new()),
            Arc::new(FakeClock::new(1)),
        );
        let prio = Arc::new(FakePriority::default());
        wire_platform(
            &f,
            Arc::new(FakePower::new(PowerStatus::Battery)),
            prio.clone(),
        );
        // Pausen gjelder bare en aktiv kjøring.
        assert_eq!(f.indexing_control().state().code(), "idle");
        wire_default_platform(&f);
        assert_eq!(prio.calls(), 0);
    }
}
