//! `impl FjellApi for Fjell`: kort, beslutninger, angre og purge (L4). Del av `api`.
//!
//! Avhenger av: `super::fjell` (`Fjell`, feilmapping), actions (plan/commit/purge), store, pipeline.
//! Grensesnitt: kun `FjellApi`. Reglene fra PLAN-TRYGGHET gjelder uendret:
//! - `decide_many`: alle valg i én batch (ett angre). Beskyttede bilder og massesletting gir
//!   `AckRequired(koder)` uten at noe skrives; `decide_many_confirmed` med kodene gir bekreftelsen.
//!   Acks lages bare her (`acks_covering`), og bare for kodene brukeren faktisk har bekreftet.
//! - V1: beskyttelse via `protection` (`ProtectedByScope`) krever bekreftelse ved både `decide_many` og
//!   `confirm_purge` (også om beskyttelsen kom etter at bildet havnet i «Nylig slettet»).
//! - Purge: `begin_purge` → `confirm_purge` (`Confirmed`/`PurgeTicket` lages bare i actions) →
//!   plattformens `delete` (utenfor kjernen) → `finish_purge`. Ingen annen vei til `purged`.
//! - Samme bilde vises aldri i flere kort (prioritet exact_dup > visual_dup > screenshot > blurry).

use super::fjell::{act_err, internal, Fjell};
use super::fjell_v1::RuleOp;
use super::{
    ApiError, Card, CardCursor, CardPage, Decision, FjellApi, HomeSummary, KindSummary, PersonView,
    ProtectionEntry, PurgeIntent, PurgeReport, RuleDraft, RulePreview, RuleView, ScanConfig,
    SeriesCompare, Settings, Thresholds, TrashSummary, UndoReport,
};
use crate::actions::{acks_covering, PlanRequest, Selection};
use crate::dashboard::Dashboard;
use crate::model::plan::ACK_CATEGORIES;
use crate::model::{
    AssetId, BatchId, CardId, ConsequenceCode, DashboardData, DeleteOutcome, GroupId, MassAck,
    PersonId, ProtectScope, PurgeOffer, PurgeTicket, ReasonCode, RuleId, RuleSpec, SeriesId,
    SuggestionKind,
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

fn reason_of(kind: SuggestionKind, r: &CardRow) -> ReasonCode {
    ReasonCode::ALL
        .into_iter()
        .find(|c| c.key() == r.reason_code)
        .unwrap_or_else(|| default_reason(kind))
}

/// Kodene som krever brukerens bekreftelse før `commit`/`confirm_purge`.
fn needs_ack(c: ConsequenceCode) -> bool {
    // V1: beskyttelse per bilde, mappe, album eller person krever alltid egen bekreftelse. `actions::commit`
    // kjenner bare plattformflaggene, så porten for `ProtectedByScope` ligger her (eneste vei til commit).
    c == ConsequenceCode::MassDelete
        || c == ConsequenceCode::ProtectedByScope
        || ACK_CATEGORIES.iter().any(|(_, code)| *code == c)
}

impl Fjell {
    /// Gjenoppretter bilder fra «Nylig slettet» (én batch, ett angre). Aktive bilder hoppes over;
    /// `None` hvis ingenting ble endret. Filflytting ut av karantenen gjør kalleren (skallet).
    pub fn restore_assets(&self, ids: &[AssetId]) -> Result<Option<BatchId>, ApiError> {
        let batch = self.actions().restore(ids).map_err(act_err)?;
        self.refresh()?;
        Ok(batch)
    }

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
        // Et bilde kan ha blitt beskyttet etter at det ble flyttet til «Nylig slettet»: da trengs egen bekreftelse.
        if !confirmed.contains(&ConsequenceCode::ProtectedByScope)
            && !self
                .actions()
                .store()
                .protected_by_scope(&intent.ids)
                .map_err(internal)?
                .is_empty()
        {
            return Err(ApiError::AckRequired(vec![
                ConsequenceCode::ProtectedByScope,
            ]));
        }
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
            .pending_cards(kind, limit.saturating_add(1), after)
            .map_err(internal)?;
        let more = rows.len() > limit;
        rows.truncate(limit);
        let next = more
            .then(|| rows.last().map(|r| encode(r.confidence, r.id)))
            .flatten();
        let a = self.actions();
        let cards = rows
            .into_iter()
            .map(|r| {
                let reason = reason_of(kind, &r);
                Card {
                    id: CardId(r.id),
                    kind,
                    asset: r.asset,
                    group: r.group,
                    keeper: r.keeper,
                    reason,
                    detail: self.v1_card_detail(a.store(), kind, reason, &r),
                    confidence: r.confidence,
                    bytes: r.bytes,
                    protect_flags: r.protect,
                }
            })
            .collect();
        Ok(CardPage { cards, next })
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
        let per_kind: Vec<KindSummary> = SuggestionKind::ALL
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
        self.v1_settings()
    }

    fn set_settings(&self, settings: Settings) {
        self.v1_set_settings(settings);
    }

    fn protect_set(&self, scope: ProtectScope) -> Result<(), ApiError> {
        self.v1_protect(scope, true)
    }

    fn unprotect(&self, scope: ProtectScope) -> Result<(), ApiError> {
        self.v1_protect(scope, false)
    }

    fn protections(&self) -> Result<Vec<ProtectionEntry>, ApiError> {
        self.v1_protections()
    }

    fn rules_list(&self) -> Result<Vec<RuleView>, ApiError> {
        self.v1_rules_list()
    }

    fn rule_save(&self, draft: RuleDraft) -> Result<RuleId, ApiError> {
        self.v1_rule_save(draft)
    }

    fn rule_preview(&self, spec: RuleSpec) -> Result<RulePreview, ApiError> {
        self.v1_rule_preview(spec)
    }

    fn rule_toggle(&self, id: RuleId, enabled: bool) -> Result<(), ApiError> {
        self.v1_rule_change(id, RuleOp::Enabled(enabled))
    }

    fn rule_delete(&self, id: RuleId) -> Result<(), ApiError> {
        self.v1_rule_change(id, RuleOp::Delete)
    }

    fn rule_restore(&self, id: RuleId) -> Result<(), ApiError> {
        self.v1_rule_change(id, RuleOp::Restore)
    }

    fn people_list(&self) -> Result<Vec<PersonView>, ApiError> {
        self.v1_people_list()
    }

    fn person_name(&self, id: PersonId, name: Option<String>) -> Result<(), ApiError> {
        self.v1_person_name(id, name)
    }

    fn person_merge(&self, target: PersonId, absorbed: PersonId) -> Result<PersonId, ApiError> {
        self.v1_person_merge(target, absorbed)
    }

    fn person_unmerge(&self, absorbed: PersonId) -> Result<(), ApiError> {
        self.v1_person_unmerge(absorbed)
    }

    fn person_protect(&self, id: PersonId, protect: bool) -> Result<(), ApiError> {
        self.v1_protect(ProtectScope::Person(id), protect)
    }

    fn face_index_enabled(&self) -> bool {
        self.v1_face_index_enabled()
    }

    fn face_index_set(&self, enabled: bool) -> Result<(), ApiError> {
        self.v1_face_index_set(enabled)
    }

    fn face_index_wipe(&self) -> Result<u64, ApiError> {
        self.v1_face_index_wipe()
    }

    fn dashboard(&self) -> Result<DashboardData, ApiError> {
        self.v1_dashboard()
    }

    fn dashboard_full(&self) -> Result<Dashboard, ApiError> {
        self.v1_dashboard_full()
    }

    fn series_compare(&self, series: SeriesId) -> Result<SeriesCompare, ApiError> {
        self.v1_series_compare(series)
    }

    fn thresholds(&self) -> Thresholds {
        self.v1_thresholds()
    }
}
