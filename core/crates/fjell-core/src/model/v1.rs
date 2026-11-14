//! V1-typer i `model` (L0): serier, personer, regler, beskyttelsesomfang og dashboard.
//! Se docs/v1/V1-PLAN.md. Ren datamodell uten I/O; alt bærer koder og tall, aldri UI-tekst
//! (unntatt brukerens egne data: regelnavn, personnavn, mappestier).
//!
//! Avhenger av: `super` (`AssetId`, `SuggestionKind`).
//! Grensesnitt:
//! - ID-er: `SeriesId` (= `grp.id` for `grp.kind = 'series'`), `PersonId`, `RuleId`.
//! - `ProtectScope` (Asset/Folder/Album/Person) med `scope_code`/`target`/`from_parts` (kodingen i `protection`).
//! - `ReasonDetail`: tall/ID bak et begrunnelsesmerke (prosent, antall, posisjon i serie, regel, person).
//! - `RuleSpec`/`RuleCondition`: regel = alle vilkår (OG). Tom regel treffer ingenting. Regler foreslår bare.
//! - `DashboardData` og delvisninger (`KindStat`, `MonthStat`, `FolderStat`).

use super::{AssetId, SuggestionKind};

/// Serie = rekke nesten like bilder tatt tett i tid. Lagres som `grp` med `kind = 'series'`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SeriesId(pub u64);

/// Person (`person.id`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PersonId(pub u64);

/// Regel (`rule.id`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RuleId(pub u64);

/// Hva en beskyttelse gjelder (`protection.scope` + `protection.target`).
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ProtectScope {
    Asset(AssetId),
    /// Mappeprefiks (normalisert `source_id`-prefiks). Gjelder alle bilder under mappen.
    Folder(String),
    Album(u64),
    Person(PersonId),
}

impl ProtectScope {
    /// Verdi for `protection.scope`.
    pub fn scope_code(&self) -> &'static str {
        match self {
            Self::Asset(_) => "asset",
            Self::Folder(_) => "folder",
            Self::Album(_) => "album",
            Self::Person(_) => "person",
        }
    }

    /// Verdi for `protection.target` (alltid tekst).
    pub fn target(&self) -> String {
        match self {
            Self::Asset(a) => a.0.to_string(),
            Self::Folder(p) => p.clone(),
            Self::Album(a) => a.to_string(),
            Self::Person(p) => p.0.to_string(),
        }
    }

    /// Motsatt vei; `None` ved ukjent kode eller ugyldig tall.
    pub fn from_parts(scope: &str, target: &str) -> Option<ProtectScope> {
        match scope {
            "asset" => target.parse().ok().map(|n| Self::Asset(AssetId(n))),
            "folder" => Some(Self::Folder(target.to_string())),
            "album" => target.parse().ok().map(Self::Album),
            "person" => target.parse().ok().map(|n| Self::Person(PersonId(n))),
            _ => None,
        }
    }
}

/// Tall eller ID bak et begrunnelsesmerke. UI setter dem inn i i18n-malen.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub enum ReasonDetail {
    #[default]
    None,
    /// Prosent (0..=100), f.eks. uskarphet.
    Percent(u8),
    Count(u32),
    /// Plass i serie, 1-basert.
    Position {
        index: u32,
        of: u32,
    },
    Rule(RuleId),
    Person(PersonId),
}

/// Ett vilkår i en regel.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RuleCondition {
    IsScreenshot,
    IsBlurry,
    IsExactDuplicate,
    OlderThanDays(u32),
    LargerThanBytes(u64),
    SmallerThanBytes(u64),
    InAlbum(u64),
    /// Mappeprefiks som i `ProtectScope::Folder`.
    InFolder(String),
    Mime(String),
}

/// Regelspesifikasjon: alle vilkår må gjelde (OG). En tom regel treffer ingenting (sikkerhet).
/// En regel produserer bare forslag (`SuggestionKind::Rule`); aldri sletting.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct RuleSpec {
    pub all_of: Vec<RuleCondition>,
}

impl RuleSpec {
    pub fn is_empty(&self) -> bool {
        self.all_of.is_empty()
    }
}

/// Antall og bytes per forslagstype (aktive, ventende forslag).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KindStat {
    pub kind: SuggestionKind,
    pub count: u64,
    pub bytes: u64,
}

/// Aktive bilder per måned (`year_month` = `ÅÅÅÅMM`, f.eks. 202611).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MonthStat {
    pub year_month: u32,
    pub count: u64,
    pub bytes: u64,
}

/// Aktive bilder per mappe (mappestien er brukerens egne data).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FolderStat {
    pub path: String,
    pub count: u64,
    pub bytes: u64,
}

/// Samlet lesemodell for dashboard. Tall kommer fra indeksen; kjernen påstår aldri frigjort plass før purge.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct DashboardData {
    pub active_count: u64,
    pub active_bytes: u64,
    pub trashed_count: u64,
    pub trashed_bytes: u64,
    /// Bytes frigjort av Fjell (permanent sletting).
    pub freed_bytes: u64,
    pub per_kind: Vec<KindStat>,
    pub by_month: Vec<MonthStat>,
    pub top_folders: Vec<FolderStat>,
    pub protected_count: u64,
    pub series_count: u64,
    pub people_count: u64,
    pub rules_active: u64,
}
