//! Restanse 7: eldre indeks (lavere `ALGO_VER`) reindekseres automatisk ved neste skann.
//! Avledede verdier (hasher, `blur_score`) regnes om; beslutninger, papirkurv og angre røres ikke.

use super::*;
use crate::model::AlgoVer;
use crate::store::{BatchSpec, Cause, RowOp, UndoResult};

fn corpus() -> Fx {
    let mut fx = Fx::new();
    fx.corpus();
    fx
}

type Fp = (String, i64, i64, Option<f64>, i64, i64);

/// Fingeravtrykk per kildenavn: (navn, dhash, phash, blur, signaler, algo_ver).
fn fingerprints(fx: &Fx) -> Vec<Fp> {
    fx.with(|s| {
        let mut st = s
            .conn()
            .prepare(
                "SELECT a.source_id, f.dhash, f.phash, f.blur_score, f.shot_signals, f.algo_ver \
                 FROM asset a JOIN fingerprint f ON f.asset_id = a.id ORDER BY a.source_id",
            )
            .unwrap();
        let rows = st
            .query_map([], |r| {
                Ok((
                    r.get(0)?,
                    r.get(1)?,
                    r.get(2)?,
                    r.get(3)?,
                    r.get(4)?,
                    r.get(5)?,
                ))
            })
            .unwrap()
            .map(|r| r.unwrap())
            .collect();
        rows
    })
}

fn trash(fx: &Fx, names: &[&str]) {
    let ops = names
        .iter()
        .map(|n| RowOp::Trash {
            id: fx.id(n),
            kind: None,
            expires_at: i64::MAX / 2,
        })
        .collect();
    fx.with(|s| {
        s.apply_batch(&BatchSpec {
            ts: 100,
            cause: Cause::User,
            ops,
        })
    })
    .unwrap()
    .unwrap();
}

fn state_of(fx: &Fx, name: &str) -> i64 {
    let id = fx.id(name).0 as i64;
    fx.with(|s| {
        s.conn()
            .query_row("SELECT state FROM asset WHERE id = ?1", [id], |r| r.get(0))
            .unwrap()
    })
}

#[test]
fn r7_old_algo_ver_index_is_migrated_and_decisions_survive() {
    let fx = corpus();
    assert_eq!(fx.scan_algo(1).analyzed, 28);
    let fresh = fingerprints(&fx);

    // Etterlign en indeks laget av en eldre algoritme: forskjellige verdier, versjon 1.
    fx.with(|s| {
        s.conn()
            .execute(
                "UPDATE fingerprint SET blur_score = 999.0, dhash = 0, phash = 0",
                [],
            )
            .unwrap();
    });
    // Brukeren har besluttet og slettet to bilder (de ligger i papirkurven, ikke analysert på nytt).
    trash(&fx, &["u00.jpg", "b0.jpg"]);
    let before_reads = fx.src.counts().1;

    // Koden har nå versjon 2: alle aktive rader regnes om (26 aktive av 28).
    let r = fx.scan_algo(2);
    assert_eq!(r.analyzed, 26);
    assert!(r.failures.is_empty());
    assert_eq!(
        fx.src.counts().1,
        before_reads,
        "originalene leses ikke ved reindeksering"
    );

    let after = fingerprints(&fx);
    for (old, new) in fresh.iter().zip(&after) {
        assert_eq!(old.0, new.0);
        if old.0 == "u00.jpg" || old.0 == "b0.jpg" {
            // I papirkurven: urørt, fortsatt gammel versjon.
            assert_eq!((new.1, new.5), (0, 1), "{}", old.0);
        } else {
            assert_eq!(
                (old.1, old.2, old.3, old.4),
                (new.1, new.2, new.3, new.4),
                "{}",
                old.0
            );
            assert_eq!(new.5, 2, "{}", old.0);
        }
    }
    // Beslutningene er bevart.
    assert_ne!(state_of(&fx, "u00.jpg"), 0);
    assert_ne!(state_of(&fx, "b0.jpg"), 0);
    assert_eq!(state_of(&fx, "u01.jpg"), 0);
    // Innstillingen følger koden, og neste skann gjør ingenting.
    assert_eq!(
        fx.with(|s| s.setting("algo_ver")).unwrap().as_deref(),
        Some("2")
    );
    assert_eq!(fx.scan_algo(2).analyzed, 0);

    // Angre fungerer fortsatt etter migreringen.
    let undo = fx.with(|s| s.undo_top(200, 30 * 86_400)).unwrap();
    assert!(
        matches!(undo, UndoResult::Done { applied: 2, .. }),
        "{undo:?}"
    );
    assert_eq!(state_of(&fx, "u00.jpg"), 0);
    assert_eq!(state_of(&fx, "b0.jpg"), 0);
    // De gjenopprettede radene har gammel versjon og tas med ved neste skann.
    assert_eq!(fx.scan_algo(2).analyzed, 2);
    assert_eq!(
        fingerprints(&fx),
        fresh
            .iter()
            .map(|f| (f.0.clone(), f.1, f.2, f.3, f.4, 2))
            .collect::<Vec<Fp>>()
    );
    let _ = AlgoVer(2);
}
