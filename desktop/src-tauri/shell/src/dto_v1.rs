//! V1-typer for Tauri-grensen: beskyttelse, regler, personer, dashboard, serier, innstillinger.
//!
//! Avhenger av: `fjell-core::api`/`model` (V1-typene), `dto` (`kind_code`), serde. Grensesnitt: `*Dto`-typer og
//! konverteringer. Samme regel som `dto`: bare koder, tall og ID-er (brukerens egne data som regelnavn,
//! personnavn og mappestier går som de er). Nøkler er camelCase; koder er snake_case.
//! Rekkefølgen og navnene her må være lik `desktop/src/bridge/types.ts`.

use crate::dto::kind_code;
use fjell_core::api::{
    PersonView, ProtectionEntry, RuleDraft, RulePreview, RuleView, Sensitivity, SeriesCompare,
    Settings, Thresholds,
};
use fjell_core::dashboard::{Dashboard, NextStep};
use fjell_core::model::{
    AssetId, DashboardData, PersonId, ProtectScope, ReasonDetail, RuleCondition, RuleId, RuleSpec,
    SuggestionKind,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Tall/ID bak et begrunnelsesmerke (`ReasonDetail`).
#[derive(Serialize, Debug, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DetailDto {
    None,
    Percent { value: u8 },
    Count { value: u32 },
    Position { index: u32, of: u32 },
    Rule { id: u64 },
    Person { id: u64 },
}

impl From<ReasonDetail> for DetailDto {
    fn from(d: ReasonDetail) -> DetailDto {
        match d {
            ReasonDetail::None => DetailDto::None,
            ReasonDetail::Percent(value) => DetailDto::Percent { value },
            ReasonDetail::Count(value) => DetailDto::Count { value },
            ReasonDetail::Position { index, of } => DetailDto::Position { index, of },
            ReasonDetail::Rule(r) => DetailDto::Rule { id: r.0 },
            ReasonDetail::Person(p) => DetailDto::Person { id: p.0 },
        }
    }
}

/// Omfang for en beskyttelse: `scope` er `asset|folder|album|person`, `target` er id eller mappeprefiks.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ScopeDto {
    pub scope: String,
    pub target: String,
}

impl From<&ProtectScope> for ScopeDto {
    fn from(s: &ProtectScope) -> ScopeDto {
        ScopeDto {
            scope: s.scope_code().to_string(),
            target: s.target(),
        }
    }
}

impl ScopeDto {
    /// `None` ved ukjent omfangskode eller ugyldig tall.
    pub fn to_core(&self) -> Option<ProtectScope> {
        ProtectScope::from_parts(&self.scope, &self.target)
    }
}

#[derive(Serialize, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProtectionDto {
    pub scope: ScopeDto,
    pub created_at: i64,
}

impl From<&ProtectionEntry> for ProtectionDto {
    fn from(e: &ProtectionEntry) -> ProtectionDto {
        ProtectionDto {
            scope: ScopeDto::from(&e.scope),
            created_at: e.created_at,
        }
    }
}

/// Ett regelvilkår: `{ "kind": "older_than_days", "value": 90 }`; vilkår uten verdi har bare `kind`.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum ConditionDto {
    IsScreenshot,
    IsBlurry,
    IsExactDuplicate,
    OlderThanDays(u32),
    LargerThanBytes(u64),
    SmallerThanBytes(u64),
    InAlbum(u64),
    InFolder(String),
    Mime(String),
}

impl From<&RuleCondition> for ConditionDto {
    fn from(c: &RuleCondition) -> ConditionDto {
        match c {
            RuleCondition::IsScreenshot => ConditionDto::IsScreenshot,
            RuleCondition::IsBlurry => ConditionDto::IsBlurry,
            RuleCondition::IsExactDuplicate => ConditionDto::IsExactDuplicate,
            RuleCondition::OlderThanDays(d) => ConditionDto::OlderThanDays(*d),
            RuleCondition::LargerThanBytes(b) => ConditionDto::LargerThanBytes(*b),
            RuleCondition::SmallerThanBytes(b) => ConditionDto::SmallerThanBytes(*b),
            RuleCondition::InAlbum(a) => ConditionDto::InAlbum(*a),
            RuleCondition::InFolder(p) => ConditionDto::InFolder(p.clone()),
            RuleCondition::Mime(m) => ConditionDto::Mime(m.clone()),
        }
    }
}

impl From<&ConditionDto> for RuleCondition {
    fn from(c: &ConditionDto) -> RuleCondition {
        match c {
            ConditionDto::IsScreenshot => RuleCondition::IsScreenshot,
            ConditionDto::IsBlurry => RuleCondition::IsBlurry,
            ConditionDto::IsExactDuplicate => RuleCondition::IsExactDuplicate,
            ConditionDto::OlderThanDays(d) => RuleCondition::OlderThanDays(*d),
            ConditionDto::LargerThanBytes(b) => RuleCondition::LargerThanBytes(*b),
            ConditionDto::SmallerThanBytes(b) => RuleCondition::SmallerThanBytes(*b),
            ConditionDto::InAlbum(a) => RuleCondition::InAlbum(*a),
            ConditionDto::InFolder(p) => RuleCondition::InFolder(p.clone()),
            ConditionDto::Mime(m) => RuleCondition::Mime(m.clone()),
        }
    }
}

pub fn spec_from(conds: &[ConditionDto]) -> RuleSpec {
    RuleSpec {
        all_of: conds.iter().map(RuleCondition::from).collect(),
    }
}

/// Regel slik UI sender den inn (`id = null` oppretter).
#[derive(Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RuleDraftDto {
    pub id: Option<u64>,
    pub name: String,
    pub conditions: Vec<ConditionDto>,
}

impl From<&RuleDraftDto> for RuleDraft {
    fn from(d: &RuleDraftDto) -> RuleDraft {
        RuleDraft {
            id: d.id.map(RuleId),
            name: d.name.clone(),
            spec: spec_from(&d.conditions),
        }
    }
}

#[derive(Serialize, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RuleDto {
    pub id: u64,
    pub name: String,
    pub conditions: Vec<ConditionDto>,
    pub enabled: bool,
    pub hits: u64,
}

impl From<&RuleView> for RuleDto {
    fn from(r: &RuleView) -> RuleDto {
        RuleDto {
            id: r.id.0,
            name: r.name.clone(),
            conditions: r.spec.all_of.iter().map(ConditionDto::from).collect(),
            enabled: r.enabled,
            hits: r.hits,
        }
    }
}

#[derive(Serialize, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RulePreviewDto {
    pub hits: u64,
    pub bytes: u64,
    pub sample: Vec<u64>,
}

impl From<&RulePreview> for RulePreviewDto {
    fn from(p: &RulePreview) -> RulePreviewDto {
        RulePreviewDto {
            hits: p.hits,
            bytes: p.bytes,
            sample: p.sample.iter().map(|a: &AssetId| a.0).collect(),
        }
    }
}

#[derive(Serialize, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PersonDto {
    pub id: u64,
    pub name: Option<String>,
    pub face_count: u64,
    pub asset_count: u64,
    pub cover: Option<u64>,
    pub protected: bool,
}

impl From<&PersonView> for PersonDto {
    fn from(p: &PersonView) -> PersonDto {
        PersonDto {
            id: p.id.0,
            name: p.name.clone(),
            face_count: p.face_count,
            asset_count: p.asset_count,
            cover: p.cover.map(|a| a.0),
            protected: p.protected,
        }
    }
}

pub fn person_id(id: u64) -> PersonId {
    PersonId(id)
}

#[derive(Serialize, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct KindStatDto {
    pub kind: &'static str,
    pub count: u64,
    pub bytes: u64,
}

#[derive(Serialize, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct MonthStatDto {
    /// `ÅÅÅÅMM`, f.eks. 202611.
    pub year_month: u32,
    pub count: u64,
    pub bytes: u64,
}

#[derive(Serialize, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct FolderStatDto {
    pub path: String,
    pub count: u64,
    pub bytes: u64,
}

#[derive(Serialize, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DashboardDto {
    pub active_count: u64,
    pub active_bytes: u64,
    pub trashed_count: u64,
    pub trashed_bytes: u64,
    pub freed_bytes: u64,
    pub per_kind: Vec<KindStatDto>,
    pub by_month: Vec<MonthStatDto>,
    pub top_folders: Vec<FolderStatDto>,
    pub protected_count: u64,
    pub series_count: u64,
    pub people_count: u64,
    pub rules_active: u64,
    /// Fremdrift (`dashboard.progress.label`): avgjorte av totale forslag.
    pub progress: ProgressDto,
    /// Foreslått neste steg som kode med tall (`dashboard.next.*`); kjernen gjør ingenting selv.
    pub next_step: NextStepDto,
    /// Tømt per måned (UTC), eldst først.
    pub freed_by_month: Vec<FreedMonthDto>,
}

#[derive(Serialize, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProgressDto {
    pub decided: u64,
    pub total: u64,
    pub permille: u16,
}

/// `kind`: `review` (med `suggestionKind`, `count`, `bytes`), `empty_trash` (`count`, `bytes`) eller `nothing`.
#[derive(Serialize, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct NextStepDto {
    pub kind: String,
    pub suggestion_kind: Option<String>,
    pub count: u64,
    pub bytes: u64,
}

#[derive(Serialize, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct FreedMonthDto {
    pub year_month: u32,
    pub count: u64,
    pub bytes: u64,
}

impl From<&Dashboard> for DashboardDto {
    fn from(d: &Dashboard) -> DashboardDto {
        let mut dto = DashboardDto::from(&d.data);
        dto.progress = ProgressDto {
            decided: d.insights.progress.decided,
            total: d.insights.progress.total,
            permille: d.insights.progress.permille,
        };
        dto.next_step = match d.insights.next_step {
            NextStep::ReviewKind { kind, count, bytes } => NextStepDto {
                kind: "review".into(),
                suggestion_kind: Some(kind_code(kind).to_string()),
                count,
                bytes,
            },
            NextStep::EmptyTrash { count, bytes } => NextStepDto {
                kind: "empty_trash".into(),
                suggestion_kind: None,
                count,
                bytes,
            },
            NextStep::Nothing => NextStepDto::nothing(),
        };
        dto.freed_by_month = d
            .insights
            .freed_by_month
            .iter()
            .map(|m| FreedMonthDto {
                year_month: m.year_month,
                count: m.count,
                bytes: m.bytes,
            })
            .collect();
        dto
    }
}

impl NextStepDto {
    fn nothing() -> NextStepDto {
        NextStepDto {
            kind: "nothing".into(),
            suggestion_kind: None,
            count: 0,
            bytes: 0,
        }
    }
}

impl From<&DashboardData> for DashboardDto {
    fn from(d: &DashboardData) -> DashboardDto {
        DashboardDto {
            progress: ProgressDto {
                decided: 0,
                total: 0,
                permille: 0,
            },
            next_step: NextStepDto::nothing(),
            freed_by_month: Vec::new(),
            active_count: d.active_count,
            active_bytes: d.active_bytes,
            trashed_count: d.trashed_count,
            trashed_bytes: d.trashed_bytes,
            freed_bytes: d.freed_bytes,
            per_kind: d
                .per_kind
                .iter()
                .map(|k| KindStatDto {
                    kind: kind_code(k.kind),
                    count: k.count,
                    bytes: k.bytes,
                })
                .collect(),
            by_month: d
                .by_month
                .iter()
                .map(|m| MonthStatDto {
                    year_month: m.year_month,
                    count: m.count,
                    bytes: m.bytes,
                })
                .collect(),
            top_folders: d
                .top_folders
                .iter()
                .map(|f| FolderStatDto {
                    path: f.path.clone(),
                    count: f.count,
                    bytes: f.bytes,
                })
                .collect(),
            protected_count: d.protected_count,
            series_count: d.series_count,
            people_count: d.people_count,
            rules_active: d.rules_active,
        }
    }
}

#[derive(Serialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SeriesMemberDto {
    pub asset: u64,
    pub rank: u32,
    pub score: f32,
    pub is_keeper: bool,
    /// Begrunnelseskode (variantnavn i kjernen).
    pub reason: String,
    pub detail: DetailDto,
    pub bytes: u64,
    pub protect_flags: u8,
}

#[derive(Serialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SeriesCompareDto {
    pub series: u64,
    pub keeper: u64,
    pub members: Vec<SeriesMemberDto>,
}

impl From<&SeriesCompare> for SeriesCompareDto {
    fn from(c: &SeriesCompare) -> SeriesCompareDto {
        SeriesCompareDto {
            series: c.series.0,
            keeper: c.keeper.0,
            members: c
                .members
                .iter()
                .map(|m| SeriesMemberDto {
                    asset: m.asset.0,
                    rank: m.rank,
                    score: m.score,
                    is_keeper: m.is_keeper,
                    reason: format!("{:?}", m.reason),
                    detail: m.detail.into(),
                    bytes: m.bytes,
                    protect_flags: m.protect_flags.0,
                })
                .collect(),
        }
    }
}

/// Innstillinger: sensitivitet per forslagstype (`low|normal|high`), ansiktsindeks og papirkurvdager.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SettingsDto {
    pub trash_days: u32,
    /// Nøkkel = forslagstype (`exact_dup`, `visual_dup`, `blurry`, `screenshot`, `series`, `rule`).
    pub sensitivity: BTreeMap<String, String>,
    pub face_index: bool,
}

impl From<&Settings> for SettingsDto {
    fn from(s: &Settings) -> SettingsDto {
        SettingsDto {
            trash_days: s.trash_days,
            sensitivity: SuggestionKind::ALL
                .iter()
                .map(|k| {
                    (
                        kind_code(*k).to_string(),
                        s.sensitivity.get(*k).as_str().into(),
                    )
                })
                .collect(),
            face_index: s.face_index,
        }
    }
}

impl SettingsDto {
    /// Ukjent forslagstype eller sensitivitetskode gir `None` (kommandoen svarer `bad_request`).
    pub fn to_core(&self) -> Option<Settings> {
        let mut out = Settings {
            trash_days: self.trash_days,
            face_index: self.face_index,
            ..Settings::default()
        };
        for (k, v) in &self.sensitivity {
            let kind = crate::dto::parse_kind(k)?;
            out.sensitivity.set(kind, Sensitivity::parse(v)?);
        }
        Some(out)
    }
}

#[derive(Serialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ThresholdsDto {
    pub visual_max_hamming: u32,
    pub blur_max_score: f32,
    pub series_gap_secs: u32,
    pub closed_eyes_min_confidence: f32,
}

impl From<&Thresholds> for ThresholdsDto {
    fn from(t: &Thresholds) -> ThresholdsDto {
        ThresholdsDto {
            visual_max_hamming: t.visual_max_hamming,
            blur_max_score: t.blur_max_score,
            series_gap_secs: t.series_gap_secs,
            closed_eyes_min_confidence: t.closed_eyes_min_confidence,
        }
    }
}
