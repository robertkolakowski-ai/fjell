//! Tester for keeper-reglene: hver regel, rekkefølge, uavgjort, beskyttelse, determinisme, lagring.

use super::*;

fn c(id: u64, path: &str) -> RuleCandidate {
    RuleCandidate {
        id: AssetId(id),
        path: path.into(),
        created_at: 1000,
        bytes: 100,
        pixels: 100,
        sharpness: None,
    }
}

fn ids(d: &Decision) -> Vec<u64> {
    d.narrowed.iter().map(|a| a.0).collect()
}

#[test]
fn no_rules_means_no_narrowing_and_no_reason() {
    let d = decide(&[], &[c(1, "/a/x.jpg"), c(2, "/b/x.jpg")]);
    assert_eq!((ids(&d), d.reason), (vec![1, 2], None));
}

#[test]
fn prefer_folder_picks_folder_and_gives_param_reason() {
    let rules = [KeeperRule::PreferFolder {
        prefix: "/Bilder/Original".into(),
        priority: 0,
    }];
    let d = decide(
        &rules,
        &[c(1, "/Bilder/Kopi/a.jpg"), c(2, "/bilder/original/a.jpg")],
    );
    assert_eq!(ids(&d), vec![2]);
    assert_eq!(
        d.reason,
        Some(RuleReason::PreferFolder {
            prefix: "/Bilder/Original".into()
        })
    );
}

#[test]
fn folder_match_is_whole_components_and_slash_agnostic() {
    assert!(in_folder("C:\\Bilder\\Ferie\\a.jpg", "c:/bilder/ferie/"));
    assert!(!in_folder("/a/bo/x.jpg", "/a/b"));
    assert!(!in_folder("/a/b/x.jpg", ""));
}

#[test]
fn folder_priority_beats_list_order_and_ties_use_list_order() {
    let rules = [
        KeeperRule::PreferFolder {
            prefix: "/lav".into(),
            priority: 1,
        },
        KeeperRule::PreferFolder {
            prefix: "/hoy".into(),
            priority: 5,
        },
    ];
    let d = decide(&rules, &[c(1, "/lav/a"), c(2, "/hoy/a"), c(3, "/annet/a")]);
    assert_eq!(ids(&d), vec![2]);
    let eq = [
        KeeperRule::PreferFolder {
            prefix: "/a".into(),
            priority: 1,
        },
        KeeperRule::PreferFolder {
            prefix: "/b".into(),
            priority: 1,
        },
    ];
    let d = decide(&eq, &[c(1, "/a/x"), c(2, "/b/x")]);
    assert_eq!(ids(&d), vec![1, 2], "lik prioritet skiller ikke");
}

#[test]
fn newest_oldest_largest_resolution_sharpest_shortest() {
    let mut a = c(1, "/lang/sti/foto_kopi_2.jpg");
    let mut b = c(2, "/x/f.jpg");
    a.created_at = 10;
    a.bytes = 900;
    a.pixels = 50;
    a.sharpness = Some(100.0);
    b.created_at = 20;
    b.bytes = 100;
    b.pixels = 80;
    b.sharpness = Some(50.0);
    let cs = [a, b];
    let one = |r: KeeperRule| decide(&[r], &cs);
    assert_eq!(ids(&one(KeeperRule::PreferNewest)), vec![2]);
    assert_eq!(ids(&one(KeeperRule::PreferOldest)), vec![1]);
    assert_eq!(ids(&one(KeeperRule::PreferLargest)), vec![1]);
    assert_eq!(ids(&one(KeeperRule::PreferHighestResolution)), vec![2]);
    assert_eq!(ids(&one(KeeperRule::PreferSharpest)), vec![1]);
    assert_eq!(ids(&one(KeeperRule::PreferShortestPath)), vec![2]);
    assert_eq!(ids(&one(KeeperRule::PreferShortestName)), vec![2]);
    assert_eq!(
        one(KeeperRule::PreferShortestName).reason,
        Some(RuleReason::PreferShortestName)
    );
}

#[test]
fn sharpest_needs_ten_percent_and_ignores_unassessable() {
    let mut a = c(1, "/a");
    let mut b = c(2, "/b");
    a.sharpness = Some(100.0);
    b.sharpness = Some(95.0);
    let d = decide(&[KeeperRule::PreferSharpest], &[a.clone(), b.clone()]);
    assert_eq!((ids(&d), d.reason), (vec![1, 2], None));
    b.sharpness = None;
    let d = decide(&[KeeperRule::PreferSharpest], &[a, b]);
    assert_eq!(ids(&d), vec![1, 2], "ikke vurderbar fjernes ikke");
}

#[test]
fn first_separating_rule_is_the_reason_and_later_rules_break_remaining_ties() {
    let mut a = c(1, "/a");
    let mut b = c(2, "/b");
    let mut d3 = c(3, "/c");
    a.bytes = 500;
    b.bytes = 500;
    d3.bytes = 10;
    a.created_at = 5;
    b.created_at = 9;
    let rules = [KeeperRule::PreferLargest, KeeperRule::PreferNewest];
    let d = decide(&rules, &[a, b, d3]);
    assert_eq!(ids(&d), vec![2]);
    assert_eq!(d.reason, Some(RuleReason::PreferLargest));
}

#[test]
fn tie_leaves_all_candidates_for_k5() {
    let rules = [KeeperRule::PreferLargest, KeeperRule::PreferNewest];
    let d = decide(&rules, &[c(1, "/a"), c(2, "/b")]);
    assert_eq!((ids(&d), d.reason), (vec![1, 2], None));
}

#[test]
fn protect_folder_restricts_candidates_before_other_rules() {
    let rules = [
        KeeperRule::PreferLargest,
        KeeperRule::ProtectFolder {
            prefix: "/Privat".into(),
        },
    ];
    let mut big = c(1, "/Ute/a.jpg");
    big.bytes = 9999;
    let d = decide(&rules, &[big, c(2, "/privat/a.jpg")]);
    assert_eq!(ids(&d), vec![2]);
    assert_eq!(
        d.reason,
        Some(RuleReason::ProtectedFolder {
            prefix: "/Privat".into()
        })
    );
    assert_eq!(
        protecting_prefix(&rules, "/PRIVAT/x/y.jpg"),
        Some("/Privat")
    );
    assert_eq!(protecting_prefix(&rules, "/Ute/y.jpg"), None);
}

#[test]
fn decision_is_independent_of_input_order() {
    let rules = [
        KeeperRule::PreferFolder {
            prefix: "/a".into(),
            priority: 0,
        },
        KeeperRule::PreferNewest,
        KeeperRule::PreferShortestName,
    ];
    let mut cs: Vec<RuleCandidate> = (1..=6)
        .map(|i| {
            let mut m = c(
                i,
                &format!(
                    "/{}/f{}.jpg",
                    if i % 2 == 0 { "a" } else { "b" },
                    "x".repeat(i as usize)
                ),
            );
            m.created_at = (i % 3) as i64;
            m
        })
        .collect();
    let base = decide(&rules, &cs);
    for _ in 0..cs.len() {
        cs.rotate_left(1);
        assert_eq!(decide(&rules, &cs), base);
        let mut r = cs.clone();
        r.reverse();
        assert_eq!(decide(&rules, &r), base);
    }
}

#[test]
fn storage_roundtrip_including_awkward_paths() {
    let rules = vec![
        KeeperRule::PreferFolder {
            prefix: "C:\\Bilder\\a\tb%c\nd".into(),
            priority: -3,
        },
        KeeperRule::PreferNewest,
        KeeperRule::PreferOldest,
        KeeperRule::PreferLargest,
        KeeperRule::PreferHighestResolution,
        KeeperRule::PreferSharpest,
        KeeperRule::PreferShortestPath,
        KeeperRule::PreferShortestName,
        KeeperRule::ProtectFolder {
            prefix: "/x:y".into(),
        },
    ];
    assert_eq!(decode_rules(&encode_rules(&rules)), rules);
    assert_eq!(
        encode_rules(&rules),
        encode_rules(&decode_rules(&encode_rules(&rules)))
    );
    assert!(decode_rules("").is_empty());
    assert!(decode_rules("ukjent\nprefer_folder\tx\tnan\n%%").is_empty());
}

#[test]
fn reason_roundtrip() {
    for r in [
        RuleReason::PreferFolder {
            prefix: "/a:b%".into(),
        },
        RuleReason::ProtectedFolder { prefix: "x".into() },
        RuleReason::PreferNewest,
        RuleReason::PreferShortestName,
    ] {
        assert_eq!(RuleReason::decode(&r.encode()), Some(r));
    }
    assert_eq!(RuleReason::decode("bogus"), None);
    assert_eq!(RuleReason::decode("prefer_newest:x"), None);
}
