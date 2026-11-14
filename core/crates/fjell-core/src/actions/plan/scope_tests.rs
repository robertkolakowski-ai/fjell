//! Tester for beskyttelse via omfang (`protection`) i `plan` og `plan_pending`.
//!
//! Dekker: konsekvenskode og telling, `plan_pending` foreslår aldri beskyttede bilder,
//! og en egenskapstest over tilfeldige beskyttelser (deterministisk frø).

use super::super::tests::Fx;
use super::*;
use crate::model::{ConsequenceCode, ProtectScope};

fn protect(fx: &Fx, scope: ProtectScope) {
    assert!(fx.a.store().protection_add(&scope, 1).unwrap());
}

#[test]
fn plan_counts_scope_protected_and_adds_code() {
    let fx = Fx::new();
    let ids = fx.assets(3);
    protect(&fx, ProtectScope::Asset(ids[1]));
    let plan = fx.a.plan(&PlanRequest::manual(&ids)).unwrap();
    assert_eq!(plan.summary.protected.scope, 1);
    assert!(plan
        .summary
        .codes
        .contains(&ConsequenceCode::ProtectedByScope));
}

#[test]
fn plan_without_scope_protection_has_no_scope_code() {
    let fx = Fx::new();
    let ids = fx.assets(2);
    let plan = fx.a.plan(&PlanRequest::manual(&ids)).unwrap();
    assert_eq!(plan.summary.protected.scope, 0);
    assert!(!plan
        .summary
        .codes
        .contains(&ConsequenceCode::ProtectedByScope));
}

#[test]
fn plan_pending_skips_album_and_asset_protected() {
    let fx = Fx::new();
    let ids = fx.assets(4);
    for id in &ids {
        fx.suggest(*id, "blurry", None);
    }
    fx.album(1, "Bryllup", false, &[ids[2]]);
    protect(&fx, ProtectScope::Album(1));
    protect(&fx, ProtectScope::Asset(ids[0]));
    let plan = fx.a.plan_pending().unwrap();
    assert_eq!(plan.ids(), vec![ids[1], ids[3]]);
    assert_eq!(plan.summary.protected.scope, 0);
}

#[test]
fn unprotect_makes_pending_visible_again() {
    let fx = Fx::new();
    let ids = fx.assets(2);
    for id in &ids {
        fx.suggest(*id, "blurry", None);
    }
    let sc = ProtectScope::Asset(ids[0]);
    protect(&fx, sc.clone());
    assert_eq!(fx.a.plan_pending().unwrap().ids(), vec![ids[1]]);
    fx.a.store().protection_remove(&sc).unwrap();
    assert_eq!(fx.a.plan_pending().unwrap().ids(), ids);
}

#[test]
fn plan_pending_never_includes_scope_protected_property() {
    // Deterministisk LCG: 200 runder med tilfeldige beskyttelser over 12 bilder i 3 mapper.
    let mut x: u64 = 0x9e37_79b9_7f4a_7c15;
    let mut next = move |m: u64| {
        x = x
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        (x >> 33) % m
    };
    for _ in 0..200 {
        let fx = Fx::new();
        let ids = fx.assets(12);
        // Mappestruktur via source_id: d0/.., d1/.., d2/..
        for (i, id) in ids.iter().enumerate() {
            fx.a.store()
                .conn()
                .execute(
                    "UPDATE asset SET source_id = ?1 WHERE id = ?2",
                    rusqlite::params![format!("d{}/f{i}", i % 3), id.0 as i64],
                )
                .unwrap();
            fx.suggest(*id, "blurry", None);
        }
        fx.album(1, "A", false, &ids[0..4]);
        let mut scopes = Vec::new();
        for _ in 0..next(4) {
            scopes.push(match next(3) {
                0 => ProtectScope::Asset(ids[next(12) as usize]),
                1 => ProtectScope::Folder(format!("d{}", next(3))),
                _ => ProtectScope::Album(1),
            });
        }
        for s in &scopes {
            fx.a.store().protection_add(s, 1).unwrap();
        }
        let protected: std::collections::HashSet<_> =
            fx.a.store()
                .protected_by_scope(&ids)
                .unwrap()
                .into_iter()
                .map(|(id, _)| id)
                .collect();
        let plan = fx.a.plan_pending().unwrap();
        for id in plan.ids() {
            assert!(!protected.contains(&id), "{id:?} er beskyttet");
        }
        assert_eq!(plan.ids().len(), 12 - protected.len());
    }
}
