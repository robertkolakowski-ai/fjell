//! V1-speiltyper for UniFFI-grensen (ingen logikk; bare `From`-konverteringer), se docs/v1/FFI.md.
//! Alt er koder, tall og ID-er. Eneste tekst er brukerens egne data (regelnavn, personnavn, mappestier,
//! MIME-typer), aldri UI-tekst.
//!
//! Avhenger av: `fjell_core` (`api`, `model`, `dashboard`), `types`.

use crate::types::{ReasonCode, SuggestionKind};
use fjell_core::api as a;
use fjell_core::dashboard as d;
use fjell_core::model as m;

/// Tall eller ID bak et begrunnelsesmerke (speiler `model::ReasonDetail`).
#[derive(Clone, Copy, Debug, PartialEq, uniffi::Enum)]
pub enum ReasonDetail {
    None,
    Percent { value: u8 },
    Count { value: u32 },
    Position { index: u32, of: u32 },
    Rule { id: u64 },
    Person { id: u64 },
}

impl From<m::ReasonDetail> for ReasonDetail {
    fn from(x: m::ReasonDetail) -> Self {
        match x {
            m::ReasonDetail::None => ReasonDetail::None,
            m::ReasonDetail::Percent(value) => ReasonDetail::Percent { value },
            m::ReasonDetail::Count(value) => ReasonDetail::Count { value },
            m::ReasonDetail::Position { index, of } => ReasonDetail::Position { index, of },
            m::ReasonDetail::Rule(r) => ReasonDetail::Rule { id: r.0 },
            m::ReasonDetail::Person(p) => ReasonDetail::Person { id: p.0 },
        }
    }
}

impl From<ReasonDetail> for m::ReasonDetail {
    fn from(x: ReasonDetail) -> Self {
        match x {
            ReasonDetail::None => m::ReasonDetail::None,
            ReasonDetail::Percent { value } => m::ReasonDetail::Percent(value),
            ReasonDetail::Count { value } => m::ReasonDetail::Count(value),
            ReasonDetail::Position { index, of } => m::ReasonDetail::Position { index, of },
            ReasonDetail::Rule { id } => m::ReasonDetail::Rule(m::RuleId(id)),
            ReasonDetail::Person { id } => m::ReasonDetail::Person(m::PersonId(id)),
        }
    }
}

/// Hva en beskyttelse gjelder.
#[derive(Clone, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum ProtectScope {
    Asset { id: u64 },
    Folder { path: String },
    Album { id: u64 },
    Person { id: u64 },
}

impl From<ProtectScope> for m::ProtectScope {
    fn from(s: ProtectScope) -> Self {
        match s {
            ProtectScope::Asset { id } => m::ProtectScope::Asset(m::AssetId(id)),
            ProtectScope::Folder { path } => m::ProtectScope::Folder(path),
            ProtectScope::Album { id } => m::ProtectScope::Album(id),
            ProtectScope::Person { id } => m::ProtectScope::Person(m::PersonId(id)),
        }
    }
}

impl From<m::ProtectScope> for ProtectScope {
    fn from(s: m::ProtectScope) -> Self {
        match s {
            m::ProtectScope::Asset(x) => ProtectScope::Asset { id: x.0 },
            m::ProtectScope::Folder(path) => ProtectScope::Folder { path },
            m::ProtectScope::Album(id) => ProtectScope::Album { id },
            m::ProtectScope::Person(p) => ProtectScope::Person { id: p.0 },
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct ProtectionEntry {
    pub scope: ProtectScope,
    pub created_at: i64,
}

impl From<a::ProtectionEntry> for ProtectionEntry {
    fn from(e: a::ProtectionEntry) -> Self {
        ProtectionEntry {
            scope: e.scope.into(),
            created_at: e.created_at,
        }
    }
}

/// Ett vilkår i en regel (speiler `model::RuleCondition`).
#[derive(Clone, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum RuleCondition {
    IsScreenshot,
    IsBlurry,
    IsExactDuplicate,
    OlderThanDays { days: u32 },
    LargerThanBytes { bytes: u64 },
    SmallerThanBytes { bytes: u64 },
    InAlbum { id: u64 },
    InFolder { path: String },
    Mime { mime: String },
}

impl From<RuleCondition> for m::RuleCondition {
    fn from(c: RuleCondition) -> Self {
        match c {
            RuleCondition::IsScreenshot => m::RuleCondition::IsScreenshot,
            RuleCondition::IsBlurry => m::RuleCondition::IsBlurry,
            RuleCondition::IsExactDuplicate => m::RuleCondition::IsExactDuplicate,
            RuleCondition::OlderThanDays { days } => m::RuleCondition::OlderThanDays(days),
            RuleCondition::LargerThanBytes { bytes } => m::RuleCondition::LargerThanBytes(bytes),
            RuleCondition::SmallerThanBytes { bytes } => m::RuleCondition::SmallerThanBytes(bytes),
            RuleCondition::InAlbum { id } => m::RuleCondition::InAlbum(id),
            RuleCondition::InFolder { path } => m::RuleCondition::InFolder(path),
            RuleCondition::Mime { mime } => m::RuleCondition::Mime(mime),
        }
    }
}

impl From<m::RuleCondition> for RuleCondition {
    fn from(c: m::RuleCondition) -> Self {
        match c {
            m::RuleCondition::IsScreenshot => RuleCondition::IsScreenshot,
            m::RuleCondition::IsBlurry => RuleCondition::IsBlurry,
            m::RuleCondition::IsExactDuplicate => RuleCondition::IsExactDuplicate,
            m::RuleCondition::OlderThanDays(days) => RuleCondition::OlderThanDays { days },
            m::RuleCondition::LargerThanBytes(bytes) => RuleCondition::LargerThanBytes { bytes },
            m::RuleCondition::SmallerThanBytes(bytes) => RuleCondition::SmallerThanBytes { bytes },
            m::RuleCondition::InAlbum(id) => RuleCondition::InAlbum { id },
            m::RuleCondition::InFolder(path) => RuleCondition::InFolder { path },
            m::RuleCondition::Mime(mime) => RuleCondition::Mime { mime },
        }
    }
}

/// Alle vilkår må gjelde (OG). Tom regel treffer ingenting og avvises ved lagring.
#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct RuleSpec {
    pub all_of: Vec<RuleCondition>,
}

impl From<RuleSpec> for m::RuleSpec {
    fn from(s: RuleSpec) -> Self {
        m::RuleSpec {
            all_of: s.all_of.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<m::RuleSpec> for RuleSpec {
    fn from(s: m::RuleSpec) -> Self {
        RuleSpec {
            all_of: s.all_of.into_iter().map(Into::into).collect(),
        }
    }
}

/// `id = None` oppretter, ellers oppdateres regelen.
#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct RuleDraft {
    pub id: Option<u64>,
    pub name: String,
    pub spec: RuleSpec,
}

impl From<RuleDraft> for a::RuleDraft {
    fn from(x: RuleDraft) -> Self {
        a::RuleDraft {
            id: x.id.map(m::RuleId),
            name: x.name,
            spec: x.spec.into(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct RuleView {
    pub id: u64,
    pub name: String,
    pub spec: RuleSpec,
    pub enabled: bool,
    pub hits: u64,
}

impl From<a::RuleView> for RuleView {
    fn from(r: a::RuleView) -> Self {
        RuleView {
            id: r.id.0,
            name: r.name,
            spec: r.spec.into(),
            enabled: r.enabled,
            hits: r.hits,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct RulePreview {
    pub hits: u64,
    pub bytes: u64,
    pub sample: Vec<u64>,
}

impl From<a::RulePreview> for RulePreview {
    fn from(p: a::RulePreview) -> Self {
        RulePreview {
            hits: p.hits,
            bytes: p.bytes,
            sample: p.sample.into_iter().map(|i| i.0).collect(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct PersonView {
    pub id: u64,
    pub name: Option<String>,
    pub face_count: u64,
    pub asset_count: u64,
    pub cover: Option<u64>,
    pub protected: bool,
}

impl From<a::PersonView> for PersonView {
    fn from(p: a::PersonView) -> Self {
        PersonView {
            id: p.id.0,
            name: p.name,
            face_count: p.face_count,
            asset_count: p.asset_count,
            cover: p.cover.map(|c| c.0),
            protected: p.protected,
        }
    }
}

#[derive(Clone, Debug, PartialEq, uniffi::Record)]
pub struct SeriesMemberView {
    pub asset: u64,
    pub rank: u32,
    pub score: f32,
    pub is_keeper: bool,
    pub reason: ReasonCode,
    pub detail: ReasonDetail,
    pub bytes: u64,
    pub protect_flags: u8,
}

#[derive(Clone, Debug, PartialEq, uniffi::Record)]
pub struct SeriesCompare {
    pub series: u64,
    pub keeper: u64,
    pub members: Vec<SeriesMemberView>,
}

impl From<a::SeriesCompare> for SeriesCompare {
    fn from(s: a::SeriesCompare) -> Self {
        SeriesCompare {
            series: s.series.0,
            keeper: s.keeper.0,
            members: s
                .members
                .into_iter()
                .map(|x| SeriesMemberView {
                    asset: x.asset.0,
                    rank: x.rank,
                    score: x.score,
                    is_keeper: x.is_keeper,
                    reason: x.reason.into(),
                    detail: x.detail.into(),
                    bytes: x.bytes,
                    protect_flags: x.protect_flags.0,
                })
                .collect(),
        }
    }
}

/// Effektive terskler for gjeldende sensitivitet. Kjernen eier verdiene; plattformen viser dem bare.
#[derive(Clone, Copy, Debug, PartialEq, uniffi::Record)]
pub struct Thresholds {
    pub visual_max_hamming: u32,
    pub blur_max_score: f32,
    pub series_gap_secs: u32,
    pub closed_eyes_min_confidence: f32,
}

impl From<a::Thresholds> for Thresholds {
    fn from(t: a::Thresholds) -> Self {
        Thresholds {
            visual_max_hamming: t.visual_max_hamming,
            blur_max_score: t.blur_max_score,
            series_gap_secs: t.series_gap_secs,
            closed_eyes_min_confidence: t.closed_eyes_min_confidence,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, uniffi::Record)]
pub struct KindStat {
    pub kind: SuggestionKind,
    pub count: u64,
    pub bytes: u64,
}

/// `year_month` = ÅÅÅÅMM (f.eks. 202611).
#[derive(Clone, Copy, Debug, PartialEq, Eq, uniffi::Record)]
pub struct MonthStat {
    pub year_month: u32,
    pub count: u64,
    pub bytes: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct FolderStat {
    pub path: String,
    pub count: u64,
    pub bytes: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct DashboardData {
    pub active_count: u64,
    pub active_bytes: u64,
    pub trashed_count: u64,
    pub trashed_bytes: u64,
    pub freed_bytes: u64,
    pub per_kind: Vec<KindStat>,
    pub by_month: Vec<MonthStat>,
    pub top_folders: Vec<FolderStat>,
    pub protected_count: u64,
    pub series_count: u64,
    pub people_count: u64,
    pub rules_active: u64,
}

impl From<m::DashboardData> for DashboardData {
    fn from(x: m::DashboardData) -> Self {
        DashboardData {
            active_count: x.active_count,
            active_bytes: x.active_bytes,
            trashed_count: x.trashed_count,
            trashed_bytes: x.trashed_bytes,
            freed_bytes: x.freed_bytes,
            per_kind: x
                .per_kind
                .into_iter()
                .map(|k| KindStat {
                    kind: k.kind.into(),
                    count: k.count,
                    bytes: k.bytes,
                })
                .collect(),
            by_month: x
                .by_month
                .into_iter()
                .map(|s| MonthStat {
                    year_month: s.year_month,
                    count: s.count,
                    bytes: s.bytes,
                })
                .collect(),
            top_folders: x
                .top_folders
                .into_iter()
                .map(|f| FolderStat {
                    path: f.path,
                    count: f.count,
                    bytes: f.bytes,
                })
                .collect(),
            protected_count: x.protected_count,
            series_count: x.series_count,
            people_count: x.people_count,
            rules_active: x.rules_active,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, uniffi::Record)]
pub struct Progress {
    pub decided: u64,
    pub total: u64,
    pub permille: u16,
}

/// Neste steg som forslag (kode og tall). Kjernen gjør ingenting selv.
#[derive(Clone, Copy, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum NextStep {
    ReviewKind {
        kind: SuggestionKind,
        count: u64,
        bytes: u64,
    },
    EmptyTrash {
        count: u64,
        bytes: u64,
    },
    Nothing,
}

impl From<d::NextStep> for NextStep {
    fn from(n: d::NextStep) -> Self {
        match n {
            d::NextStep::ReviewKind { kind, count, bytes } => NextStep::ReviewKind {
                kind: kind.into(),
                count,
                bytes,
            },
            d::NextStep::EmptyTrash { count, bytes } => NextStep::EmptyTrash { count, bytes },
            d::NextStep::Nothing => NextStep::Nothing,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, uniffi::Record)]
pub struct FreedMonth {
    pub year_month: u32,
    pub count: u64,
    pub bytes: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct Insights {
    pub reclaimable_count: u64,
    pub reclaimable_bytes: u64,
    pub protected_count: u64,
    pub progress: Progress,
    pub next_step: NextStep,
    pub freed_by_month: Vec<FreedMonth>,
}

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct Dashboard {
    pub data: DashboardData,
    pub insights: Insights,
}

impl From<d::Dashboard> for Dashboard {
    fn from(x: d::Dashboard) -> Self {
        let i = x.insights;
        Dashboard {
            data: x.data.into(),
            insights: Insights {
                reclaimable_count: i.reclaimable_count,
                reclaimable_bytes: i.reclaimable_bytes,
                protected_count: i.protected_count,
                progress: Progress {
                    decided: i.progress.decided,
                    total: i.progress.total,
                    permille: i.progress.permille,
                },
                next_step: i.next_step.into(),
                freed_by_month: i
                    .freed_by_month
                    .into_iter()
                    .map(|f| FreedMonth {
                        year_month: f.year_month,
                        count: f.count,
                        bytes: f.bytes,
                    })
                    .collect(),
            },
        }
    }
}
