//! Speiltyper for UniFFI-grensen (L-utenfor-kjernen, ingen logikk): `From`-konverteringer til og fra
//! `fjell_core::api`/`model`. ID-er er rene `u64`; tekst finnes ikke (bare koder, tall og ID-er).
//!
//! Avhenger av: `fjell_core`. Grensesnitt: records/enums som speiler `api::types`, samt `FjellError`.

use fjell_core::api as a;
use fjell_core::model as m;

macro_rules! mirror_enum {
    ($name:ident <=> $core:path { $($v:ident),+ $(,)? }) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq, uniffi::Enum)]
        pub enum $name { $($v),+ }
        impl From<$core> for $name {
            fn from(x: $core) -> Self {
                use $core as C;
                match x { $(C::$v => $name::$v),+ }
            }
        }
        impl From<$name> for $core {
            fn from(x: $name) -> Self {
                use $core as C;
                match x { $($name::$v => C::$v),+ }
            }
        }
    };
}

mirror_enum!(SuggestionKind <=> m::SuggestionKind {
    ExactDup, VisualDup, Blurry, Screenshot, Series, Rule,
});
mirror_enum!(ReasonCode <=> m::ReasonCode {
    ExactCopy, VisualDuplicate, Blurry, Screenshot, HigherResolution, LessCompressed, Sharper,
    Older, ShotPlatformFlag, ShotScreenSize, ShotNoCameraExif, ShotPng, SeriesMember, KeeperBestOverall,
    KeeperEyesOpen, ClosedEyes, Rule, ProtectedPerson,
});
mirror_enum!(ConsequenceCode <=> m::ConsequenceCode {
    ProtectedFavorite, ProtectedSharedAlbum, ProtectedEdited, InAlbum, NotBackedUp, BackupUnknown,
    MassDelete, FreesNoStorageYet, PlatformDialogNext, ExpiredNotice, ProtectedByScope,
});
mirror_enum!(ScanMode <=> a::ScanMode { Quick, Full });
mirror_enum!(ScanPhase <=> a::ScanPhase { Idle, Listing, Exact, ExactReady, Analyzing, Done });
mirror_enum!(Decision <=> a::Decision { Trash, Keep });
mirror_enum!(DeleteOutcome <=> m::DeleteOutcome { Deleted, Cancelled, Failed });
mirror_enum!(AssetKind <=> m::Kind { Photo, ScreenshotHint, Video });
mirror_enum!(ThumbFmt <=> m::ThumbFmt { Gray8, Rgba8 });
mirror_enum!(BackupState <=> m::BackupState { NotBackedUp, BackedUp });

#[derive(Clone, Debug, PartialEq, uniffi::Record)]
pub struct Card {
    pub id: u64,
    pub kind: SuggestionKind,
    pub asset: u64,
    pub group: Option<u64>,
    pub keeper: Option<u64>,
    pub reason: ReasonCode,
    pub confidence: f32,
    pub bytes: u64,
    pub protect_flags: u8,
}

impl From<a::Card> for Card {
    fn from(c: a::Card) -> Self {
        Card {
            id: c.id.0,
            kind: c.kind.into(),
            asset: c.asset.0,
            group: c.group.map(|g| g.0),
            keeper: c.keeper.map(|k| k.0),
            reason: c.reason.into(),
            confidence: c.confidence,
            bytes: c.bytes,
            protect_flags: c.protect_flags.0,
        }
    }
}

#[derive(Clone, Debug, PartialEq, uniffi::Record)]
pub struct CardPage {
    pub cards: Vec<Card>,
    pub next: Option<u64>,
}

impl From<a::CardPage> for CardPage {
    fn from(p: a::CardPage) -> Self {
        CardPage {
            cards: p.cards.into_iter().map(Card::from).collect(),
            next: p.next.map(|c| c.0),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, uniffi::Record)]
pub struct CardDecision {
    pub card: u64,
    pub decision: Decision,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, uniffi::Record)]
pub struct ScanConfig {
    pub mode: ScanMode,
    pub priority: u8,
    pub deadline: Option<i64>,
}

impl From<ScanConfig> for a::ScanConfig {
    fn from(c: ScanConfig) -> Self {
        a::ScanConfig {
            mode: c.mode.into(),
            priority: c.priority,
            deadline: c.deadline,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, uniffi::Record)]
pub struct ScanProgress {
    pub phase: ScanPhase,
    pub done: u64,
    pub total: u64,
    pub found_exact: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, uniffi::Record)]
pub struct KindSummary {
    pub kind: SuggestionKind,
    pub count: u64,
    pub bytes: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, uniffi::Record)]
pub struct TrashSummary {
    pub count: u64,
    pub bytes: u64,
    pub expired: u64,
    pub freed_bytes: u64,
}

impl From<a::TrashSummary> for TrashSummary {
    fn from(t: a::TrashSummary) -> Self {
        TrashSummary {
            count: t.count,
            bytes: t.bytes,
            expired: t.expired,
            freed_bytes: t.freed_bytes,
        }
    }
}

#[derive(Clone, Debug, PartialEq, uniffi::Record)]
pub struct HomeSummary {
    pub per_kind: Vec<KindSummary>,
    pub reclaimable_bytes: u64,
    pub trash: TrashSummary,
    pub scan: ScanProgress,
}

impl From<a::HomeSummary> for HomeSummary {
    fn from(h: a::HomeSummary) -> Self {
        HomeSummary {
            per_kind: h
                .per_kind
                .into_iter()
                .map(|k| KindSummary {
                    kind: k.kind.into(),
                    count: k.count,
                    bytes: k.bytes,
                })
                .collect(),
            reclaimable_bytes: h.reclaimable_bytes,
            trash: h.trash.into(),
            scan: ScanProgress {
                phase: h.scan.phase.into(),
                done: h.scan.done,
                total: h.scan.total,
                found_exact: h.scan.found_exact,
            },
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, uniffi::Record)]
pub struct Settings {
    pub trash_days: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct PurgeIntent {
    pub ids: Vec<u64>,
    pub bytes: u64,
    pub digest: u64,
}

impl From<a::PurgeIntent> for PurgeIntent {
    fn from(p: a::PurgeIntent) -> Self {
        PurgeIntent {
            ids: p.ids.into_iter().map(|i| i.0).collect(),
            bytes: p.bytes,
            digest: p.digest,
        }
    }
}

impl From<PurgeIntent> for a::PurgeIntent {
    fn from(p: PurgeIntent) -> Self {
        a::PurgeIntent {
            ids: p.ids.into_iter().map(m::AssetId).collect(),
            bytes: p.bytes,
            digest: p.digest,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct PurgeReport {
    pub purged: Vec<u64>,
    pub kept_trashed: Vec<u64>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, uniffi::Record)]
pub struct UndoReport {
    pub batch: u64,
    pub restored: u64,
}

/// Plattform-ID for et asset (til plattformens egen sletting).
#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct SourceRef {
    pub asset: u64,
    pub source_id: String,
}

/// Plattformens utfall per asset, sendt tilbake til `finish_purge`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, uniffi::Record)]
pub struct DeleteResult {
    pub asset: u64,
    pub outcome: DeleteOutcome,
}

/// Feil på grensen (koder, ingen tekst). Speiler `api::ApiError`.
#[derive(Clone, Debug, PartialEq, Eq, uniffi::Error)]
pub enum FjellError {
    AckRequired { codes: Vec<ConsequenceCode> },
    UnknownCard { id: u64 },
    UnknownAsset { id: u64 },
    NothingToUndo,
    NotUndoable,
    AlreadyPurged { id: u64 },
    PurgeStale,
    PlanStale,
    NotTrashed { id: u64 },
    WouldEmptyGroup { group: u64 },
    Internal { detail: String },
}

impl std::fmt::Display for FjellError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for FjellError {}

impl From<a::ApiError> for FjellError {
    fn from(e: a::ApiError) -> Self {
        use a::ApiError as E;
        match e {
            E::AckRequired(c) => FjellError::AckRequired {
                codes: c.into_iter().map(Into::into).collect(),
            },
            E::UnknownCard(c) => FjellError::UnknownCard { id: c.0 },
            E::UnknownAsset(x) => FjellError::UnknownAsset { id: x.0 },
            E::NothingToUndo => FjellError::NothingToUndo,
            E::NotUndoable => FjellError::NotUndoable,
            E::AlreadyPurged(x) => FjellError::AlreadyPurged { id: x.0 },
            E::PurgeStale => FjellError::PurgeStale,
            E::PlanStale => FjellError::PlanStale,
            E::NotTrashed(x) => FjellError::NotTrashed { id: x.0 },
            E::WouldEmptyGroup(g) => FjellError::WouldEmptyGroup { group: g.0 },
            E::Internal(detail) => FjellError::Internal { detail },
        }
    }
}
