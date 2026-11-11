//! Modellbasert test (PLAN-TRYGGHET del 5): en referansemodell (tilstand + forslagsstatus + angre-stabel)
//! kjøres parallelt med ekte `actions`. Etter hver operasjon sammenlignes alt. Dekker I1, I2, I3,
//! I5, I9, I10, I14 og I15 for vilkårlige operasjonsrekker.
//!
//! Operasjoner: Trash, Restore, Keep, Undo, Tick, ScanChange, DryRun, Purge (BeginPurge, ConfirmPurge,
//! FinishPurge(utfall), avbrutt bekreftelse, CrashBeforeDb) og SetTrashDays.

use super::*;
use crate::model::{MassAck, SuggestionKind};
use proptest::prelude::*;
use std::collections::HashSet;

const N: usize = 8;

#[derive(Clone, Debug)]
pub(super) enum Op {
    Trash(Vec<bool>),
    Restore(Vec<bool>),
    Keep(Vec<bool>),
    Undo,
    Tick(i64),
    ScanChange(Vec<bool>),
    DryRun,
    Purge {
        mask: Vec<bool>,
        outcomes: Vec<u8>,
        crash: bool,
        confirm: bool,
    },
    SetTrashDays(u32),
}

fn mask() -> impl Strategy<Value = Vec<bool>> {
    prop::collection::vec(any::<bool>(), N)
}

pub(super) fn op() -> impl Strategy<Value = Op> {
    prop_oneof![
        3 => mask().prop_map(Op::Trash),
        2 => mask().prop_map(Op::Restore),
        1 => mask().prop_map(Op::Keep),
        3 => Just(Op::Undo),
        1 => (0i64..60).prop_map(Op::Tick),
        1 => mask().prop_map(Op::ScanChange),
        1 => Just(Op::DryRun),
        2 => (mask(), prop::collection::vec(0u8..3, N), any::<bool>(), prop::bool::weighted(0.8))
            .prop_map(|(mask, outcomes, crash, confirm)| Op::Purge { mask, outcomes, crash, confirm }),
        1 => (1u32..60).prop_map(Op::SetTrashDays),
    ]
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Sg {
    Pending,
    Accepted,
    Rejected,
}

#[derive(Clone, Copy, Debug)]
enum Entry {
    Trash(usize, Sg),
    Restore(usize),
    Keep(usize, Sg),
}

struct Batch {
    entries: Vec<Entry>,
    has_purge: bool,
}

pub(super) struct Sim {
    pub(super) fx: Fx,
    pub(super) ids: Vec<AssetId>,
    st: Vec<State>,
    sg: Vec<Sg>,
    stack: Vec<Batch>,
    gone: Vec<usize>,
    pub(super) plat: FakePlatform,
    confirmed: HashSet<u64>,
}

fn sg_of(s: &str) -> Sg {
    match s {
        "pending" => Sg::Pending,
        "accepted" => Sg::Accepted,
        _ => Sg::Rejected,
    }
}

impl Sim {
    pub(super) fn new() -> Sim {
        let fx = Fx::new();
        let ids = fx.assets(N as u64);
        for id in &ids {
            fx.suggest(*id, "blurry", None);
        }
        Sim {
            fx,
            ids,
            st: vec![State::Active; N],
            sg: vec![Sg::Pending; N],
            stack: vec![],
            gone: vec![],
            plat: FakePlatform::default(),
            confirmed: HashSet::new(),
        }
    }

    fn sel(&self, m: &[bool]) -> Vec<usize> {
        (0..N).filter(|&i| m[i]).collect()
    }

    fn ids_of(&self, ix: &[usize]) -> Vec<AssetId> {
        ix.iter().map(|&i| self.ids[i]).collect()
    }

    fn push(&mut self, entries: Vec<Entry>, has_purge: bool) {
        if !entries.is_empty() || has_purge {
            self.stack.push(Batch { entries, has_purge });
        }
    }

    pub(super) fn apply(&mut self, op: &Op) {
        match op {
            Op::Trash(m) => {
                let ix = self.sel(m);
                let req = PlanRequest {
                    items: ix
                        .iter()
                        .map(|&i| Selection {
                            id: self.ids[i],
                            kind: (i % 2 == 0).then_some(SuggestionKind::Blurry),
                            reason: None,
                        })
                        .collect(),
                };
                let r = self.fx.a.plan(&req);
                if ix.iter().any(|&i| self.st[i] == State::Purged) {
                    assert!(matches!(r, Err(ActionError::AlreadyPurged(_))));
                    return;
                }
                let plan = r.unwrap();
                let b = self.fx.a.commit(&plan, &acks_covering(&plan)).unwrap();
                let mut entries = vec![];
                let active: Vec<usize> = ix
                    .iter()
                    .copied()
                    .filter(|&i| self.st[i] == State::Active)
                    .collect();
                for i in active {
                    entries.push(Entry::Trash(i, self.sg[i]));
                    self.st[i] = State::Trashed;
                    if i % 2 == 0 {
                        self.sg[i] = Sg::Accepted;
                    }
                }
                assert_eq!(b.is_some(), !entries.is_empty());
                self.push(entries, false);
            }
            Op::Restore(m) => {
                let ix = self.sel(m);
                let r = self.fx.a.restore(&self.ids_of(&ix));
                if ix.iter().any(|&i| self.st[i] == State::Purged) {
                    assert!(matches!(r, Err(ActionError::AlreadyPurged(_))));
                    return;
                }
                let entries: Vec<_> = ix
                    .iter()
                    .copied()
                    .filter(|&i| self.st[i] == State::Trashed)
                    .map(Entry::Restore)
                    .collect();
                assert_eq!(r.unwrap().is_some(), !entries.is_empty());
                for e in &entries {
                    if let Entry::Restore(i) = e {
                        self.st[*i] = State::Active;
                    }
                }
                self.push(entries, false);
            }
            Op::Keep(m) => {
                let ix = self.sel(m);
                let b = self.fx.a.keep(&self.ids_of(&ix)).unwrap();
                let mut entries = vec![];
                let pending: Vec<usize> = ix
                    .into_iter()
                    .filter(|&i| self.sg[i] == Sg::Pending)
                    .collect();
                for i in pending {
                    entries.push(Entry::Keep(i, Sg::Pending));
                    self.sg[i] = Sg::Rejected;
                }
                assert_eq!(b.is_some(), !entries.is_empty());
                self.push(entries, false);
            }
            Op::Undo => match (self.fx.a.undo(), self.stack.last()) {
                (Err(ActionError::NothingToUndo), None) => {}
                (Err(ActionError::NotUndoable), Some(b)) if b.has_purge => {}
                (Ok(u), Some(b)) if !b.has_purge => {
                    let b = self.stack.pop().unwrap();
                    let (mut applied, mut skipped) = (0, vec![]);
                    for e in b.entries {
                        match e {
                            Entry::Trash(i, prev) if self.st[i] == State::Trashed => {
                                self.st[i] = State::Active;
                                self.sg[i] = prev;
                                applied += 1;
                            }
                            Entry::Restore(i) if self.st[i] == State::Active => {
                                self.st[i] = State::Trashed;
                                applied += 1;
                            }
                            Entry::Keep(i, prev) => {
                                self.sg[i] = prev;
                                applied += 1;
                            }
                            Entry::Trash(i, _) | Entry::Restore(i) => skipped.push(self.ids[i]),
                        }
                    }
                    assert_eq!((u.restored, u.skipped), (applied, skipped));
                }
                (r, top) => panic!(
                    "undo: {r:?}, modellens øverste batch har purge = {:?}",
                    top.map(|b| b.has_purge)
                ),
            },
            Op::Tick(d) => {
                self.fx.clock.advance(d * DAY);
                self.fx.a.expire_due().unwrap();
            }
            Op::ScanChange(m) => {
                let mut ix = self.sel(m);
                ix.append(&mut self.gone);
                let removed: Vec<_> = ix.iter().map(|i| SourceId(format!("s{}", i + 1))).collect();
                self.fx
                    .a
                    .store_mut()
                    .reconcile_removed(&removed, self.fx.clock.now())
                    .unwrap();
                for i in ix {
                    self.st[i] = State::Purged;
                }
            }
            Op::DryRun => {
                let (ch, sum) = (self.fx.total_changes(), dump_sum(&self.fx.a));
                self.fx.a.plan_pending().unwrap();
                assert_eq!((self.fx.total_changes(), dump_sum(&self.fx.a)), (ch, sum));
            }
            Op::Purge {
                mask,
                outcomes,
                crash,
                confirm,
            } => {
                let ix: Vec<_> = self
                    .sel(mask)
                    .into_iter()
                    .filter(|&i| self.st[i] == State::Trashed)
                    .collect();
                if ix.is_empty() {
                    return;
                }
                let offer = self.fx.a.begin_purge(&self.ids_of(&ix)).unwrap();
                if !confirm {
                    return; // Aborted: bruker avbrøt før bekreftelse. Ingen billett, ingen delete.
                }
                let t = self
                    .fx
                    .a
                    .confirm_purge(&offer, Some(&MassAck::new(offer.digest)))
                    .unwrap();
                self.confirmed.insert(t.confirmed().digest());
                let pre = self.plat.call_count();
                self.plat
                    .delete_checked(&self.fx.a, &t, DeleteOutcome::Deleted)
                    .unwrap();
                assert_eq!(self.plat.call_count(), pre + 1);
                let res: Vec<_> = ix
                    .iter()
                    .map(|&i| {
                        (
                            self.ids[i],
                            match outcomes[i] {
                                0 => DeleteOutcome::Deleted,
                                1 => DeleteOutcome::Cancelled,
                                _ => DeleteOutcome::Failed,
                            },
                        )
                    })
                    .collect();
                let deleted: Vec<_> = ix.iter().copied().filter(|&i| outcomes[i] == 0).collect();
                if *crash {
                    self.gone.extend(deleted); // plattformen slettet, DB ble aldri skrevet
                    return;
                }
                let out = self.fx.a.finish_purge(t, &res).unwrap();
                assert_eq!(out.purged, self.ids_of(&deleted));
                for &i in &deleted {
                    self.st[i] = State::Purged;
                }
                if !deleted.is_empty() {
                    self.push(vec![], true);
                }
            }
            Op::SetTrashDays(d) => {
                let before = self
                    .fx
                    .sql_i64("SELECT COALESCE(SUM(expires_at), 0) FROM asset");
                self.fx.a.set_trash_days(*d).unwrap();
                assert_eq!(
                    before,
                    self.fx
                        .sql_i64("SELECT COALESCE(SUM(expires_at), 0) FROM asset"),
                    "eksisterende frister skal ikke endres"
                );
            }
        }
    }

    fn check(&self) {
        for i in 0..N {
            assert_eq!(
                self.fx.state(self.ids[i]),
                self.st[i],
                "tilstand for bilde {}",
                i + 1
            );
            let s: String = self
                .fx
                .a
                .store()
                .conn()
                .query_row(
                    "SELECT status FROM suggestion WHERE asset_id = ?1",
                    [self.ids[i].0 as i64],
                    |r| r.get(0),
                )
                .unwrap();
            assert_eq!(sg_of(&s), self.sg[i], "forslagsstatus for bilde {}", i + 1);
        }
        let ts = self.fx.a.trash_summary().unwrap();
        assert_eq!(
            ts.trashed_count,
            self.st.iter().filter(|s| **s == State::Trashed).count() as u64
        );
        // I1: `delete` kalles bare med billett fra `confirm_purge`.
        assert!(self
            .plat
            .calls
            .lock()
            .unwrap()
            .iter()
            .all(|(d, _)| self.confirmed.contains(d)));
    }
}

fn log_hash(fx: &Fx, up_to_id: i64) -> u64 {
    let c = fx.a.store().conn();
    let mut st = c.prepare("SELECT id, batch_id, op, asset_id, prev_state, inverse_of, cause FROM action_log WHERE id <= ?1 ORDER BY id").unwrap();
    let mut rows = st.query([up_to_id]).unwrap();
    let mut h = 14695981039346656037u64;
    while let Some(r) = rows.next().unwrap() {
        for i in 0..7 {
            let v: rusqlite::types::Value = r.get(i).unwrap();
            for b in format!("{v:?}").bytes() {
                h = (h ^ b as u64).wrapping_mul(1099511628211);
            }
        }
    }
    h
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(96))]

    /// Referansemodell og ekte `actions` er like etter hver operasjon i vilkårlige rekker.
    #[test]
    fn model_matches_real_after_every_op(ops in prop::collection::vec(op(), 1..40)) {
        let mut sim = Sim::new();
        let mut purged_once: HashSet<usize> = HashSet::new();
        for o in &ops {
            let max_before = sim.fx.sql_i64("SELECT COALESCE(MAX(id), 0) FROM action_log");
            let hash_before = log_hash(&sim.fx, max_before);
            sim.apply(o);
            // I14: alt som fantes før er bit-for-bit likt etter.
            prop_assert_eq!(log_hash(&sim.fx, max_before), hash_before);
            sim.check();
            // I3: purged er terminal.
            for i in 0..N {
                if purged_once.contains(&i) { prop_assert_eq!(sim.st[i], State::Purged); }
                if sim.st[i] == State::Purged { purged_once.insert(i); }
            }
        }
    }

    /// I2: uten finish_purge/Deleted eller ekstern sletting oppstår aldri `purged`.
    #[test]
    fn i2_no_purged_without_confirmed_or_external(ops in prop::collection::vec(op(), 1..40)) {
        let mut sim = Sim::new();
        let ops: Vec<_> = ops.into_iter().filter(|o| !matches!(o, Op::Purge { .. } | Op::ScanChange(_))).collect();
        for o in &ops { sim.apply(o); }
        prop_assert!(sim.ids.iter().all(|i| sim.fx.state(*i) != State::Purged));
        prop_assert_eq!(sim.fx.sql_i64("SELECT COUNT(*) FROM action_log WHERE op = 'purge'"), 0);
        prop_assert_eq!(sim.plat.call_count(), 0);
    }

    /// I5: alle batcher angret i omvendt rekkefølge gir opprinnelig tilstand, `trashed_at`, `expires_at`
    /// og forslagsstatus (sekvenser uten purge og ekstern endring).
    #[test]
    fn i5_undo_roundtrip(ops in prop::collection::vec(op(), 1..30)) {
        let mut sim = Sim::new();
        let snap = |s: &Sim| -> String {
            let c = s.fx.a.store().conn();
            let mut st = c.prepare("SELECT a.id, a.state, a.trashed_at, a.expires_at, g.status FROM asset a JOIN suggestion g ON g.asset_id = a.id ORDER BY a.id").unwrap();
            let rows: Vec<String> = st.query_map([], |r| Ok(format!("{:?}", (r.get::<_, i64>(0)?, r.get::<_, i64>(1)?, r.get::<_, Option<i64>>(2)?, r.get::<_, Option<i64>>(3)?, r.get::<_, String>(4)?)))).unwrap().map(|r| r.unwrap()).collect();
            rows.join("\n")
        };
        let before = snap(&sim);
        for o in ops.iter().filter(|o| matches!(o, Op::Trash(_) | Op::Restore(_) | Op::Keep(_) | Op::Tick(_) | Op::SetTrashDays(_))) {
            sim.apply(o);
        }
        while sim.fx.a.undo().is_ok() {}
        prop_assert_eq!(snap(&sim), before);
    }
}
