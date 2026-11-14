//! V1-kommandoer for skallet (`impl Shell`): beskyttelse, forslagsregler, personer og ansiktsindeks, dashboard,
//! seriesammenligning, forslagsinnstillinger og terskler. Del av `fjell-shell`. Beslutning med bekreftelse
//! (`decide_confirmed`), `set_keeper` og skallets innstillinger (`settings`) ligger i `lib.rs`.
//!
//! Avhenger av: `fjell-core::api` (`FjellApi`), `dto`, `dto_v1`. Grensesnitt: metodene under, én per
//! Tauri-kommando i `fjell-desktop` og én per funksjon i `desktop/src/bridge/bridge.ts`.
//!
//! Regler (CLAUDE.md): ingen sletting av bilder her. `unprotect`, `delete_rule` og `merge_people` har angre
//! (`protect`, `restore_rule`, `unmerge_person`); `wipe_face_index` fjerner bare avledede ansiktsdata og er et
//! eksplisitt personvernvalg UI må bekrefte. Beskyttede bilder gir `ack_required` med `protected_by_scope`
//! til brukeren har bekreftet via `decide_confirmed`. Ingen skjermer bruker disse ennå.

use crate::dto::ErrorDto;
use crate::dto_v1::{
    person_id, spec_from, ConditionDto, DashboardDto, PersonDto, ProtectionDto, RuleDraftDto,
    RuleDto, RulePreviewDto, ScopeDto, SeriesCompareDto, SuggestionSettingsDto, ThresholdsDto,
};
use crate::Shell;
use fjell_core::api::FjellApi;
use fjell_core::model::{RuleId, SeriesId};
use std::sync::Arc;

fn scope(s: &ScopeDto) -> Result<fjell_core::model::ProtectScope, ErrorDto> {
    s.to_core()
        .ok_or_else(|| ErrorDto::bad_request("unknown scope"))
}

impl Shell {
    /// Kjernens API (`no_library` før en mappe er valgt).
    fn api(&self) -> Result<Arc<dyn FjellApi>, ErrorDto> {
        Ok(self.parts()?.0)
    }

    /// Beskytter (`on`) eller fjerner beskyttelse (konfigurasjon; bildene røres ikke).
    pub fn protect(&self, s: &ScopeDto, on: bool) -> Result<(), ErrorDto> {
        let scope = scope(s)?;
        let api = self.api()?;
        if on {
            api.protect_set(scope)
        } else {
            api.unprotect(scope)
        }
        .map_err(ErrorDto::from)
    }

    pub fn protections(&self) -> Result<Vec<ProtectionDto>, ErrorDto> {
        Ok(self
            .api()?
            .protections()?
            .iter()
            .map(ProtectionDto::from)
            .collect())
    }

    pub fn rules(&self) -> Result<Vec<RuleDto>, ErrorDto> {
        Ok(self
            .api()?
            .rules_list()?
            .iter()
            .map(RuleDto::from)
            .collect())
    }

    pub fn save_rule(&self, draft: &RuleDraftDto) -> Result<u64, ErrorDto> {
        Ok(self.api()?.rule_save(draft.into())?.0)
    }

    pub fn preview_rule(&self, conditions: &[ConditionDto]) -> Result<RulePreviewDto, ErrorDto> {
        Ok(RulePreviewDto::from(
            &self.api()?.rule_preview(spec_from(conditions))?,
        ))
    }

    pub fn toggle_rule(&self, id: u64, enabled: bool) -> Result<(), ErrorDto> {
        self.api()?
            .rule_toggle(RuleId(id), enabled)
            .map_err(Into::into)
    }

    /// Angrebar (`restore_rule`).
    pub fn delete_rule(&self, id: u64) -> Result<(), ErrorDto> {
        self.api()?.rule_delete(RuleId(id)).map_err(Into::into)
    }

    pub fn restore_rule(&self, id: u64) -> Result<(), ErrorDto> {
        self.api()?.rule_restore(RuleId(id)).map_err(Into::into)
    }

    pub fn people(&self) -> Result<Vec<PersonDto>, ErrorDto> {
        Ok(self
            .api()?
            .people_list()?
            .iter()
            .map(PersonDto::from)
            .collect())
    }

    pub fn name_person(&self, id: u64, name: Option<String>) -> Result<(), ErrorDto> {
        self.api()?
            .person_name(person_id(id), name)
            .map_err(Into::into)
    }

    /// Returnerer personen som ble stående. Angrebar (`unmerge_person`).
    pub fn merge_people(&self, target: u64, absorbed: u64) -> Result<u64, ErrorDto> {
        Ok(self
            .api()?
            .person_merge(person_id(target), person_id(absorbed))?
            .0)
    }

    pub fn unmerge_person(&self, absorbed: u64) -> Result<(), ErrorDto> {
        self.api()?
            .person_unmerge(person_id(absorbed))
            .map_err(Into::into)
    }

    pub fn protect_person(&self, id: u64, protect: bool) -> Result<(), ErrorDto> {
        self.api()?
            .person_protect(person_id(id), protect)
            .map_err(Into::into)
    }

    /// Ansiktsindeks er av som standard.
    pub fn face_index(&self) -> Result<bool, ErrorDto> {
        Ok(self.api()?.face_index_enabled())
    }

    pub fn set_face_index(&self, enabled: bool) -> Result<(), ErrorDto> {
        self.api()?.face_index_set(enabled).map_err(Into::into)
    }

    /// Sletter ansiktsdata (aldri bilder). UI må bekrefte først. Returnerer antall ansikter.
    pub fn wipe_face_index(&self) -> Result<u64, ErrorDto> {
        self.api()?.face_index_wipe().map_err(Into::into)
    }

    pub fn dashboard(&self) -> Result<DashboardDto, ErrorDto> {
        Ok(DashboardDto::from(&self.api()?.dashboard_full()?))
    }

    pub fn compare_series(&self, series: u64) -> Result<SeriesCompareDto, ErrorDto> {
        Ok(SeriesCompareDto::from(
            &self.api()?.series_compare(SeriesId(series))?,
        ))
    }

    /// Forslagsinnstillinger (sensitivitet per forslagstype, ansiktsindeks, papirkurvfrist). Skallets egne
    /// innstillinger (karantenemappe, «Tøm»-standard) ligger i `Shell::settings`.
    pub fn suggestion_settings(&self) -> Result<SuggestionSettingsDto, ErrorDto> {
        Ok(SuggestionSettingsDto::from(&self.api()?.settings()))
    }

    pub fn set_suggestion_settings(
        &self,
        s: &SuggestionSettingsDto,
    ) -> Result<SuggestionSettingsDto, ErrorDto> {
        let core = s
            .to_core()
            .ok_or_else(|| ErrorDto::bad_request("unknown sensitivity or kind"))?;
        if !(1..=365).contains(&core.trash_days) {
            return Err(ErrorDto::bad_request("trash_days"));
        }
        self.api()?.set_settings(core);
        self.suggestion_settings()
    }

    pub fn thresholds(&self) -> Result<ThresholdsDto, ErrorDto> {
        Ok(ThresholdsDto::from(&self.api()?.thresholds()))
    }
}
