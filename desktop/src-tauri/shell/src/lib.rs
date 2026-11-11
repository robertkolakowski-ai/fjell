//! Kommandologikk for desktop-skallet, uavhengig av Tauri (testbar uten webkit/WebView2).
//!
//! Avhenger av: `fjell-core::api` (`FjellApi`, `MemApi`), `dto`.
//! Grensesnitt: `Shell::mock()`, `Shell::new(api, backend)`, `state`, `cards`, `decide`, `undo`,
//! `start_scan`. Tauri-kommandoene i `fjell-desktop` er tynne omslag rundt disse.
//!
//! Merk (beslutning 17): ingen sletting her. `begin_purge`/`confirm_purge`/`delete` og
//! karantene kobles på i senere steg (adapter `fjell-fs`), aldri direkte fra frontend.

pub mod dto;
#[cfg(test)]
mod tests;

use dto::{parse_kind, CardPageDto, DecisionDto, ErrorDto, StateDto};
use fjell_core::api::{Card, CardCursor, Decision, FjellApi, MemApi, ScanConfig, ScanMode};
use fjell_core::model::{
    AssetId, CardId, Clock, GroupId, ProtectFlags, ReasonCode, SuggestionKind,
};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

struct SysClock;
impl Clock for SysClock {
    fn now(&self) -> i64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_secs() as i64)
    }
}

/// Skallets tilstand: én `FjellApi` bak en `Arc`.
pub struct Shell {
    api: Arc<dyn FjellApi>,
    backend: &'static str,
}

impl Shell {
    pub fn new(api: Arc<dyn FjellApi>, backend: &'static str) -> Shell {
        Shell { api, backend }
    }

    /// Mot `MemApi` med noen få seedede kort (M5: ekte kjerne kobles på senere).
    pub fn mock() -> Shell {
        Shell::new(
            Arc::new(MemApi::new(seed_cards(), Arc::new(SysClock))),
            "mock",
        )
    }

    pub fn state(&self) -> StateDto {
        StateDto::from_core(self.backend, &self.api.home_summary(), &self.api.settings())
    }

    pub fn start_scan(&self, full: bool) -> Result<(), ErrorDto> {
        let mode = if full {
            ScanMode::Full
        } else {
            ScanMode::Quick
        };
        self.api
            .start_scan(ScanConfig {
                mode,
                priority: 1,
                deadline: None,
            })
            .map_err(ErrorDto::from)
    }

    pub fn cards(
        &self,
        kind: &str,
        limit: usize,
        cursor: Option<u64>,
    ) -> Result<CardPageDto, ErrorDto> {
        let kind = parse_kind(kind).ok_or_else(|| ErrorDto::bad_request("unknown kind"))?;
        let limit = limit.clamp(1, 200);
        let page = self.api.cards(kind, limit, cursor.map(CardCursor))?;
        Ok(CardPageDto::from(&page))
    }

    /// Returnerer batch-id (ett angre). Bekreftelser (`AckRequired`) kommer i UX-fasen.
    pub fn decide(&self, ids: &[u64], decision: DecisionDto) -> Result<u64, ErrorDto> {
        let d = match decision {
            DecisionDto::Trash => Decision::Trash,
            DecisionDto::Keep => Decision::Keep,
        };
        let batch = self
            .api
            .decide_many(ids.iter().map(|i| (CardId(*i), d)).collect())?;
        Ok(batch.0)
    }

    pub fn undo(&self) -> Result<u64, ErrorDto> {
        Ok(self.api.undo()?.batch.0)
    }
}

fn seed_cards() -> Vec<Card> {
    let mk = |n: u64, kind, reason, bytes| Card {
        id: CardId(n),
        kind,
        asset: AssetId(n),
        group: (kind == SuggestionKind::ExactDup).then_some(GroupId(1)),
        keeper: (kind == SuggestionKind::ExactDup).then_some(AssetId(100)),
        reason,
        confidence: 1.0,
        bytes,
        protect_flags: ProtectFlags::default(),
    };
    vec![
        mk(
            1,
            SuggestionKind::ExactDup,
            ReasonCode::ExactCopy,
            2_400_000,
        ),
        mk(
            2,
            SuggestionKind::ExactDup,
            ReasonCode::ExactCopy,
            3_100_000,
        ),
        mk(3, SuggestionKind::Blurry, ReasonCode::Blurry, 1_800_000),
        mk(
            4,
            SuggestionKind::Screenshot,
            ReasonCode::Screenshot,
            600_000,
        ),
    ]
}
