//! Modul `model` (L0). Se docs/mvp/ARKITEKTUR.md del 2 og PLAN-KJERNE del 2 (K0).
//!
//! Avhenger av: (ingen). Ingen I/O. Undermoduler: `traits` (AssetSource, Clock, PlatformDelete)
//! og `mem` (testdobler MemSource, FakeClock). Alt re-eksporteres her.
//! Grensesnitt (hovedpunkter):
//! - ID-er: `AssetId`, `SourceId`, `CardId`, `GroupId`, `BatchId`, `ChangeToken`.
//! - Verdier: `Hash64`, `Thumb`, `Kind`, `State`, `ReasonCode`, `SuggestionKind`, `ProtectFlags`, `AlgoVer`.
//! - Aggregat: `AssetMeta`, `Asset`, `Album`, `ChangeSet`, `Confirmed`, `PurgeTicket`.
//! - Trygghet (undermodul `plan`): `Plan`, `ConsequenceSummary`, `ConsequenceCode`, `Acks`, `PurgeOffer`.
//!
//! V1-tillegg (kontraktfasen, docs/v1/V1-PLAN.md): undermodulene `v1` (`SeriesId`, `PersonId`, `RuleId`,
//! `ProtectScope`, `ReasonDetail`, `RuleSpec`, `DashboardData`) og `analyze` (`EyeAnalyzer`, `FaceAnalyzer`
//! med Null-implementasjoner). `SuggestionKind` har `Series` og `Rule`; `ReasonCode` har flere koder.
//!
//! Kjernen returnerer koder, aldri tekst (beslutning 13).

pub mod analyze;
pub mod mem;
pub mod plan;
pub mod traits;
pub mod v1;

pub use analyze::{
    AnalyzeError, Embedding, EyeAnalyzer, EyeReport, FaceAnalyzer, FaceBox, FaceObservation,
    NullEyeAnalyzer, NullFaceAnalyzer,
};
pub use v1::{
    DashboardData, FolderStat, KindStat, MonthStat, PersonId, ProtectScope, ReasonDetail,
    RuleCondition, RuleId, RuleSpec, SeriesId,
};

pub use mem::{FakeClock, MemSource};
pub use plan::{
    Acks, AlbumCount, BackupSummary, ConsequenceCode, ConsequenceSummary, ExpiryNotice, KindCounts,
    MassAck, Plan, PlannedItem, ProtectCounts, ProtectedAck, PurgeOffer,
};
pub use traits::{AssetSource, Clock, DeleteOutcome, PlatformDelete, SourceError};

use std::fmt;

/// Algoritmeversjon for fingeravtrykk (beslutning 14). Økes når hash/score-algoritmer endres.
pub const ALGO_VER: AlgoVer = AlgoVer(1);
/// Lengste side på miniatyr levert av plattformen.
pub const THUMB_MAX_SIDE: u32 = 256;

/// Stabil intern ID (`asset.id` i indeksen).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AssetId(pub u64);

/// Plattformens egen ID eller sti (`asset.source_id`).
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SourceId(pub String);

/// Kort i UI (ett forslag per kort).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CardId(pub u64);

/// Gruppe (`grp.id`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GroupId(pub u64);

/// Handlingsbatch (`action_log.batch_id`): én batch = ett angre.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BatchId(pub u64);

/// Ugjennomsiktig posisjon i adapterens endringsstrøm (`mod_token`-basert skanning).
#[derive(Clone, Debug, PartialEq, Eq, Hash, Default)]
pub struct ChangeToken(pub String);

/// Algoritmeversjon (`fingerprint.algo_ver`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AlgoVer(pub u32);

/// 64-bits perseptuell hash (dHash eller pHash).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Hash64(pub u64);

impl Hash64 {
    /// Hammingavstand (antall ulike biter).
    pub fn hamming(self, other: Hash64) -> u32 {
        (self.0 ^ other.0).count_ones()
    }
}

/// Type element (`asset.kind`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Kind {
    Photo,
    ScreenshotHint,
    Video,
}

impl Kind {
    pub fn as_str(self) -> &'static str {
        match self {
            Kind::Photo => "photo",
            Kind::ScreenshotHint => "screenshot_hint",
            Kind::Video => "video",
        }
    }
}

/// Tilstand (`asset.state`). `Purged` er terminal.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum State {
    Active = 0,
    Trashed = 1,
    Purged = 2,
}

impl State {
    pub fn as_i64(self) -> i64 {
        self as i64
    }
    pub fn from_i64(v: i64) -> Option<State> {
        match v {
            0 => Some(State::Active),
            1 => Some(State::Trashed),
            2 => Some(State::Purged),
            _ => None,
        }
    }
}

/// Forslagstype (`suggestion.kind`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SuggestionKind {
    ExactDup,
    VisualDup,
    Blurry,
    Screenshot,
    /// V1: bilde i en serie som ikke er behold-kandidat.
    Series,
    /// V1: bilde som treffer en brukerdefinert regel.
    Rule,
}

impl SuggestionKind {
    pub const ALL: [SuggestionKind; 6] = [
        Self::ExactDup,
        Self::VisualDup,
        Self::Blurry,
        Self::Screenshot,
        Self::Series,
        Self::Rule,
    ];
    /// Forslagstypene som finnes i MVP-flaten (oversikten `home_summary`). Integrasjonsfasen bytter til `ALL`
    /// når UI viser serier og regler.
    pub const MVP: [SuggestionKind; 4] = [
        Self::ExactDup,
        Self::VisualDup,
        Self::Blurry,
        Self::Screenshot,
    ];
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ExactDup => "exact_dup",
            Self::VisualDup => "visual_dup",
            Self::Blurry => "blurry",
            Self::Screenshot => "screenshot",
            Self::Series => "series",
            Self::Rule => "rule",
        }
    }
}

/// Begrunnelseskode (PLAN-KJERNE del 11). UI oversetter via i18n-nøkkelen fra `key()`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ReasonCode {
    ExactCopy,
    VisualDuplicate,
    Blurry,
    Screenshot,
    HigherResolution,
    LessCompressed,
    Sharper,
    Older,
    ShotPlatformFlag,
    ShotScreenSize,
    ShotNoCameraExif,
    ShotPng,
    /// V1: bildet er med i en serie (se `ReasonDetail::Position`).
    SeriesMember,
    /// V1 keeper-forklaring: best samlet av seriens bilder.
    KeeperBestOverall,
    /// V1 keeper-forklaring: øynene er åpne i dette bildet.
    KeeperEyesOpen,
    /// V1: noen har lukkede øyne (`ReasonDetail::Count` = antall ansikter).
    ClosedEyes,
    /// V1: treffer en regel (`ReasonDetail::Rule`).
    Rule,
    /// V1: bildet viser en beskyttet person (`ReasonDetail::Person`); gjelder som beskyttelsesmerke.
    ProtectedPerson,
}

impl ReasonCode {
    pub const ALL: [ReasonCode; 18] = [
        Self::ExactCopy,
        Self::VisualDuplicate,
        Self::Blurry,
        Self::Screenshot,
        Self::HigherResolution,
        Self::LessCompressed,
        Self::Sharper,
        Self::Older,
        Self::ShotPlatformFlag,
        Self::ShotScreenSize,
        Self::ShotNoCameraExif,
        Self::ShotPng,
        Self::SeriesMember,
        Self::KeeperBestOverall,
        Self::KeeperEyesOpen,
        Self::ClosedEyes,
        Self::Rule,
        Self::ProtectedPerson,
    ];
    /// Stabil snake_case-nøkkel (kodenavn i i18n-nøkkelen `reason.<key>`).
    pub fn key(self) -> &'static str {
        match self {
            Self::ExactCopy => "exact_copy",
            Self::VisualDuplicate => "visual_duplicate",
            Self::Blurry => "blurry",
            Self::Screenshot => "screenshot",
            Self::HigherResolution => "higher_resolution",
            Self::LessCompressed => "less_compressed",
            Self::Sharper => "sharper",
            Self::Older => "older",
            Self::ShotPlatformFlag => "shot_platform_flag",
            Self::ShotScreenSize => "shot_screen_size",
            Self::ShotNoCameraExif => "shot_no_camera_exif",
            Self::ShotPng => "shot_png",
            Self::SeriesMember => "series_member",
            Self::KeeperBestOverall => "keeper_best_overall",
            Self::KeeperEyesOpen => "keeper_eyes_open",
            Self::ClosedEyes => "closed_eyes",
            Self::Rule => "rule",
            Self::ProtectedPerson => "protected_person",
        }
    }
}

/// Beskyttelsesflagg (`asset.protect_flags`, bitmaske).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub struct ProtectFlags(pub u8);

impl ProtectFlags {
    pub const FAVORITE: u8 = 1;
    pub const SHARED_ALBUM: u8 = 2;
    pub const EDITED: u8 = 4;
    /// Reservert (ansikter, post-MVP).
    pub const FACES: u8 = 8;
    pub fn has(self, flag: u8) -> bool {
        self.0 & flag != 0
    }
    pub fn is_protected(self) -> bool {
        self.0 != 0
    }
}

/// Album fra adapter (`album`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Album {
    pub id: u64,
    pub name: String,
    pub shared: bool,
}

/// Sikkerhetskopistatus fra plattformen (valgfri).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BackupState {
    NotBackedUp,
    BackedUp,
}

/// Metadata fra adapter. Alle felt fra PLAN-KJERNE 2 og PLAN-TRYGGHET landes her (endres ikke senere).
#[derive(Clone, Debug, PartialEq)]
pub struct AssetMeta {
    pub source_id: SourceId,
    pub kind: Kind,
    /// Unix-sekunder.
    pub created_at: i64,
    /// Stabil, endres ved redigering.
    pub mod_token: String,
    /// Etter orientering.
    pub width: u32,
    pub height: u32,
    pub bytes: u64,
    pub mime: String,
    pub platform_screenshot_hint: bool,
    pub has_camera_exif: Option<bool>,
    /// Originalen ligger på enheten (ikke bare i sky).
    pub is_local: bool,
    pub protect_flags: ProtectFlags,
    pub albums: Vec<Album>,
    pub backup: Option<BackupState>,
}

/// Rad i indeksen: metadata pluss tilstand.
#[derive(Clone, Debug, PartialEq)]
pub struct Asset {
    pub id: AssetId,
    pub meta: AssetMeta,
    pub state: State,
    pub trashed_at: Option<i64>,
    pub expires_at: Option<i64>,
}

/// Svar fra `AssetSource::list_changes`. `removed` avstemmes til `purged` med `cause=external`.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct ChangeSet {
    pub changed: Vec<AssetMeta>,
    pub removed: Vec<SourceId>,
    pub next_token: ChangeToken,
}

/// Pikselformat for `Thumb`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ThumbFmt {
    Gray8,
    Rgba8,
}

impl ThumbFmt {
    pub fn bytes_per_pixel(self) -> usize {
        match self {
            ThumbFmt::Gray8 => 1,
            ThumbFmt::Rgba8 => 4,
        }
    }
}

/// Miniatyr: lengste side ≤ 256, bildeforhold bevart, orientering brukt, sRGB.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Thumb {
    pub w: u32,
    pub h: u32,
    pub fmt: ThumbFmt,
    pub data: Vec<u8>,
}

/// Ugyldig `Thumb`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ThumbError {
    ZeroSize,
    TooLarge,
    BadLength { expected: usize, got: usize },
}

impl fmt::Display for ThumbError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ThumbError::ZeroSize => write!(f, "thumb has zero width or height"),
            ThumbError::TooLarge => write!(f, "thumb longest side exceeds {THUMB_MAX_SIDE}"),
            ThumbError::BadLength { expected, got } => {
                write!(f, "thumb data length {got}, expected {expected}")
            }
        }
    }
}

impl std::error::Error for ThumbError {}

impl Thumb {
    /// Validerer størrelse og datalengde.
    pub fn new(w: u32, h: u32, fmt: ThumbFmt, data: Vec<u8>) -> Result<Thumb, ThumbError> {
        if w == 0 || h == 0 {
            return Err(ThumbError::ZeroSize);
        }
        if w.max(h) > THUMB_MAX_SIDE {
            return Err(ThumbError::TooLarge);
        }
        let expected = w as usize * h as usize * fmt.bytes_per_pixel();
        if data.len() != expected {
            return Err(ThumbError::BadLength {
                expected,
                got: data.len(),
            });
        }
        Ok(Thumb { w, h, fmt, data })
    }
}

/// Bevis for brukerbekreftelse av purge. Kan bare lages i kjernen (`api`); digesten binder
/// bekreftelsen til mengden (I12 i PLAN-TRYGGHET).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Confirmed {
    digest: u64,
}

impl Confirmed {
    pub(crate) fn new(digest: u64) -> Confirmed {
        Confirmed { digest }
    }
    pub fn digest(&self) -> u64 {
        self.digest
    }
}

/// Engangsbillett for plattformens `delete`. Lages bare av `api::confirm_purge`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PurgeTicket {
    confirmed: Confirmed,
    ids: Vec<AssetId>,
    issued_at: i64,
}

impl PurgeTicket {
    pub(crate) fn new(confirmed: Confirmed, ids: Vec<AssetId>, issued_at: i64) -> PurgeTicket {
        PurgeTicket {
            confirmed,
            ids,
            issued_at,
        }
    }
    pub fn confirmed(&self) -> &Confirmed {
        &self.confirmed
    }
    pub fn ids(&self) -> &[AssetId] {
        &self.ids
    }
    pub fn issued_at(&self) -> i64 {
        self.issued_at
    }
}

#[cfg(test)]
mod tests;
