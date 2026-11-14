//! Typer for `api` (L4): lesemodeller og feil på kontraktflaten mot UI/CLI.
//!
//! Avhenger av: `crate::model`. Alle typer bærer koder, tall og ID-er, aldri tekst.
//! Grensesnitt: `Card`, `CardPage`, `Decision`, `HomeSummary`, `TrashSummary`, `Settings`,
//! `PurgeIntent`, `PurgeReport`, `UndoReport`, `ScanConfig`, `ApiError`.
//! V1 (docs/v1/V1-PLAN.md): `Sensitivity`/`Sensitivities`, `Thresholds`, `ProtectionEntry`, `RuleDraft`, `RuleView`,
//! `RulePreview`, `PersonView`, `SeriesCompare`/`SeriesMemberView`, `FACE_INDEX_KEY`, `sensitivity_key`.

use crate::model::{
    AssetId, BatchId, CardId, ConsequenceCode, GroupId, PersonId, ProtectFlags, ProtectScope,
    ReasonCode, ReasonDetail, RuleId, RuleSpec, SeriesId, SuggestionKind,
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
    /// V1: tall/ID bak begrunnelsesmerket (`ReasonDetail::None` når merket ikke trenger noe).
    pub detail: ReasonDetail,
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
    /// Rask fase pågår (BLAKE3, ingen dekoding).
    Exact,
    /// Eksakte grupper og forslag er skrevet; ingen miniatyr er nødvendigvis hentet ennå.
    ExactReady,
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

/// Hvor ivrig appen er per forslagstype (V1). Styrer terskler, ikke hva som er lov: appen foreslår bare.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Sensitivity {
    /// Færre, sikrere forslag.
    Low,
    #[default]
    Normal,
    /// Flere forslag, lavere sikkerhet.
    High,
}

impl Sensitivity {
    pub const ALL: [Sensitivity; 3] = [Self::Low, Self::Normal, Self::High];

    /// Stabil kode for lagring i `setting`.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Normal => "normal",
            Self::High => "high",
        }
    }

    /// Ukjent kode gir `None` (kalleren bruker standard).
    pub fn parse(code: &str) -> Option<Sensitivity> {
        Self::ALL.into_iter().find(|s| s.as_str() == code)
    }
}

/// `setting.key` for sensitivitet per forslagstype, f.eks. `sensitivity.blurry`.
pub fn sensitivity_key(kind: SuggestionKind) -> String {
    format!("sensitivity.{}", kind.as_str())
}

/// `setting.key` for ansiktsindeks (`'1'` = på, alt annet eller manglende = av).
pub const FACE_INDEX_KEY: &str = "face_index_enabled";

/// Sensitivitet per `SuggestionKind` (indeks = posisjon i `SuggestionKind::ALL`). Manglende innstilling = `Normal`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Sensitivities([Sensitivity; SuggestionKind::ALL.len()]);

impl Sensitivities {
    fn index(kind: SuggestionKind) -> usize {
        SuggestionKind::ALL
            .iter()
            .position(|k| *k == kind)
            .unwrap_or(0)
    }

    pub fn get(&self, kind: SuggestionKind) -> Sensitivity {
        self.0[Self::index(kind)]
    }

    pub fn set(&mut self, kind: SuggestionKind, value: Sensitivity) {
        self.0[Self::index(kind)] = value;
    }
}

/// Innstillinger. Nye felt har standardverdier slik at eldre databaser (uten nøklene) leses uendret:
/// `Settings { trash_days: n, ..Settings::default() }`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Settings {
    pub trash_days: u32,
    /// V1: sensitivitet per forslagstype.
    pub sensitivity: Sensitivities,
    /// V1: ansiktsindeks (personer). Av som standard; ingen ansiktsdata lages før brukeren slår det på.
    pub face_index: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            trash_days: 30,
            sensitivity: Sensitivities::default(),
            face_index: false,
        }
    }
}

/// Effektive terskler utledet av `Settings::sensitivity` (V1). Kjernen eier verdiene; UI viser dem bare.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Thresholds {
    /// Størst Hamming-avstand for visuelt duplikat.
    pub visual_max_hamming: u32,
    /// Uskarphetsscore under dette regnes som uskarpt.
    pub blur_max_score: f32,
    /// Størst tidsavstand (sekunder) mellom nabobilder i en serie.
    pub series_gap_secs: u32,
    /// Minste sikkerhet for å merke lukkede øyne.
    pub closed_eyes_min_confidence: f32,
}

impl Default for Thresholds {
    /// Dagens MVP-verdier (`cluster::Params`, `quality::BLUR_ABS_MAX`) pluss V1-startverdier som justeres i
    /// docs/v1/V1-PLAN.md først.
    fn default() -> Self {
        Thresholds {
            visual_max_hamming: crate::cluster::Params::default().max_dist,
            blur_max_score: crate::quality::BLUR_ABS_MAX,
            series_gap_secs: 10,
            closed_eyes_min_confidence: 0.8,
        }
    }
}

/// Én aktiv beskyttelse (`protection`-rad).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProtectionEntry {
    pub scope: ProtectScope,
    pub created_at: i64,
}

/// Regel slik UI sender den inn (`id = None` oppretter).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RuleDraft {
    pub id: Option<RuleId>,
    /// Brukerens eget navn.
    pub name: String,
    pub spec: RuleSpec,
}

/// Regel slik UI leser den.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RuleView {
    pub id: RuleId,
    pub name: String,
    pub spec: RuleSpec,
    pub enabled: bool,
    /// Antall bilder regelen treffer nå.
    pub hits: u64,
}

/// Forhåndsvisning av en regel før den lagres: ingenting endres.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct RulePreview {
    pub hits: u64,
    pub bytes: u64,
    /// Et lite utvalg treff (høyst 24) for miniatyrvisning.
    pub sample: Vec<AssetId>,
}

/// Person slik UI leser den (bare sammenslåtte «rot»-personer).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PersonView {
    pub id: PersonId,
    /// Brukerens eget navn; `None` til personen er navngitt.
    pub name: Option<String>,
    pub face_count: u64,
    pub asset_count: u64,
    /// Bilde som viser personen best (til avatar).
    pub cover: Option<AssetId>,
    pub protected: bool,
}

/// Ett medlem i serien, rangert (best først).
#[derive(Clone, Debug, PartialEq)]
pub struct SeriesMemberView {
    pub asset: AssetId,
    /// 1-basert, 1 = behold-kandidat.
    pub rank: u32,
    pub score: f32,
    pub is_keeper: bool,
    /// Forklaring på plasseringen (keeper: `Keeper*`; øvrige: `SeriesMember`, evt. `ClosedEyes`).
    pub reason: ReasonCode,
    pub detail: ReasonDetail,
    pub bytes: u64,
    pub protect_flags: ProtectFlags,
}

/// Side-ved-side-sammenligning av en serie.
#[derive(Clone, Debug, PartialEq)]
pub struct SeriesCompare {
    pub series: SeriesId,
    pub keeper: AssetId,
    pub members: Vec<SeriesMemberView>,
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
    /// Planen er for gammel, eller bildene er endret siden forhåndsvisningen.
    PlanStale,
    /// `begin_purge` krever at bildet er i «Nylig slettet».
    NotTrashed(AssetId),
    /// Valget ville fjernet alle aktive medlemmer i gruppen.
    WouldEmptyGroup(GroupId),
    Internal(String),
}

impl fmt::Display for ApiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for ApiError {}
