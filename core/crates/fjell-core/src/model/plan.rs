//! Typer for trygg sletting (PLAN-TRYGGHET del 4): plan, konsekvenser, bekreftelser.
//!
//! Avhenger av: `super` (id-er og enum-er). Ingen I/O og ingen tekst: bare koder, tall og id-er.
//! Grensesnitt:
//! - `Plan { items, summary, digest, created_at }` fra `actions::plan`; `ConsequenceSummary` og `ConsequenceCode`.
//! - `ProtectedAck` (per beskyttelseskategori) og `MassAck`: bundet til plan-digesten (I12).
//!   Konstruktørene er `pub(crate)`: bare kjernen (`api`) kan lage dem, aldri adaptere.
//! - `PurgeOffer` (fra `begin_purge`) og `ExpiryNotice` (fra `expire_due`).

use super::{AssetId, ProtectFlags, ReasonCode, SuggestionKind};

/// Konsekvenskode som UI oversetter (nøkkel `consequence.<key>`). Aldri tekst fra kjernen.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ConsequenceCode {
    ProtectedFavorite,
    ProtectedSharedAlbum,
    ProtectedEdited,
    InAlbum,
    /// Adapteren ga `Unverified` (minst ett bilde er kjent som ikke sikkerhetskopiert).
    NotBackedUp,
    BackupUnknown,
    MassDelete,
    FreesNoStorageYet,
    PlatformDialogNext,
    ExpiredNotice,
    /// V1: bildet er beskyttet via mappe, album, person eller enkeltbilde (`protection`-tabellen).
    ProtectedByScope,
}

impl ConsequenceCode {
    pub const ALL: [ConsequenceCode; 11] = [
        Self::ProtectedFavorite,
        Self::ProtectedSharedAlbum,
        Self::ProtectedEdited,
        Self::InAlbum,
        Self::NotBackedUp,
        Self::BackupUnknown,
        Self::MassDelete,
        Self::FreesNoStorageYet,
        Self::PlatformDialogNext,
        Self::ExpiredNotice,
        Self::ProtectedByScope,
    ];

    /// Stabil snake_case-nøkkel (i18n-nøkkel `consequence.<key>`).
    pub fn key(self) -> &'static str {
        match self {
            Self::ProtectedFavorite => "protected_favorite",
            Self::ProtectedSharedAlbum => "protected_shared_album",
            Self::ProtectedEdited => "protected_edited",
            Self::InAlbum => "in_album",
            Self::NotBackedUp => "not_backed_up",
            Self::BackupUnknown => "backup_unknown",
            Self::MassDelete => "mass_delete",
            Self::FreesNoStorageYet => "frees_no_storage_yet",
            Self::PlatformDialogNext => "platform_dialog_next",
            Self::ExpiredNotice => "expired_notice",
            Self::ProtectedByScope => "protected_by_scope",
        }
    }

    /// Beskyttelsesbit (`ProtectFlags::*`) som koden gjelder, hvis det er en beskyttelseskode.
    pub fn protect_flag(self) -> Option<u8> {
        match self {
            Self::ProtectedFavorite => Some(ProtectFlags::FAVORITE),
            Self::ProtectedSharedAlbum => Some(ProtectFlags::SHARED_ALBUM),
            Self::ProtectedEdited => Some(ProtectFlags::EDITED),
            _ => None,
        }
    }
}

/// Beskyttelseskategorier som krever egen ack (`FACES` er reservert og ikke satt i MVP).
pub const ACK_CATEGORIES: [(u8, ConsequenceCode); 3] = [
    (ProtectFlags::FAVORITE, ConsequenceCode::ProtectedFavorite),
    (
        ProtectFlags::SHARED_ALBUM,
        ConsequenceCode::ProtectedSharedAlbum,
    ),
    (ProtectFlags::EDITED, ConsequenceCode::ProtectedEdited),
];

/// Ett bilde i en plan med begrunnelse og beskyttelse slik de var da planen ble laget.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlannedItem {
    pub id: AssetId,
    /// `None` = manuelt valg.
    pub kind: Option<SuggestionKind>,
    pub reason: Option<ReasonCode>,
    pub protect: ProtectFlags,
}

/// Antall per forslagstype (`manual` = ikke fra et forslag).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct KindCounts {
    pub exact_dup: u64,
    pub visual_dup: u64,
    pub blurry: u64,
    pub screenshot: u64,
    pub series: u64,
    pub rule: u64,
    pub manual: u64,
}

/// Antall bilder per beskyttelseskategori (et bilde kan telle i flere).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct ProtectCounts {
    pub favorite: u64,
    pub shared_album: u64,
    pub edited: u64,
    /// V1: beskyttet via mappe, album, person eller enkeltbilde (`ProtectScope`).
    pub scope: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AlbumCount {
    pub album_id: u64,
    pub name: String,
    pub count: u64,
}

/// Sikkerhetskopistatus samlet over mengden.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BackupSummary {
    Verified,
    Unverified,
    Unknown,
}

/// Konsekvensforhåndsvisning (PLAN-TRYGGHET 4.1). Beregnes ren på indeksdata.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConsequenceSummary {
    pub count: u64,
    pub bytes: u64,
    pub by_kind: KindCounts,
    /// Sortert synkende på antall, deretter album-id.
    pub albums: Vec<AlbumCount>,
    pub protected: ProtectCounts,
    pub backup: BackupSummary,
    pub groups_untouched_keeper: bool,
    /// Alltid `false` for papirkurv: plassen frigjøres først ved «Tøm».
    pub frees_storage_now: bool,
    pub codes: Vec<ConsequenceCode>,
}

/// Resultat av `plan()`: nøyaktig det `commit()` vil endre. Intet er skrevet.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Plan {
    pub items: Vec<PlannedItem>,
    pub summary: ConsequenceSummary,
    /// Over (id, tilstand, beskyttelse) for alle `items` (I12).
    pub digest: u64,
    /// Unix-sekunder (planen foreldes etter 5 min).
    pub created_at: i64,
}

impl Plan {
    pub fn ids(&self) -> Vec<AssetId> {
        self.items.iter().map(|i| i.id).collect()
    }
}

/// Ack for én beskyttelseskategori. Dekker bare id-ene og kategorien den lister (I7, T5).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProtectedAck {
    category: u8,
    ids: Vec<AssetId>,
    digest: u64,
}

impl ProtectedAck {
    #[allow(dead_code)] // laget av `api` (M4) og testene
    pub(crate) fn new(category: u8, ids: Vec<AssetId>, digest: u64) -> ProtectedAck {
        ProtectedAck {
            category,
            ids,
            digest,
        }
    }
    pub fn category(&self) -> u8 {
        self.category
    }
    pub fn ids(&self) -> &[AssetId] {
        &self.ids
    }
    pub fn digest(&self) -> u64 {
        self.digest
    }
}

/// Avkrysning for massesletting. Bundet til digesten til planen eller `PurgeOffer`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MassAck {
    digest: u64,
}

impl MassAck {
    #[allow(dead_code)] // laget av `api` (M4) og testene
    pub(crate) fn new(digest: u64) -> MassAck {
        MassAck { digest }
    }
    pub fn digest(&self) -> u64 {
        self.digest
    }
}

/// Alle bekreftelser som følger et `commit`.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct Acks {
    pub protected: Vec<ProtectedAck>,
    pub mass: Option<MassAck>,
}

/// Resultat av `begin_purge`: det UI forklarer før `confirm_purge`. Bundet via `digest`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PurgeOffer {
    pub ids: Vec<AssetId>,
    pub bytes: u64,
    pub expired_count: u64,
    pub digest: u64,
    pub codes: Vec<ConsequenceCode>,
}

/// Utløpsvarsel (én gang per element). Kjernen sletter aldri.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ExpiryNotice {
    pub count: u64,
    pub bytes: u64,
}
