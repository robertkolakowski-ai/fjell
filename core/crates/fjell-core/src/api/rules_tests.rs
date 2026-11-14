//! Tester for brukerstyrte keeper-regler gjennom `Fjell`: regler, beskyttelse, forhåndsvisning,
//! «bruk regler», determinisme, migrasjon og at K5 er uendret uten regler.

use super::*;
use crate::model::{FakeClock, SuggestionKind};
use crate::pipeline::tests::{meta, scene, thumb, Counting, Fx};
use crate::pipeline::{compute, compute_with_rules, GroupEngine, RuleCtx};
use crate::rules::{KeeperRule, RuleReason};
use crate::store::{index_rows_on, source_ids_on, Store};
use std::sync::Arc;

/// Eksakte par: (navn, navn, seed). Første navn er eldst (kortere navn gir lavere `created_at`),
/// så K5 velger alltid første navn.
const PAIRS: [(&str, &str, u64); 4] = [
    ("orig/a.jpg", "kopi/aa.jpg", 11),
    ("privat/bbb.jpg", "kopi/b.jpg", 12), // K5: kopi/b.jpg (eldst); privat er lengst
    ("x/c.jpg", "y/cc.jpg", 13),
    ("privat/d.jpg", "privat/dd.jpg", 14), // begge beskyttet
];

fn fixture() -> Arc<Counting> {
    let mut fx = Fx::new();
    for (a, b, seed) in PAIRS {
        let px = scene(seed);
        let bytes = vec![seed as u8; 4_000 + seed as usize];
        fx.put(a, px.clone(), bytes.clone(), "t1");
        fx.put(b, px, bytes, "t1");
    }
    for i in 0..4u64 {
        fx.uniq(&format!("u/u{i}.jpg"), scene(900 + i));
    }
    let _ = (meta, thumb);
    fx.src.clone()
}

fn open() -> Fjell {
    Fjell::new(
        Store::open_in_memory().unwrap(),
        fixture(),
        Arc::new(FakeClock::new(5_000_000)),
    )
}

fn scan(f: &Fjell) {
    f.start_scan(ScanConfig {
        mode: ScanMode::Full,
        priority: 1,
        deadline: None,
    })
    .unwrap();
    f.wait_scan().unwrap().unwrap();
    f.sync_groups().unwrap();
}

fn fold(prefix: &str) -> KeeperRule {
    KeeperRule::PreferFolder {
        prefix: prefix.into(),
        priority: 0,
    }
}

type GroupRow = (String, String, Vec<String>, Option<String>, Option<String>);

/// (kind, keeper-navn, medlemsnavn, keeper_reason, keeper_rule) for alle grupper, sortert.
fn groups(f: &Fjell) -> Vec<GroupRow> {
    f.with_store(|s| {
        let c = s.conn();
        let mut st = c
            .prepare(
                "SELECT g.id, g.kind, k.source_id, g.keeper_reason, g.keeper_rule FROM grp g \
                 JOIN asset k ON k.id = g.keeper_id ORDER BY g.id",
            )
            .unwrap();
        #[allow(clippy::type_complexity)]
        let rows: Vec<(i64, String, String, Option<String>, Option<String>)> = st
            .query_map([], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?))
            })
            .unwrap()
            .map(Result::unwrap)
            .collect();
        let mut out: Vec<_> = rows
            .into_iter()
            .map(|(id, kind, keeper, reason, rule)| {
                let mut m = c
                    .prepare(
                        "SELECT a.source_id FROM grp_member m JOIN asset a ON a.id = m.asset_id \
                         WHERE m.grp_id = ?1 ORDER BY a.source_id",
                    )
                    .unwrap();
                let names: Vec<String> = m
                    .query_map([id], |r| r.get(0))
                    .unwrap()
                    .map(Result::unwrap)
                    .collect();
                (kind, keeper, names, reason, rule)
            })
            .collect();
        out.sort();
        out
    })
}

fn group_of<'a>(g: &'a [GroupRow], name: &str) -> Option<&'a GroupRow> {
    g.iter().find(|x| x.2.iter().any(|n| n == name))
}

/// Hele tilstanden en forhåndsvisning ikke skal røre.
fn snapshot(f: &Fjell) -> Vec<String> {
    f.with_store(|s| {
        let c = s.conn();
        let mut out = Vec::new();
        for sql in [
            "SELECT id||'|'||kind||'|'||COALESCE(keeper_id,'')||'|'||COALESCE(keeper_reason,'')||'|'||COALESCE(keeper_rule,'') FROM grp",
            "SELECT grp_id||'|'||asset_id||'|'||rank FROM grp_member",
            "SELECT id||'|'||asset_id||'|'||kind||'|'||COALESCE(grp_id,'')||'|'||reason_code||'|'||status FROM suggestion",
            "SELECT key||'|'||value FROM setting",
            "SELECT id||'|'||state FROM asset",
            "SELECT CAST(COUNT(*) AS TEXT) FROM action_log",
        ] {
            let mut st = c.prepare(sql).unwrap();
            let rows: Vec<String> = st
                .query_map([], |r| r.get::<_, String>(0))
                .map(|it| it.map(Result::unwrap).collect())
                .unwrap_or_else(|_| vec![]);
            out.extend(rows);
        }
        out.sort();
        out
    })
}

fn pending_for_prefix(f: &Fjell, prefix: &str) -> i64 {
    f.with_store(|s| {
        s.conn()
            .query_row(
                "SELECT COUNT(*) FROM suggestion s JOIN asset a ON a.id = s.asset_id \
                 WHERE a.source_id LIKE ?1 AND s.status = 'pending' AND s.kind = 'exact_dup'",
                [format!("{prefix}%")],
                |r| r.get(0),
            )
            .unwrap()
    })
}

fn pending_any_for_prefix(f: &Fjell, prefix: &str) -> i64 {
    f.with_store(|s| {
        s.conn()
            .query_row(
                "SELECT COUNT(*) FROM suggestion s JOIN asset a ON a.id = s.asset_id \
                 WHERE a.source_id LIKE ?1 AND s.status = 'pending'",
                [format!("{prefix}%")],
                |r| r.get(0),
            )
            .unwrap()
    })
}

#[test]
fn k5_is_unchanged_without_rules() {
    let f = open();
    scan(&f);
    let g = groups(&f);
    assert_eq!(group_of(&g, "orig/a.jpg").unwrap().1, "orig/a.jpg");
    assert_eq!(group_of(&g, "kopi/b.jpg").unwrap().1, "kopi/b.jpg");
    assert!(g.iter().all(|x| x.4.is_none() && x.3.is_some()));
    // Ren utregning: tom regelliste (selv med alle stier tilgjengelig) er byte-lik `compute`.
    let (rows, paths) = f.with_store(|s| {
        (
            index_rows_on(s.conn()).unwrap(),
            source_ids_on(s.conn()).unwrap(),
        )
    });
    let none = RuleCtx {
        rules: &[],
        paths: &paths,
    };
    assert_eq!(
        compute(&rows),
        compute_with_rules(&rows, &mut GroupEngine::new(), &none)
    );
}

#[test]
fn rules_set_before_scan_pick_keeper_and_store_typed_reason() {
    let f = open();
    f.set_keeper_rules(vec![fold("kopi")]).unwrap();
    assert_eq!(f.keeper_rules(), vec![fold("kopi")]);
    scan(&f);
    let g = groups(&f);
    let a = group_of(&g, "orig/a.jpg").unwrap();
    assert_eq!(a.1, "kopi/aa.jpg");
    assert_eq!(a.3, None);
    assert_eq!(
        a.4.as_deref().and_then(RuleReason::decode),
        Some(RuleReason::PreferFolder {
            prefix: "kopi".into()
        })
    );
    // Uavgjort (ingen av `x`/`y` i `kopi`): K5.
    let c = group_of(&g, "x/c.jpg").unwrap();
    assert_eq!((c.1.as_str(), c.4.is_none()), ("x/c.jpg", true));
}

#[test]
fn protected_folder_never_gets_a_suggestion() {
    let f = open();
    scan(&f);
    assert!(
        pending_any_for_prefix(&f, "privat/") > 0,
        "forutsetning: privat foreslås uten regel"
    );
    f.set_keeper_rules(vec![KeeperRule::ProtectFolder {
        prefix: "PRIVAT".into(),
    }])
    .unwrap();
    f.sync_groups().unwrap();
    assert_eq!(pending_any_for_prefix(&f, "privat/"), 0);
    let g = groups(&f);
    let b = group_of(&g, "kopi/b.jpg").unwrap();
    assert_eq!(b.1, "privat/bbb.jpg", "beskyttet bilde blir keeper");
    assert_eq!(
        b.4.as_deref().and_then(RuleReason::decode),
        Some(RuleReason::ProtectedFolder {
            prefix: "PRIVAT".into()
        })
    );
    // To beskyttede i samme eksakte gruppe: ingen gruppe og ingen forslag.
    assert!(group_of(&g, "privat/d.jpg").is_none());
    // Gjelder også nye skanninger og alle forslagstyper.
    scan(&f);
    assert_eq!(pending_any_for_prefix(&f, "privat/"), 0);
    let _ = SuggestionKind::ALL;
}

fn name_of(f: &Fjell, a: AssetId) -> String {
    f.with_store(|s| {
        s.conn()
            .query_row(
                "SELECT source_id FROM asset WHERE id = ?1",
                [a.0 as i64],
                |r| r.get(0),
            )
            .unwrap()
    })
}

fn id_of(f: &Fjell, name: &str) -> AssetId {
    f.with_store(|s| {
        AssetId(
            s.conn()
                .query_row("SELECT id FROM asset WHERE source_id = ?1", [name], |r| {
                    r.get::<_, i64>(0)
                })
                .unwrap() as u64,
        )
    })
}

fn named(f: &Fjell, v: &[KeeperChange]) -> Vec<(String, String, Option<RuleReason>)> {
    v.iter()
        .map(|c| {
            (
                name_of(f, c.old_keeper),
                name_of(f, c.new_keeper),
                c.rule.clone(),
            )
        })
        .collect()
}

#[test]
fn preview_changes_nothing() {
    let f = open();
    scan(&f);
    let before = snapshot(&f);
    assert!(!before.is_empty());
    let rules = vec![fold("kopi")];
    let p1 = f.preview_keeper_rules(&rules).unwrap();
    assert_eq!(
        p1,
        f.preview_keeper_rules(&rules).unwrap(),
        "deterministisk"
    );
    assert_eq!(
        named(&f, &p1),
        vec![(
            "orig/a.jpg".to_string(),
            "kopi/aa.jpg".to_string(),
            Some(RuleReason::PreferFolder {
                prefix: "kopi".into()
            })
        )]
    );
    assert_eq!(snapshot(&f), before, "forhåndsvisning skriver ingenting");
    assert!(f.keeper_rules().is_empty());
}

#[test]
fn new_rules_do_not_touch_existing_groups_until_applied() {
    let f = open();
    scan(&f);
    let before = groups(&f);
    f.set_keeper_rules(vec![fold("kopi")]).unwrap();
    f.sync_groups().unwrap();
    assert_eq!(groups(&f), before, "eksisterende grupper beholder keeper");
    let done = f.apply_keeper_rules().unwrap();
    assert_eq!(done.len(), 1);
    let g = groups(&f);
    let a = group_of(&g, "orig/a.jpg").unwrap();
    assert_eq!((a.1.as_str(), a.3.clone()), ("kopi/aa.jpg", None));
    assert!(a.4.is_some());
    // Forslaget flyttet med: gammel keeper er nå forslaget, ny keeper har ingen.
    assert_eq!(pending_for_prefix(&f, "orig/"), 1);
    assert_eq!(pending_for_prefix(&f, "kopi/aa"), 0);
    // Ingenting å gjøre andre gang.
    assert!(f.apply_keeper_rules().unwrap().is_empty());
    // Tomme regler: tilbake til K5 (begrunnelse gjenopprettet, ingen regel).
    f.set_keeper_rules(vec![]).unwrap();
    assert_eq!(f.apply_keeper_rules().unwrap().len(), 1);
    let a = groups(&f);
    let a = group_of(&a, "orig/a.jpg").unwrap();
    assert_eq!(
        (a.1.as_str(), a.3.is_some(), a.4.is_none()),
        ("orig/a.jpg", true, true)
    );
}

#[test]
fn decided_groups_are_never_touched_by_apply() {
    let f = open();
    scan(&f);
    // Manuelt keeper-valg i x/y-gruppen.
    let c = f
        .cards(SuggestionKind::ExactDup, 50, None)
        .unwrap()
        .cards
        .into_iter()
        .find(|c| name_of(&f, c.asset) == "y/cc.jpg")
        .unwrap();
    f.set_keeper(c.group.unwrap(), c.asset).unwrap();
    // Avvist forslag (Keep) i kopi/b-gruppen.
    let b = f
        .cards(SuggestionKind::ExactDup, 50, None)
        .unwrap()
        .cards
        .into_iter()
        .find(|c| name_of(&f, c.asset) == "privat/bbb.jpg")
        .unwrap();
    f.decide_many(vec![(b.id, Decision::Keep)]).unwrap();
    let before = groups(&f);
    f.set_keeper_rules(vec![fold("x"), fold("privat")]).unwrap();
    f.sync_groups().unwrap();
    let p = f.preview_keeper_rules(&f.keeper_rules()).unwrap();
    let names = named(&f, &p);
    assert!(
        names
            .iter()
            .all(|(_, new, _)| new != "x/c.jpg" && new != "privat/bbb.jpg"),
        "{names:?}"
    );
    f.apply_keeper_rules().unwrap();
    let after = groups(&f);
    for n in ["x/c.jpg", "kopi/b.jpg"] {
        assert_eq!(group_of(&before, n), group_of(&after, n), "{n}");
    }
}

#[test]
fn trashed_members_and_trash_are_untouched() {
    let f = open();
    scan(&f);
    let c = f
        .cards(SuggestionKind::ExactDup, 50, None)
        .unwrap()
        .cards
        .into_iter()
        .find(|c| name_of(&f, c.asset) == "kopi/aa.jpg")
        .unwrap();
    f.decide_many(vec![(c.id, Decision::Trash)]).unwrap();
    f.sync_groups().unwrap();
    let trashed = id_of(&f, "kopi/aa.jpg");
    f.set_keeper_rules(vec![fold("kopi"), KeeperRule::PreferNewest])
        .unwrap();
    f.apply_keeper_rules().unwrap();
    let st = f.with_store(|s| s.asset_row(trashed).unwrap().unwrap().state);
    assert_eq!(st, crate::model::State::Trashed);
    assert_eq!(f.trash_summary().count, 1);
}

#[test]
fn same_input_gives_same_output() {
    let rules = vec![
        fold("kopi"),
        KeeperRule::PreferNewest,
        KeeperRule::PreferShortestName,
        KeeperRule::ProtectFolder {
            prefix: "privat".into(),
        },
    ];
    let run = || {
        let f = open();
        f.set_keeper_rules(rules.clone()).unwrap();
        scan(&f);
        (groups(&f), f.preview_keeper_rules(&[]).unwrap().len())
    };
    assert_eq!(run(), run());
}

#[test]
fn index_without_rules_migrates_and_behaves_as_before() {
    let mut conn = rusqlite::Connection::open_in_memory().unwrap();
    crate::store::migrate_to(&mut conn, 2).unwrap();
    conn.execute("INSERT INTO grp(kind, keeper_id, created_at, keeper_reason) VALUES ('exact', NULL, 0, 'older')", [])
        .unwrap();
    // Regelmotorens migrering er nr. 4 (nr. 3 er indeksen fra main).
    crate::store::migrate_to(&mut conn, 4).unwrap();
    let rule: Option<String> = conn
        .query_row("SELECT keeper_rule FROM grp", [], |r| r.get(0))
        .unwrap();
    assert_eq!(rule, None);
    assert_eq!(
        crate::store::setting_on(&conn, crate::store::KEEPER_RULES_KEY).unwrap(),
        None
    );
    let f = open();
    assert!(f.keeper_rules().is_empty());
}
