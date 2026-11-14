//! Serialiserbare typer for Tauri-grensen (frontend ser bare disse, aldri kjernetyper).
//!
//! Avhenger av: `fjell-core::api` (les-/beslutningstyper), serde.
//! Grensesnitt: `StateDto`, `CardDto`, `CardPageDto`, `ErrorDto`, `DecisionDto`, `kind_code`.
//! V1-typene (beskyttelse, regler, personer, dashboard, serier, innstillinger) ligger i `dto_v1`.
//! Kjernen returnerer koder, aldri tekst (ARKITEKTUR beslutning 13): alle strenger her er
//! stabile koder (snake_case) som UI slår opp i nøkkelkatalogen.

use crate::dto_v1::DetailDto;
use fjell_core::api::{ApiError, Card, CardPage, HomeSummary, ScanPhase, Settings, TrashSummary};
use fjell_core::model::{ConsequenceCode, SuggestionKind};
use fjell_fs::DeleteError;
use serde::{Deserialize, Serialize};

/// Stabil kode for forslagstype.
pub fn kind_code(k: SuggestionKind) -> &'static str {
    match k {
        SuggestionKind::ExactDup => "exact_dup",
        SuggestionKind::VisualDup => "visual_dup",
        SuggestionKind::Blurry => "blurry",
        SuggestionKind::Screenshot => "screenshot",
        SuggestionKind::Series => "series",
        SuggestionKind::Rule => "rule",
    }
}

/// Motsatt vei; ukjent kode gir `None` (kommandoen svarer da med `bad_request`).
pub fn parse_kind(code: &str) -> Option<SuggestionKind> {
    SuggestionKind::ALL
        .into_iter()
        .find(|k| kind_code(*k) == code)
}

/// Motsatt vei for konsekvenskoder (`ack_required` gir dem; UI sender dem tilbake som bekreftelse).
pub fn parse_consequence(code: &str) -> Option<ConsequenceCode> {
    ConsequenceCode::ALL.into_iter().find(|c| c.key() == code)
}

fn phase_code(p: ScanPhase) -> &'static str {
    match p {
        ScanPhase::Idle => "idle",
        ScanPhase::Listing => "listing",
        ScanPhase::Exact => "exact",
        ScanPhase::ExactReady => "exact_ready",
        ScanPhase::Analyzing => "analyzing",
        ScanPhase::Done => "done",
    }
}

#[derive(Serialize, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct KindDto {
    pub kind: &'static str,
    pub count: u64,
    pub bytes: u64,
}

/// Tilstandssiden: oppsummering av forslag, «Nylig slettet» og skanning.
#[derive(Serialize, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct StateDto {
    /// `"mock"` eller `"core"`: hvilken `FjellApi` skallet er koblet til.
    pub backend: &'static str,
    pub per_kind: Vec<KindDto>,
    pub reclaimable_bytes: u64,
    pub trash_count: u64,
    pub trash_bytes: u64,
    pub trash_expired: u64,
    pub freed_bytes: u64,
    pub scan_phase: &'static str,
    pub scan_done: u64,
    pub scan_total: u64,
    pub trash_days: u32,
    /// Skann satt på pause av brukeren (avbrutt, gjenopptas med `resume_scan`).
    pub scan_paused: bool,
    /// Leverandørkode (`icloud`/`onedrive`/`dropbox`) hvis HELE biblioteket (alle røtter) ligger i
    /// skysynkede mapper, altså når all papirkurv er sperret. Én skysynket rot blant flere står i
    /// `roots[].cloud_sync`, og sperrer bare bildene i den roten.
    pub cloud_sync: Option<&'static str>,
    /// Bibliotekets røtter i rekkefølge (første er `primary`). Uten tall og uten filsystemtilgang
    /// her (`online`/`count`/… er `None`); `list_library_roots` fyller dem ut.
    pub roots: Vec<RootDto>,
}

/// Én rot (mappe) i biblioteket. `key` er stabil og brukes i `remove_library_root`.
#[derive(Serialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RootDto {
    pub key: u32,
    pub path: String,
    /// Første rot: eier indeksen og kan ikke fjernes (bytt bibliotek med `set_library`).
    pub primary: bool,
    /// Leverandørkode hvis roten ligger i en skysynket mappe: papirkurv er sperret for dens bilder.
    pub cloud_sync: Option<&'static str>,
    /// Rotens egen karantenemappe (`<rot>/.fjell-karantene`).
    pub quarantine_dir: String,
    /// Mappen finnes nå (ekstern disk tilkoblet). Bare fra `list_library_roots`.
    pub online: Option<bool>,
    /// Aktive filer (bilder og video) i indeksen, og samlet størrelse. Bare fra `list_library_roots`.
    pub count: Option<u64>,
    pub bytes: Option<u64>,
    /// Herav video (telles og listes, dekodes aldri).
    pub videos: Option<u64>,
    pub video_bytes: Option<u64>,
    /// Antall i «Nylig slettet» fra denne roten. Over 0: roten kan ikke fjernes.
    pub trashed: Option<u64>,
}

impl StateDto {
    pub fn from_core(backend: &'static str, h: &HomeSummary, s: &Settings) -> StateDto {
        StateDto {
            backend,
            per_kind: h
                .per_kind
                .iter()
                .map(|k| KindDto {
                    kind: kind_code(k.kind),
                    count: k.count,
                    bytes: k.bytes,
                })
                .collect(),
            reclaimable_bytes: h.reclaimable_bytes,
            trash_count: h.trash.count,
            trash_bytes: h.trash.bytes,
            trash_expired: h.trash.expired,
            freed_bytes: h.trash.freed_bytes,
            scan_phase: phase_code(h.scan.phase),
            scan_done: h.scan.done,
            scan_total: h.scan.total,
            trash_days: s.trash_days,
            scan_paused: false,
            cloud_sync: None,
            roots: Vec::new(),
        }
    }
}

/// Miniatyr til frontend: JPEG som base64 uten `data:`-prefiks (UI bygger `data:`-URL).
#[derive(Serialize, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ThumbDto {
    pub mime: &'static str,
    pub data: String,
    pub width: u32,
    pub height: u32,
}

#[derive(Serialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CardDto {
    pub id: u64,
    pub kind: &'static str,
    pub asset: u64,
    pub group: Option<u64>,
    pub keeper: Option<u64>,
    /// Begrunnelseskode (variantnavn i kjernen, f.eks. `ExactCopy`).
    pub reason: String,
    /// Tall/ID bak begrunnelsesmerket (serieposisjon, regel, antall lukkede øyne).
    pub detail: DetailDto,
    pub confidence: f32,
    pub bytes: u64,
    pub protect_flags: u8,
}

impl From<&Card> for CardDto {
    fn from(c: &Card) -> CardDto {
        CardDto {
            id: c.id.0,
            kind: kind_code(c.kind),
            asset: c.asset.0,
            group: c.group.map(|g| g.0),
            keeper: c.keeper.map(|a| a.0),
            reason: format!("{:?}", c.reason),
            detail: c.detail.into(),
            confidence: c.confidence,
            bytes: c.bytes,
            protect_flags: c.protect_flags.0,
        }
    }
}

#[derive(Serialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CardPageDto {
    pub cards: Vec<CardDto>,
    pub next: Option<u64>,
}

impl From<&CardPage> for CardPageDto {
    fn from(p: &CardPage) -> CardPageDto {
        CardPageDto {
            cards: p.cards.iter().map(CardDto::from).collect(),
            next: p.next.map(|c| c.0),
        }
    }
}

/// Beslutning fra UI.
#[derive(Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DecisionDto {
    Trash,
    Keep,
}

/// Feil på grensen: kode + valgfri detalj (aldri brukertekst).
#[derive(Serialize, Debug, PartialEq, Eq)]
pub struct ErrorDto {
    pub code: &'static str,
    pub detail: String,
}

impl ErrorDto {
    pub fn new(code: &'static str, detail: impl Into<String>) -> ErrorDto {
        ErrorDto {
            code,
            detail: detail.into(),
        }
    }

    pub fn bad_request(detail: impl Into<String>) -> ErrorDto {
        ErrorDto {
            code: "bad_request",
            detail: detail.into(),
        }
    }
}

impl From<ApiError> for ErrorDto {
    fn from(e: ApiError) -> ErrorDto {
        if let ApiError::AckRequired(codes) = &e {
            // Detalj = konsekvenskodene (kommaseparert), slik at UI kan vise og sende dem tilbake.
            let keys: Vec<&str> = codes.iter().map(|c| c.key()).collect();
            return ErrorDto::new("ack_required", keys.join(","));
        }
        let code = match &e {
            ApiError::AckRequired(_) => "ack_required",
            ApiError::WouldEmptyGroup(_) => "would_empty_group",
            ApiError::UnknownCard(_) => "unknown_card",
            ApiError::UnknownAsset(_) => "unknown_asset",
            ApiError::NothingToUndo => "nothing_to_undo",
            ApiError::NotUndoable => "not_undoable",
            ApiError::AlreadyPurged(_) => "already_purged",
            ApiError::PurgeStale => "purge_stale",
            ApiError::PlanStale => "plan_stale",
            ApiError::NotTrashed(_) => "not_trashed",
            _ => "core_error",
        };
        // `core_error` med `Internal(kode)`: detaljen er kjernens stabile feilkode (f.eks. `empty_rule`).
        let detail = match &e {
            ApiError::Internal(c) => c.clone(),
            other => format!("{other:?}"),
        };
        ErrorDto { code, detail }
    }
}

impl From<DeleteError> for ErrorDto {
    fn from(e: DeleteError) -> ErrorDto {
        match e {
            DeleteError::AckRequired => ErrorDto::new("ack_required", "permanent"),
            DeleteError::AckMismatch => ErrorDto::new("ack_mismatch", "permanent"),
        }
    }
}

/// «Nylig slettet»: merket / kan frigjøres / frigjort.
#[derive(Serialize, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TrashSummaryDto {
    pub count: u64,
    pub bytes: u64,
    pub expired: u64,
    pub freed_bytes: u64,
    pub trash_days: u32,
}

impl TrashSummaryDto {
    pub fn from_core(t: &TrashSummary, s: &Settings) -> TrashSummaryDto {
        TrashSummaryDto {
            count: t.count,
            bytes: t.bytes,
            expired: t.expired,
            freed_bytes: t.freed_bytes,
            trash_days: s.trash_days,
        }
    }
}

/// Brukerens ekstra bekreftelse for permanent sletting (avkrysning + antallet på knappen).
#[derive(Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PermanentAckDto {
    pub checked: bool,
    pub count: usize,
}

/// Resultat av «Tøm». Alt som ikke er `purged` ligger fortsatt i «Nylig slettet» (ingen tap).
#[derive(Serialize, Debug, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub struct EmptyDto {
    pub purged: u64,
    pub kept_trashed: u64,
    /// Volum uten Papirkurv: ikke slettet, UI kan tilby permanent med egen advarsel.
    pub unsupported: u64,
    pub cancelled: u64,
    pub unavailable: u64,
    pub freed_bytes: u64,
}

/// Ett bilde i «Nylig slettet». Tider i sekunder siden epoch. `name` er bare filnavnet (ingen sti).
#[derive(Serialize, Debug, PartialEq, Eq, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TrashedDto {
    pub asset: u64,
    pub name: String,
    pub bytes: u64,
    pub trashed_at: i64,
    pub expires_at: i64,
    pub expired: bool,
}

/// Resultat av «Gjenopprett». Bare bilder som lå i «Nylig slettet» telles.
#[derive(Serialize, Debug, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub struct RestoreDto {
    pub restored: u64,
}

/// Innstillinger skallet viser. `quarantine_dir` er `None` til en mappe er valgt.
/// `empty_default` er alltid `"recycle"`: «Tøm» går via Windows Papirkurv, permanent har eget ark.
#[derive(Serialize, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SettingsDto {
    pub trash_days: u32,
    pub quarantine_dir: Option<String>,
    pub empty_default: &'static str,
}
