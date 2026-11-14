//! `MemApi`: kjørbar mock av `FjellApi` i minnet (L4). Se docs/mvp/MVP-PLAN.md rad M1.
//!
//! Avhenger av: `crate::model`, `super` (trait og typer). Ingen indeks, ingen skanning.
//! Grensesnitt: `MemApi::new(cards, clock)`; oppfører seg som kjernen for trash/keep/angre/purge
//! slik at UI (`CoreBridge`) og CLI kan utvikles parallelt. Én batch = ett angre.
//! Kort for samme asset vises ikke samtidig; avgjorte kort forsvinner fra `cards`.

use super::{
    ApiError, Card, CardCursor, CardPage, Decision, FjellApi, HomeSummary, KindSummary,
    PurgeIntent, PurgeReport, ScanConfig, ScanPhase, ScanProgress, Settings, TrashSummary,
    UndoReport,
};
use crate::model::{
    AssetId, BatchId, CardId, Clock, Confirmed, DeleteOutcome, GroupId, PurgeTicket, State,
    SuggestionKind,
};
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, MutexGuard};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Status {
    Pending,
    Accepted,
    Rejected,
}

struct Entry {
    card: Card,
    status: Status,
}

#[derive(Default)]
struct Inner {
    entries: BTreeMap<CardId, Entry>,
    states: BTreeMap<AssetId, State>,
    batches: Vec<(BatchId, Vec<(CardId, Status)>)>,
    next_batch: u64,
    freed_bytes: u64,
    settings: Settings,
    scanning: bool,
}

/// Mock av kjernen. Seedes med ferdige kort.
pub struct MemApi {
    inner: Mutex<Inner>,
    clock: Arc<dyn Clock>,
}

fn digest(ids: &[AssetId]) -> u64 {
    // FNV-1a over sorterte id-er: bare for å binde bekreftelsen til mengden.
    let mut v: Vec<u64> = ids.iter().map(|a| a.0).collect();
    v.sort_unstable();
    v.iter()
        .flat_map(|x| x.to_le_bytes())
        .fold(0xcbf29ce484222325, |h, b| {
            (h ^ b as u64).wrapping_mul(0x100000001b3)
        })
}

impl MemApi {
    pub fn new(cards: Vec<Card>, clock: Arc<dyn Clock>) -> MemApi {
        let mut inner = Inner {
            settings: Settings::default(),
            ..Inner::default()
        };
        for c in cards {
            inner.states.insert(c.asset, State::Active);
            inner.entries.insert(
                c.id,
                Entry {
                    card: c,
                    status: Status::Pending,
                },
            );
        }
        MemApi {
            inner: Mutex::new(inner),
            clock,
        }
    }

    fn lock(&self) -> MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Tilstand for et asset (for tester).
    pub fn state_of(&self, id: AssetId) -> Option<State> {
        self.lock().states.get(&id).copied()
    }

    fn bytes_of(g: &Inner, id: AssetId) -> u64 {
        g.entries
            .values()
            .find(|e| e.card.asset == id)
            .map_or(0, |e| e.card.bytes)
    }
}

impl FjellApi for MemApi {
    fn start_scan(&self, _config: ScanConfig) -> Result<(), ApiError> {
        self.lock().scanning = true;
        Ok(())
    }

    fn cancel(&self) {
        self.lock().scanning = false;
    }

    fn cards(
        &self,
        kind: SuggestionKind,
        limit: usize,
        cursor: Option<CardCursor>,
    ) -> Result<CardPage, ApiError> {
        let g = self.lock();
        let from = cursor.map_or(0, |c| c.0);
        let mut it = g
            .entries
            .values()
            .filter(|e| e.status == Status::Pending && e.card.kind == kind && e.card.id.0 >= from)
            .map(|e| &e.card);
        let cards: Vec<Card> = it.by_ref().take(limit).cloned().collect();
        let next = it.next().map(|c| CardCursor(c.id.0));
        Ok(CardPage { cards, next })
    }

    fn decide_many(&self, decisions: Vec<(CardId, Decision)>) -> Result<BatchId, ApiError> {
        let mut g = self.lock();
        // Valider alt før noe endres: alt eller ingenting.
        for (id, _) in &decisions {
            match g.entries.get(id) {
                None => return Err(ApiError::UnknownCard(*id)),
                Some(e) if e.status != Status::Pending => return Err(ApiError::UnknownCard(*id)),
                Some(_) => {}
            }
        }
        g.next_batch += 1;
        let batch = BatchId(g.next_batch);
        let mut prev = Vec::new();
        for (id, d) in decisions {
            let asset = {
                let e = g.entries.get_mut(&id).expect("validated");
                prev.push((id, e.status));
                e.status = if d == Decision::Trash {
                    Status::Accepted
                } else {
                    Status::Rejected
                };
                e.card.asset
            };
            if d == Decision::Trash {
                g.states.insert(asset, State::Trashed);
            }
        }
        g.batches.push((batch, prev));
        Ok(batch)
    }

    fn set_keeper(&self, group: GroupId, keeper: AssetId) -> Result<(), ApiError> {
        let mut g = self.lock();
        if !g.states.contains_key(&keeper) {
            return Err(ApiError::UnknownAsset(keeper));
        }
        for e in g
            .entries
            .values_mut()
            .filter(|e| e.card.group == Some(group))
        {
            e.card.keeper = Some(keeper);
        }
        Ok(())
    }

    fn undo(&self) -> Result<UndoReport, ApiError> {
        let mut g = self.lock();
        let (batch, prev) = g.batches.pop().ok_or(ApiError::NothingToUndo)?;
        let mut restored = 0;
        for (id, status) in prev {
            let e = g.entries.get_mut(&id).expect("batch entries exist");
            let asset = e.card.asset;
            e.status = status;
            if g.states.get(&asset) == Some(&State::Trashed) {
                g.states.insert(asset, State::Active);
                restored += 1;
            }
        }
        Ok(UndoReport { batch, restored })
    }

    fn begin_purge(&self, ids: Vec<AssetId>) -> Result<PurgeIntent, ApiError> {
        let g = self.lock();
        let mut bytes = 0;
        for id in &ids {
            match g.states.get(id) {
                None => return Err(ApiError::UnknownAsset(*id)),
                Some(State::Purged) => return Err(ApiError::AlreadyPurged(*id)),
                Some(State::Active) => return Err(ApiError::NotUndoable),
                Some(State::Trashed) => bytes += Self::bytes_of(&g, *id),
            }
        }
        Ok(PurgeIntent {
            digest: digest(&ids),
            ids,
            bytes,
        })
    }

    fn confirm_purge(&self, intent: &PurgeIntent) -> Result<PurgeTicket, ApiError> {
        if digest(&intent.ids) != intent.digest {
            return Err(ApiError::PurgeStale);
        }
        let g = self.lock();
        if intent
            .ids
            .iter()
            .any(|i| g.states.get(i) != Some(&State::Trashed))
        {
            return Err(ApiError::PurgeStale);
        }
        Ok(PurgeTicket::new(
            Confirmed::new(intent.digest),
            intent.ids.clone(),
            self.clock.now(),
        ))
    }

    fn finish_purge(
        &self,
        ticket: PurgeTicket,
        results: Vec<(AssetId, DeleteOutcome)>,
    ) -> Result<PurgeReport, ApiError> {
        let mut g = self.lock();
        let (mut purged, mut kept_trashed) = (Vec::new(), Vec::new());
        for (id, outcome) in results {
            if !ticket.ids().contains(&id) {
                return Err(ApiError::PurgeStale);
            }
            if outcome == DeleteOutcome::Deleted && g.states.get(&id) == Some(&State::Trashed) {
                g.states.insert(id, State::Purged);
                g.freed_bytes += Self::bytes_of(&g, id);
                purged.push(id);
            } else {
                kept_trashed.push(id);
            }
        }
        Ok(PurgeReport {
            purged,
            kept_trashed,
        })
    }

    fn home_summary(&self) -> HomeSummary {
        let (per_kind, reclaimable, scanning) = {
            let g = self.lock();
            let per_kind: Vec<KindSummary> = SuggestionKind::MVP
                .iter()
                .map(|k| {
                    let mut s = KindSummary {
                        kind: *k,
                        count: 0,
                        bytes: 0,
                    };
                    for e in g
                        .entries
                        .values()
                        .filter(|e| e.status == Status::Pending && e.card.kind == *k)
                    {
                        s.count += 1;
                        s.bytes += e.card.bytes;
                    }
                    s
                })
                .collect();
            let total = per_kind.iter().map(|s| s.bytes).sum();
            (per_kind, total, g.scanning)
        };
        let phase = if scanning {
            ScanPhase::Analyzing
        } else {
            ScanPhase::Idle
        };
        HomeSummary {
            per_kind,
            reclaimable_bytes: reclaimable,
            trash: self.trash_summary(),
            scan: ScanProgress {
                phase,
                done: 0,
                total: 0,
                found_exact: 0,
            },
        }
    }

    fn trash_summary(&self) -> TrashSummary {
        let g = self.lock();
        let mut t = TrashSummary {
            freed_bytes: g.freed_bytes,
            ..TrashSummary::default()
        };
        for (id, st) in &g.states {
            if *st == State::Trashed {
                t.count += 1;
                t.bytes += Self::bytes_of(&g, *id);
            }
        }
        t
    }

    fn settings(&self) -> Settings {
        self.lock().settings
    }

    fn set_settings(&self, settings: Settings) {
        self.lock().settings = settings;
    }
}

#[cfg(test)]
#[path = "mock_tests.rs"]
mod tests;
