//! Robusthetstyper for Tauri-grensen: sikkerhetskopi av indeksen, integritet, indeksering og diagnose.
//!
//! Avhenger av: `fjell-core::api`/`model` (robusthetstypene), `dto` (`phase_code`), serde. Samme regel som `dto`:
//! bare koder, tall og ID-er; nøkler er camelCase, koder er snake_case. Rekkefølge og navn må være lik
//! `desktop/src/bridge/types-robust.ts`. Eies av kontraktfasen (docs/v1/ROBUSTHET-KONTRAKT.md): grenene
//! endrer ikke denne filen uten en ny kontraktrunde.

use crate::dto::phase_code;
use fjell_core::api::{BackupEntry, IndexingSettings, IndexingStatus, RestoreReport};
use fjell_core::model::{DiagReport, IndexPriority};
use serde::{Deserialize, Serialize};

/// Sikkerhetskopi av indeksen (`BackupEntry`).
#[derive(Serialize, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct BackupDto {
    pub id: u64,
    /// `manual`, `auto` eller `pre_restore`.
    pub kind: &'static str,
    /// Unix-sekunder.
    pub created_at: i64,
    pub bytes: u64,
    pub asset_count: u64,
    /// `ok`, `degraded`, `corrupt` eller `unreadable`.
    pub integrity: &'static str,
}

impl From<&BackupEntry> for BackupDto {
    fn from(b: &BackupEntry) -> BackupDto {
        BackupDto {
            id: b.id.0,
            kind: b.kind.as_str(),
            created_at: b.created_at,
            bytes: b.bytes,
            asset_count: b.asset_count,
            integrity: b.integrity.as_str(),
        }
    }
}

/// Resultat av gjenoppretting (`RestoreReport`). `safety` er kopien som angrer den.
#[derive(Serialize, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RestoreReportDto {
    pub restored: u64,
    pub safety: u64,
    pub asset_count: u64,
}

impl From<&RestoreReport> for RestoreReportDto {
    fn from(r: &RestoreReport) -> RestoreReportDto {
        RestoreReportDto {
            restored: r.restored.0,
            safety: r.safety.0,
            asset_count: r.asset_count,
        }
    }
}

/// Indekseringens status (`IndexingStatus`).
#[derive(Serialize, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct IndexingStatusDto {
    /// `idle`, `running`, `paused` eller `paused_on_battery`.
    pub state: &'static str,
    /// Samme koder som `StateDto::scan_phase`.
    pub phase: &'static str,
    pub done: u64,
    pub total: u64,
    pub rate_per_sec: u32,
    pub eta_secs: Option<u64>,
}

impl From<&IndexingStatus> for IndexingStatusDto {
    fn from(s: &IndexingStatus) -> IndexingStatusDto {
        IndexingStatusDto {
            state: s.state.as_str(),
            phase: phase_code(s.phase),
            done: s.done,
            total: s.total,
            rate_per_sec: s.rate_per_sec,
            eta_secs: s.eta_secs,
        }
    }
}

/// Indekseringsinnstillinger (begge retninger).
#[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct IndexingSettingsDto {
    /// 0 = automatisk.
    pub threads: u32,
    /// `low` eller `normal`.
    pub priority: String,
    pub pause_on_battery: bool,
}

impl From<&IndexingSettings> for IndexingSettingsDto {
    fn from(s: &IndexingSettings) -> IndexingSettingsDto {
        IndexingSettingsDto {
            threads: s.threads,
            priority: s.priority.as_str().to_string(),
            pause_on_battery: s.pause_on_battery,
        }
    }
}

impl IndexingSettingsDto {
    /// `None` ved ukjent prioritetskode (skallet svarer `indexing_invalid_settings`). Antall tråder valideres av kjernen.
    pub fn to_core(&self) -> Option<IndexingSettings> {
        Some(IndexingSettings {
            threads: self.threads,
            priority: IndexPriority::parse(&self.priority)?,
            pause_on_battery: self.pause_on_battery,
        })
    }
}

/// Én måling i diagnoserapporten.
#[derive(Serialize, Debug, PartialEq, Eq)]
pub struct DiagMetricDto {
    pub code: &'static str,
    pub value: u64,
}

/// Diagnoserapport (`DiagReport`): tall, koder og redigert tekstblokk. Ingen stier, filnavn eller bilder.
#[derive(Serialize, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DiagDto {
    pub format: u32,
    pub integrity: &'static str,
    pub indexing: &'static str,
    pub metrics: Vec<DiagMetricDto>,
    pub text: String,
}

impl From<&DiagReport> for DiagDto {
    fn from(r: &DiagReport) -> DiagDto {
        DiagDto {
            format: r.format,
            integrity: r.integrity.as_str(),
            indexing: r.indexing.as_str(),
            metrics: r
                .metrics
                .iter()
                .map(|(m, v)| DiagMetricDto {
                    code: m.as_str(),
                    value: *v,
                })
                .collect(),
            text: r.text.clone(),
        }
    }
}
