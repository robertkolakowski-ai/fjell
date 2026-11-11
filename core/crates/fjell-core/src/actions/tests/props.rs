//! Egenskapstester (proptest) for I4-I12 og I15. I1-I3, I5 og I14 ligger i `sim` (modelltest).

use super::*;
use crate::model::ProtectFlags;
use proptest::prelude::*;

fn cfg() -> ProptestConfig {
    ProptestConfig::with_cases(48)
}

/// Verden: n bilder (bytes 10+i), tilfeldige beskyttelsesflagg, grupper på 2+ og ventende forslag.
fn world(n: usize, flags: &[u8], gsizes: &[usize], sugg: &[bool]) -> (Fx, Vec<AssetId>) {
    let fx = Fx::new();
    let ids: Vec<_> = (0..n)
        .map(|i| fx.asset(i as u64 + 1, 10 + i as u64, flags[i] & 7, None))
        .collect();
    let (mut next, mut gid) = (0, 0);
    for &sz in gsizes {
        if sz < 2 || next + sz > n {
            continue;
        }
        gid += 1;
        fx.group(gid, &ids[next..next + sz]);
        for i in next..next + sz {
            if sugg[i] {
                fx.suggest(ids[i], "visual_dup", Some(gid));
            }
        }
        next += sz;
    }
    for i in next..n {
        if sugg[i] {
            fx.suggest(ids[i], "blurry", None);
        }
    }
    (fx, ids)
}

fn pick(ids: &[AssetId], mask: &[bool]) -> Vec<AssetId> {
    ids.iter()
        .zip(mask)
        .filter(|(_, m)| **m)
        .map(|(i, _)| *i)
        .collect()
}

fn spec() -> impl Strategy<Value = (usize, Vec<u8>, Vec<usize>, Vec<bool>, Vec<bool>)> {
    (2usize..14).prop_flat_map(|n| {
        (
            Just(n),
            prop::collection::vec(0u8..8, n),
            prop::collection::vec(0usize..5, 0..5),
            prop::collection::vec(any::<bool>(), n),
            prop::collection::vec(any::<bool>(), n),
        )
    })
}

proptest! {
    #![proptest_config(cfg())]

    /// I4: feil på vilkårlig rad i en batch etterlater ingenting (alt eller ingenting).
    #[test]
    fn i4_batch_is_atomic_under_injected_failure((n, fl, gs, sg, mask) in spec(), fail in 0usize..14) {
        let (mut fx, ids) = world(n, &fl, &gs, &sg);
        let sel = pick(&ids, &mask);
        prop_assume!(!sel.is_empty());
        let Ok(plan) = fx.a.plan(&PlanRequest::manual(&sel)) else { return Ok(()) };
        prop_assume!(!plan.items.is_empty());
        let victim = plan.items[fail % plan.items.len()].id;
        fx.a.store().conn().execute_batch(&format!(
            "CREATE TEMP TRIGGER inject BEFORE INSERT ON action_log WHEN NEW.asset_id = {} \
             BEGIN SELECT RAISE(ABORT, 'x'); END;", victim.0)).unwrap();
        let sum = dump_sum(&fx.a);
        prop_assert!(fx.a.commit(&plan, &acks_covering(&plan)).is_err());
        prop_assert_eq!(dump_sum(&fx.a), sum);
        // Samme for undo og restore midt i en batch.
        fx.a.store().conn().execute_batch("DROP TRIGGER inject").unwrap();
        fx.a.commit(&plan, &acks_covering(&plan)).unwrap();
        fx.a.store().conn().execute_batch(&format!(
            "CREATE TEMP TRIGGER inject BEFORE INSERT ON action_log WHEN NEW.asset_id = {} \
             BEGIN SELECT RAISE(ABORT, 'x'); END;", victim.0)).unwrap();
        let sum = dump_sum(&fx.a);
        prop_assert!(fx.a.undo().is_err());
        prop_assert_eq!(dump_sum(&fx.a), sum);
    }

    /// I6: dry-run har ingen bivirkning; `commit` endrer nøyaktig `plan.ids` og summene stemmer.
    #[test]
    fn i6_dry_run_and_apply_match((n, fl, gs, sg, _m) in spec()) {
        let (mut fx, ids) = world(n, &fl, &gs, &sg);
        let (ch, sum) = (fx.total_changes(), dump_sum(&fx.a));
        let plan = fx.a.plan_pending().unwrap();
        prop_assert_eq!((fx.total_changes(), dump_sum(&fx.a)), (ch, sum));
        fx.a.commit(&plan, &acks_covering(&plan)).unwrap();
        let changed: Vec<_> = ids.iter().copied().filter(|i| fx.state(*i) == State::Trashed).collect();
        prop_assert_eq!(&changed, &plan.ids());
        let bytes: u64 = changed.iter().map(|i| 10 + i.0 - 1).sum();
        prop_assert_eq!((plan.summary.count, plan.summary.bytes), (changed.len() as u64, bytes));
    }

    /// I7: beskyttet bilde trashes aldri uten dekkende ack; standardutvalget har aldri beskyttede.
    #[test]
    fn i7_protected_requires_matching_ack((n, fl, gs, sg, mask) in spec()) {
        let (mut fx, ids) = world(n, &fl, &gs, &sg);
        let pending = fx.a.plan_pending().unwrap();
        prop_assert!(pending.items.iter().all(|i| !i.protect.is_protected()));
        let sel = pick(&ids, &mask);
        let Ok(plan) = fx.a.plan(&PlanRequest::manual(&sel)) else { return Ok(()) };
        prop_assume!(!plan.items.is_empty());
        let mut want = Vec::new();
        for (flag, code) in crate::model::plan::ACK_CATEGORIES {
            if plan.items.iter().any(|i| i.protect.has(flag)) { want.push(code); }
        }
        let sum = dump_sum(&fx.a);
        let r = fx.a.commit(&plan, &Acks::default());
        if want.is_empty() {
            prop_assert!(r.is_ok());
        } else {
            match r { Err(ActionError::AckRequired(c)) => prop_assert_eq!(c, want), o => prop_assert!(false, "{o:?}") }
            prop_assert_eq!(dump_sum(&fx.a), sum);
            // Alle acks unntatt én kategori: akkurat den mangler.
            let all = acks_covering(&plan);
            let drop_cat = all.protected[0].category();
            let part = Acks { protected: all.protected.iter().filter(|a| a.category() != drop_cat).cloned().collect(), mass: None };
            match fx.a.commit(&plan, &part) {
                Err(ActionError::AckRequired(c)) => prop_assert_eq!(c.len(), 1),
                o if all.protected.len() == 1 => prop_assert!(o.is_err()),
                o => prop_assert!(false, "{o:?}"),
            }
        }
    }

    /// I8: «godta alle forslag» etterlater minst ett aktivt medlem per gruppe; å velge en hel gruppe avvises.
    #[test]
    fn i8_group_keeps_an_active_member((n, fl, gs, sg, _m) in spec()) {
        let (mut fx, ids) = world(n, &fl, &gs, &sg);
        let plan = fx.a.plan_pending().unwrap();
        fx.a.commit(&plan, &acks_covering(&plan)).unwrap();
        let groups = fx.sql_i64("SELECT COUNT(*) FROM grp");
        let alive = fx.sql_i64("SELECT COUNT(*) FROM grp g WHERE EXISTS (SELECT 1 FROM grp_member m JOIN asset a ON a.id = m.asset_id WHERE m.grp_id = g.id AND a.state = 0)");
        prop_assert_eq!(groups, alive);
        // Manuelt valg av alle aktive medlemmer i en gruppe.
        for g in 1..=groups {
            let members: Vec<AssetId> = fx.a.store().group_active_members(crate::model::GroupId(g as u64)).unwrap();
            if !members.is_empty() {
                prop_assert!(matches!(fx.a.plan(&PlanRequest::manual(&members)), Err(ActionError::WouldEmptyGroup(_))));
            }
        }
        let _ = ids;
    }

    /// I9 + I10: tid sletter ikke; varsler summerer til antall utløpte; utløpte kan gjenopprettes.
    #[test]
    fn i9_i10_time_never_purges_and_notifies_once((n, fl, gs, sg, mask) in spec(), ticks in prop::collection::vec(0i64..4000, 1..6)) {
        let (mut fx, ids) = world(n, &fl, &gs, &sg);
        let sel = pick(&ids, &mask);
        prop_assume!(!sel.is_empty());
        let Ok(plan) = fx.a.plan(&PlanRequest::manual(&sel)) else { return Ok(()) };
        fx.a.commit(&plan, &acks_covering(&plan)).unwrap();
        let (mut notified, mut total) = (0, 0i64);
        for t in ticks {
            fx.clock.advance(t * DAY);
            total += t;
            for _ in 0..3 { // gjentatte kall gir ikke nye varsler
                notified += fx.a.expire_due().unwrap().map_or(0, |n| n.count);
            }
            prop_assert!(fx.states(&ids).iter().all(|s| *s != State::Purged));
        }
        let expired = fx.sql_i64(&format!("SELECT COUNT(*) FROM asset WHERE state = 1 AND expires_at <= {}", fx.clock.now()));
        prop_assert_eq!(notified as i64, expired);
        prop_assert_eq!(expired, if total >= 30 { plan.items.len() as i64 } else { fx.sql_i64("SELECT COUNT(*) FROM asset WHERE state = 1 AND expired_notified_at IS NOT NULL") });
        let trashed: Vec<_> = ids.iter().copied().filter(|i| fx.state(*i) == State::Trashed).collect();
        fx.a.restore(&trashed).unwrap();
        prop_assert!(fx.states(&ids).iter().all(|s| *s == State::Active));
    }

    /// I11: trash/restore på riktig tilstand gir ingen loggrad; dobbel undo inverterer ikke dobbelt.
    #[test]
    fn i11_operations_are_idempotent((n, fl, gs, sg, mask) in spec()) {
        let (mut fx, ids) = world(n, &fl, &gs, &sg);
        let sel = pick(&ids, &mask);
        let Ok(plan) = fx.a.plan(&PlanRequest::manual(&sel)) else { return Ok(()) };
        prop_assume!(!plan.items.is_empty());
        fx.a.commit(&plan, &acks_covering(&plan)).unwrap();
        let logs = fx.log_count();
        let again = fx.a.plan(&PlanRequest::manual(&sel)).unwrap();
        prop_assert!(again.items.is_empty());
        prop_assert_eq!(fx.a.commit(&again, &Acks::default()).unwrap(), None);
        let active: Vec<_> = ids.iter().copied().filter(|i| fx.state(*i) == State::Active).collect();
        prop_assert_eq!(fx.a.restore(&active).unwrap(), None);
        prop_assert_eq!(fx.log_count(), logs);
        fx.a.undo().unwrap();
        let after_first = (fx.log_count(), dump_sum(&fx.a));
        // Andre undo har ingenting å angre (ingen suggestions-batch igjen): ingen dobbel inversjon.
        let second = fx.a.undo();
        prop_assert!(matches!(second, Err(ActionError::NothingToUndo)));
        prop_assert_eq!((fx.log_count(), dump_sum(&fx.a)), after_first);
        prop_assert!(fx.states(&ids).iter().all(|s| *s == State::Active));
    }

    /// I12: mengden endret mellom plan og commit (eller begin og confirm) gir PlanStale/PurgeStale.
    #[test]
    fn i12_confirmation_is_bound_to_the_set((n, fl, gs, sg, mask) in spec(), which in 0usize..14) {
        let (mut fx, ids) = world(n, &fl, &gs, &sg);
        let sel = pick(&ids, &mask);
        let Ok(plan) = fx.a.plan(&PlanRequest::manual(&sel)) else { return Ok(()) };
        prop_assume!(!plan.items.is_empty());
        let victim = plan.items[which % plan.items.len()].id;
        let acks = acks_covering(&plan);
        let bit = if plan.items[which % plan.items.len()].protect.has(ProtectFlags::EDITED) { 0 } else { ProtectFlags::EDITED };
        fx.a.store().conn().execute("UPDATE asset SET protect_flags = ?2 WHERE id = ?1",
            rusqlite::params![victim.0 as i64, bit]).unwrap();
        let sum = dump_sum(&fx.a);
        prop_assert!(matches!(fx.a.commit(&plan, &acks), Err(ActionError::PlanStale)));
        prop_assert_eq!(dump_sum(&fx.a), sum);
        // Ack fra en annen plan (annen digest) avvises også.
        let fresh = fx.a.plan(&PlanRequest::manual(&sel)).unwrap();
        prop_assert!(matches!(fx.a.commit(&fresh, &acks), Err(ActionError::PlanStale)) || fresh.digest == plan.digest);
        // Purge: mengden endres etter begin.
        fx.a.store().conn().execute("UPDATE asset SET protect_flags = 0", []).unwrap();
        fx.trash(&ids[..1]);
        let offer = fx.a.begin_purge(&ids[..1]).unwrap();
        fx.a.restore(&ids[..1]).unwrap();
        prop_assert!(matches!(fx.a.confirm_purge(&offer, None), Err(ActionError::PurgeStale)));
    }

    /// I15: delvis plattformresultat taper ingenting. `purged` = nøyaktig de som meldte `Deleted`.
    #[test]
    fn i15_partial_results_lose_nothing(outcomes in prop::collection::vec(0u8..3, 1..20)) {
        let mut fx = Fx::new();
        let ids = fx.assets(outcomes.len() as u64);
        fx.trash(&ids);
        let offer = fx.a.begin_purge(&ids).unwrap();
        let t = fx.a.confirm_purge(&offer, Some(&crate::model::MassAck::new(offer.digest))).unwrap();
        let res: Vec<_> = ids.iter().zip(&outcomes).map(|(&i, &o)| (i, match o {
            0 => DeleteOutcome::Deleted, 1 => DeleteOutcome::Cancelled, _ => DeleteOutcome::Failed })).collect();
        let out = fx.a.finish_purge(t, &res).unwrap();
        for (i, o) in ids.iter().zip(&outcomes) {
            prop_assert_eq!(fx.state(*i), if *o == 0 { State::Purged } else { State::Trashed });
        }
        prop_assert_eq!(out.purged.len() + out.kept_trashed.len(), ids.len());
        prop_assert_eq!(out.freed_bytes, out.purged.len() as u64 * 3);
    }
}
