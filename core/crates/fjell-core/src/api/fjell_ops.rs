//! `impl FjellApi for Fjell`: kort, beslutninger, angre og purge (L4). Del av `api`.
//!
//! Avhenger av: `super::fjell` (`Fjell`, feilmapping), actions (plan/commit/purge), store, pipeline.
//! Grensesnitt: kun `FjellApi`. Reglene fra PLAN-TRYGGHET gjelder uendret:
//! - `decide_many`: alle valg i én batch (ett angre). Beskyttede bilder og massesletting gir
//!   `AckRequired(koder)` uten at noe skrives; `decide_many_confirmed` med kodene gir bekreftelsen.
//!   Acks lages bare her (`acks_covering`), og bare for kodene brukeren faktisk har bekreftet.
//! - Purge: `begin_purge` → `confirm_purge` (`Confirmed`/`PurgeTicket` lages bare i actions) →
//!   plattformens `delete` (utenfor kjernen) → `finish_purge`. Ingen annen vei til `purged`.
//! - Samme bilde vises aldri i flere kort (prioritet exact_dup > visual_dup > screenshot > blurry).

use super::fjell::{act_err, internal, Fjell};
use super::{
    ApiError, Card, CardCursor, CardPage, Decision, FjellApi, HomeSummary, KindSummary,
    PurgeIntent, PurgeReport, ScanConfig, Settings, TrashSummary, UndoReport,
};
use crate::actions::{acks_covering, PlanRequest, Selection};
use crate::model::plan::ACK_CATEGORIES;
use crate::model::{
    AssetId, BatchId, CardId, ConsequenceCode, DeleteOutcome, GroupId, MassAck, PurgeOffer,
    PurgeTicket, ReasonCode, ReasonDetail, SuggestionKind,
};
use crate::store::CardRow;
use std::collections::BTreeSet;

fn encode(conf: f32, id: u64) -> CardCursor {
    CardCursor(((conf.to_bits() as u64) << 32) | (id & 0xFFFF_FFFF))
}

fn decode(c: CardCursor) -> (f32, u64) {
    (f32::from_bits((c.0 >> 32) as u32), c.0 & 0xFFFF_FFFF)
}

fn default_reason(kind: SuggestionKind) -> ReasonCode {
    match kind {
        SuggestionKind::ExactDup => ReasonCode::ExactCopy,
        SuggestionKind::VisualDup => ReasonCode::VisualDuplicate,
        SuggestionKind::Blurry => ReasonCode::Blurry,
        SuggestionKind::Screenshot => ReasonCode::Screenshot,
        SuggestionKind::Series => ReasonCode::SeriesMember,
        SuggestionKind::Rule => ReasonCode::Rule,
    }
}

fn card(kind: SuggestionKind, r: CardRow) -> Card {
    Card {
        id: CardId(r.id),
        kind,
        asset: r.asset,
        group: r.group,
        keeper: r.keeper,
        reason: ReasonCode::ALL
            .into_iter()
            .find(|c| c.key() == r.reason_code)
            .unwrap_or_else(|| default_reason(kind)),
        // Fylles av integrasjonsfasen (serie-posisjon, regel, person).
        detail: ReasonDetail::None,
        confidence: r.confidence,
        bytes: r.bytes,
        protect_flags: r.protect,
    }
}

/// Kodene som krever brukerens bekreftelse før `commit`/`confirm_purge`.
fn needs_ack(c: ConsequenceCode) -> bool {
    c == ConsequenceCode::MassDelete || ACK_CATEGORIES.iter().any(|(_, code)| *code == c)
}

impl Fjell {
    fn decide(
        &self,
        decisions: Vec<(CardId, Decision)>,
        confirmed: &[ConsequenceCode],
    ) -> Result<BatchId, ApiError> {
        if decisions.is_empty() {
            return Err(ApiError::Internal("tom beslutningsliste".into()));
        }
        let ids: Vec<u64> = decisions.iter().map(|(c, _)| c.0).collect();
        let rows = self
            .actions()
            .store()
            .suggestion_rows(&ids)
            .map_err(internal)?;
        let mut seen = BTreeSet::new();
        let (mut trash, mut keep) = (Vec::new(), Vec::new());
        for ((card, d), row) in decisions.iter().zip(rows) {
            // Alt eller ingenting: ukjent eller allerede avgjort kort stopper hele runden.
            let row = row
                .filter(|r| r.pending)
                .ok_or(ApiError::UnknownCard(*card))?;
            if !seen.insert(*card) {
                continue;
            }
            match d {
                Decision::Trash => trash.push(Selection {
                    id: row.asset,
                    kind: Some(row.kind),
                    reason: None,
                }),
                Decision::Keep => keep.push(row.asset),
            }
        }
        // En asset kan ikke både slettes og beholdes i samme runde: sletting vinner ikke, avvis.
        if keep.iter().any(|k| trash.iter().any(|t| t.id == *k)) {
            return Err(ApiError::Internal(
                "samme bilde både slettet og beholdt".into(),
            ));
        }
        let batch = {
            let mut a = self.actions();
            let plan = a.plan(&PlanRequest { items: trash }).map_err(act_err)?;
            let needed: Vec<ConsequenceCode> = plan
                .summary
                .codes
                .iter()
                .copied()
                .filter(|c| needs_ack(*c) && !confirmed.contains(c))
                .collect();
            if !needed.is_empty() {
                return Err(ApiError::AckRequired(needed));
            }
            let mut acks = acks_covering(&plan);
            if !confirmed.contains(&ConsequenceCode::MassDelete) {
                acks.mass = None;
            }
            a.commit_with_keeps(&plan, &acks, &keep).map_err(act_err)?
        };
        self.refresh()?;
        batch.ok_or_else(|| ApiError::Internal("ingen endring".into()))
    }

    fn confirm(
        &self,
        intent: &PurgeIntent,
        confirmed: &[ConsequenceCode],
    ) -> Result<PurgeTicket, ApiError> {
        let offer = PurgeOffer {
            ids: intent.ids.clone(),
            bytes: intent.bytes,
            expired_count: 0,
            digest: intent.digest,
            codes: Vec::new(),
        };
        let mass = confirmed
            .contains(&ConsequenceCode::MassDelete)
            .then(|| MassAck::new(intent.digest));
        self.actions()
            .confirm_purge(&offer, mass.as_ref())
            .map_err(act_err)
    }
}

impl FjellApi for Fjell {
    fn start_scan(&self, config: ScanConfig) -> Result<(), ApiError> {
        self.start(config)
    }

    fn cancel(&self) {
        self.sh
            .cancel
            .store(true, std::sync::atomic::Ordering::SeqCst);
    }

    fn cards(
        &self,
        kind: SuggestionKind,
        limit: usize,
        cursor: Option<CardCursor>,
    ) -> Result<CardPage, ApiError> {
        let limit = limit.max(1);
        let after = cursor.map(decode);
        let mut rows = self
            .actions()
            .store()
            .pending_cards(kind, limit + 1, after)
            .map_err(internal)?;
        let more = rows.len() > limit;
        rows.truncate(limit);
        let next = more
            .then(|| rows.last().map(|r| encode(r.confidence, r.id)))
            .flatten();
        Ok(CardPage {
            cards: rows.into_iter().map(|r| card(kind, r)).collect(),
            next,
        })
    }

    fn decide_many(&self, decisions: Vec<(CardId, Decision)>) -> Result<BatchId, ApiError> {
        self.decide(decisions, &[])
    }

    fn decide_many_confirmed(
        &self,
        decisions: Vec<(CardId, Decision)>,
        confirmed: Vec<ConsequenceCode>,
    ) -> Result<BatchId, ApiError> {
        self.decide(decisions, &confirmed)
    }

    fn set_keeper(&self, group: GroupId, keeper: AssetId) -> Result<(), ApiError> {
        let ok = self
            .actions()
            .store_mut()
            .set_group_keeper(group, keeper)
            .map_err(internal)?;
        if ok {
            Ok(())
        } else {
            Err(ApiError::UnknownAsset(keeper))
        }
    }

    fn undo(&self) -> Result<UndoReport, ApiError> {
        let out = self.actions().undo().map_err(act_err)?;
        self.refresh()?;
        Ok(UndoReport {
            batch: out.undone,
            restored: out.restored,
        })
    }

    fn begin_purge(&self, ids: Vec<AssetId>) -> Result<PurgeIntent, ApiError> {
        let offer = self.actions().begin_purge(&ids).map_err(act_err)?;
        Ok(PurgeIntent {
            ids: offer.ids,
            bytes: offer.bytes,
            digest: offer.digest,
        })
    }

    fn confirm_purge(&self, intent: &PurgeIntent) -> Result<PurgeTicket, ApiError> {
        self.confirm(intent, &[])
    }

    fn confirm_purge_confirmed(
        &self,
        intent: &PurgeIntent,
        confirmed: Vec<ConsequenceCode>,
    ) -> Result<PurgeTicket, ApiError> {
        self.confirm(intent, &confirmed)
    }

    fn finish_purge(
        &self,
        ticket: PurgeTicket,
        results: Vec<(AssetId, DeleteOutcome)>,
    ) -> Result<PurgeReport, ApiError> {
        let out = self
            .actions()
            .finish_purge(ticket, &results)
            .map_err(act_err)?;
        self.refresh()?;
        Ok(PurgeReport {
            purged: out.purged,
            kept_trashed: out.kept_trashed,
        })
    }

    fn home_summary(&self) -> HomeSummary {
        let totals = self.actions().store().kind_totals().unwrap_or_default();
        let per_kind: Vec<KindSummary> = SuggestionKind::MVP
            .iter()
            .map(|k| {
                let (count, bytes) = totals
                    .iter()
                    .find(|t| t.0 == *k)
                    .map_or((0, 0), |t| (t.1, t.2));
                KindSummary {
                    kind: *k,
                    count,
                    bytes,
                }
            })
            .collect();
        HomeSummary {
            reclaimable_bytes: per_kind.iter().map(|k| k.bytes).sum(),
            per_kind,
            trash: self.trash_summary(),
            scan: self.scan_progress(),
        }
    }

    fn trash_summary(&self) -> TrashSummary {
        match self.actions().trash_summary() {
            Ok(t) => TrashSummary {
                count: t.trashed_count,
                bytes: t.trashed_bytes,
                expired: t.expired_count,
                freed_bytes: t.freed_bytes,
            },
            Err(_) => TrashSummary::default(),
        }
    }

    fn settings(&self) -> Settings {
        Settings {
            trash_days: self.actions().trash_days().unwrap_or(30),
            // Sensitivitet og ansiktsindeks leses/lagres i integrasjonsfasen (standardverdier til da).
            ..Settings::default()
        }
    }

    fn set_settings(&self, settings: Settings) {
        // Feil her gir uendret innstilling; `settings()` viser det som gjelder.
        let _ = self.actions().set_trash_days(settings.trash_days);
    }
}
