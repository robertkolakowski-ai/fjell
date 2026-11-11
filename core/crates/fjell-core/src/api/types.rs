//! Typer for `api` (L4): lesemodeller og feil på kontraktflaten mot UI/CLI.
//!
//! Avhenger av: `crate::model`. Alle typer bærer koder, tall og ID-er, aldri tekst.
//! Grensesnitt: `Card`, `CardPage`, `Decision`, `HomeSummary`, `TrashSummary`, `Settings`,
//! `PurgeIntent`, `PurgeReport`, `UndoReport`, `ScanConfig`, `ApiError`.

use crate::model::{
    AssetId, BatchId, CardId, ConsequenceCode, GroupId, ProtectFlags, ReasonCode, SuggestionKind,
};
use std::fmt;

/// Ett forslag som kort. Samme asset vises aldri i flere kort samtidig.
#[derive(Clone, Debug, PartialEq)]
pub struct Card {
    pub id: CardId,
    pub kind: SuggestionKind,
    pub asset: AssetId,
    /// Gruppe og behold-kandidat (bare duplikatkort).
    pub group: Option<GroupId>,
    pub keeper: Option<AssetId>,
    pub reason: ReasonCode,
    /// 0,0..=1,0.
    pub confidence: f32,
    /// Brukes til «kan frigjøres». Kjernen påstår aldri frigjort plass før purge.
    pub bytes: u64,
    pub protect_flags: ProtectFlags,
}

/// Posisjon i kortlisten (ugjennomsiktig for UI).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct CardCursor(pub u64);

/// Én side kort.
#[derive(Clone, Debug, PartialEq)]
pub struct CardPage {
    pub cards: Vec<Card>,
    pub next: Option<CardCursor>,
}

/// Brukerens beslutning for ett kort.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Decision {
    Trash,
    Keep,
}

/// Skanningsmodus.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScanMode {
    Quick,
    Full,
}

/// Konfigurasjon for `start_scan`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ScanConfig {
    pub mode: ScanMode,
    /// Høyere = viktigere (bakgrunn vs. forgrunn).
    pub priority: u8,
    /// Unix-sekunder; skanningen stopper og kan gjenopptas.
    pub deadline: Option<i64>,
}

/// Fase i skanning.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScanPhase {
    Idle,
    Listing,
    Exact,
    Analyzing,
    Done,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ScanProgress {
    pub phase: ScanPhase,
    pub done: u64,
    pub total: u64,
    pub found_exact: u64,
}

/// Antall og bytes per forslagstype.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KindSummary {
    pub kind: SuggestionKind,
    pub count: u64,
    pub bytes: u64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct HomeSummary {
    pub per_kind: Vec<KindSummary>,
    /// Sum bytes for ventende forslag (uten keepere).
    pub reclaimable_bytes: u64,
    pub trash: TrashSummary,
    pub scan: ScanProgress,
}

/// «Nylig slettet»: merket / kan frigjøres / frigjort (tre tall i UX).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct TrashSummary {
    pub count: u64,
    pub bytes: u64,
    pub expired: u64,
    /// Bytes frigjort av Fjell (etter purge).
    pub freed_bytes: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Settings {
    pub trash_days: u32,
}

impl Default for Settings {
    fn default() -> Self {
        Settings { trash_days: 30 }
    }
}

/// Resultat av `begin_purge`: forklaring før systemdialog. `digest` binder mengden.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PurgeIntent {
    pub ids: Vec<AssetId>,
    pub bytes: u64,
    pub digest: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PurgeReport {
    pub purged: Vec<AssetId>,
    /// Forblir `trashed` (avbrutt eller feilet): ingen tap.
    pub kept_trashed: Vec<AssetId>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UndoReport {
    pub batch: BatchId,
    pub restored: u64,
}

/// Feil på kontraktflaten (koder, ingen tekst).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ApiError {
    /// Beskyttede/masse: kall på nytt med bekreftelse (kommer med TRYGGHET).
    AckRequired(Vec<ConsequenceCode>),
    UnknownCard(CardId),
    UnknownAsset(AssetId),
    NothingToUndo,
    NotUndoable,
    AlreadyPurged(AssetId),
    PurgeStale,
    Internal(String),
}

impl fmt::Display for ApiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for ApiError {}
