//! Tester for K5 (PLAN-KJERNE del 7). Egenskapstester bruker fast xorshift-frø (proptest finnes ikke offline).

use super::*;
use crate::quality::LOW_CONTRAST;

fn m(id: u64, w: u32, h: u32, bytes: u64, blur: f32, created: i64) -> Member {
    Member {
        id: AssetId(id),
        width: w,
        height: h,
        bytes,
        blur_score: blur,
        created_at: created,
        protect_flags: ProtectFlags(0),
    }
}

fn prot(mut x: Member, flags: u8) -> Member {
    x.protect_flags = ProtectFlags(flags);
    x
}

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
}

fn random_group(r: &mut Rng) -> Vec<Member> {
    let n = 2 + r.below(8);
    (0..n)
        .map(|i| {
            let blur = if r.below(5) == 0 {
                LOW_CONTRAST
            } else {
                0.1 + r.below(10) as f32 * 0.05
            };
            let flags = if r.below(4) == 0 { 1 << r.below(3) } else { 0 };
            prot(
                m(
                    i + 1,
                    100 * (1 + r.below(3) as u32),
                    100,
                    1000 * (1 + r.below(3)),
                    blur,
                    r.below(3) as i64,
                ),
                flags as u8,
            )
        })
        .collect()
}

#[test]
fn k5_1_higher_resolution_wins_regardless_of_sharpness() {
    let big_blurry = m(1, 4000, 3000, 3_000_000, 0.05, 10);
    let small_sharp = m(2, 2000, 1500, 3_000_000, 0.9, 5);
    let r = rank_group(&[small_sharp, big_blurry]).unwrap();
    assert_eq!(
        (r.keeper, r.reason),
        (AssetId(1), ReasonCode::HigherResolution)
    );
}

#[test]
fn k5_2_equal_resolution_highest_bytes_per_pixel_wins() {
    let a = m(1, 1000, 1000, 500_000, 0.9, 1);
    let b = m(2, 1000, 1000, 900_000, 0.1, 9);
    let r = rank_group(&[a, b]).unwrap();
    assert_eq!(
        (r.keeper, r.reason),
        (AssetId(2), ReasonCode::LessCompressed)
    );
}

#[test]
fn k5_3_sharper_wins_only_with_more_than_ten_percent() {
    let a = m(1, 1000, 1000, 500_000, 0.50, 9);
    let b = m(2, 1000, 1000, 500_000, 0.60, 9);
    let r = rank_group(&[a, b]).unwrap();
    assert_eq!((r.keeper, r.reason), (AssetId(2), ReasonCode::Sharper));
    // 9 % forskjell: faller til alder (a er eldst).
    let c = m(3, 1000, 1000, 500_000, 0.545, 9);
    let a_old = m(1, 1000, 1000, 500_000, 0.50, 1);
    let r = rank_group(&[c, a_old]).unwrap();
    assert_eq!((r.keeper, r.reason), (AssetId(1), ReasonCode::Older));
    // LOW_CONTRAST er aldri «skarpere».
    let flat = m(4, 1000, 1000, 500_000, LOW_CONTRAST, 1);
    let sharp = m(5, 1000, 1000, 500_000, 0.9, 2);
    let r = rank_group(&[sharp, flat]).unwrap();
    assert_eq!((r.keeper, r.reason), (AssetId(4), ReasonCode::Older));
}

#[test]
fn k5_4_full_tie_gives_oldest_then_lowest_id() {
    let a = m(7, 10, 10, 100, 0.2, 50);
    let b = m(3, 10, 10, 100, 0.2, 20);
    let c = m(2, 10, 10, 100, 0.2, 20);
    let r = rank_group(&[a, b, c]).unwrap();
    assert_eq!((r.keeper, r.reason), (AssetId(2), ReasonCode::Older));
}

#[test]
fn step0_protected_member_preferred_and_ties_fall_through() {
    let big = m(1, 4000, 3000, 3_000_000, 0.5, 1);
    let small_fav = prot(m(2, 100, 100, 1000, 0.5, 9), ProtectFlags::FAVORITE);
    assert_eq!(rank_group(&[big, small_fav]).unwrap().keeper, AssetId(2));
    // Flere beskyttede: steg 1 avgjør blant dem.
    let big_edit = prot(m(3, 2000, 1500, 1_000_000, 0.5, 9), ProtectFlags::EDITED);
    let r = rank_group(&[big, small_fav, big_edit]).unwrap();
    assert_eq!(
        (r.keeper, r.reason),
        (AssetId(3), ReasonCode::HigherResolution)
    );
}

#[test]
fn k5_5_result_independent_of_member_order() {
    let mut r = Rng(0x9E37_79B9_7F4A_7C15);
    for _ in 0..2000 {
        let g = random_group(&mut r);
        let want = rank_group(&g).unwrap();
        for _ in 0..4 {
            let mut s = g.clone();
            for i in (1..s.len()).rev() {
                s.swap(i, r.below(i as u64 + 1) as usize);
            }
            assert_eq!(rank_group(&s).unwrap(), want);
        }
    }
}

#[test]
fn k5_7_exactly_one_keeper_without_suggestion() {
    let mut r = Rng(0xDEAD_BEEF_1234_5678);
    for _ in 0..2000 {
        let g = random_group(&mut r);
        for kind in [GroupKind::Exact, GroupKind::Visual { max_hamming: 4 }] {
            let res = group_suggestions(kind, &g).unwrap();
            assert!(g.iter().any(|x| x.id == res.keeper));
            assert_eq!(res.suggestions.len(), g.len() - 1);
            assert!(res.suggestions.iter().all(|s| s.asset_id != res.keeper));
            let ids: std::collections::BTreeSet<_> =
                res.suggestions.iter().map(|s| s.asset_id).collect();
            assert_eq!(ids.len(), g.len() - 1);
        }
    }
}

#[test]
fn k5_7_protected_never_loses_to_unprotected() {
    let mut r = Rng(42);
    for _ in 0..2000 {
        let g = random_group(&mut r);
        let k = rank_group(&g).unwrap().keeper;
        if g.iter().any(|x| x.protect_flags.is_protected()) {
            assert!(g
                .iter()
                .find(|x| x.id == k)
                .unwrap()
                .protect_flags
                .is_protected());
        }
    }
}

#[test]
fn empty_and_single_groups() {
    assert!(rank_group(&[]).is_none());
    let one = [m(1, 10, 10, 100, 0.2, 1)];
    let r = group_suggestions(GroupKind::Visual { max_hamming: 0 }, &one).unwrap();
    assert!(r.suggestions.is_empty());
    assert_eq!(r.keeper_reason, ReasonCode::Older);
}

#[test]
fn exact_group_reason_and_confidence() {
    let g = [
        m(1, 10, 10, 100, 0.2, 5),
        m(2, 10, 10, 100, 0.2, 3),
        m(3, 10, 10, 100, 0.2, 4),
    ];
    let res = group_suggestions(GroupKind::Exact, &g).unwrap();
    assert_eq!(
        (res.keeper, res.keeper_reason),
        (AssetId(2), ReasonCode::Older)
    );
    assert!(res.suggestions.iter().all(|s| {
        s.kind == SuggestionKind::ExactDup
            && s.reason == ReasonCode::ExactCopy
            && s.confidence == 1.0
    }));
}

#[test]
fn visual_confidence_steps_of_005_rounded_down() {
    let want = [1.0, 0.85, 0.70, 0.55, 0.40, 0.25, 0.10, 0.0];
    for (h, w) in want.iter().enumerate() {
        assert_eq!(visual_confidence(h as u32), *w as f32, "h={h}");
    }
    assert_eq!(visual_confidence(99), 0.0);
}

#[test]
fn suggestions_index_groups_and_skip_empty() {
    let a = [m(1, 10, 10, 100, 0.2, 1), m(2, 10, 10, 100, 0.2, 2)];
    let b = [m(5, 10, 10, 100, 0.2, 1), m(6, 20, 10, 100, 0.2, 2)];
    let out = suggestions(&[
        (GroupKind::Exact, &a),
        (GroupKind::Exact, &[]),
        (GroupKind::Visual { max_hamming: 2 }, &b),
    ]);
    assert_eq!(out.len(), 2);
    assert_eq!(out[1].keeper, AssetId(6));
    assert_eq!(out[1].suggestions[0].group, Some(2));
    assert_eq!(out[1].suggestions[0].confidence, 0.70);
}

#[test]
fn blur_threshold_rules() {
    assert_eq!(blur_threshold(&[]), BLUR_ABS_MAX);
    // Under 50 vurderbare: bare ABS_MAX.
    let few: Vec<f32> = (0..49).map(|i| 0.0001 * i as f32).collect();
    assert_eq!(blur_threshold(&few), BLUR_ABS_MAX);
    // Skarpt bibliotek: P10 over ABS_MAX gir ABS_MAX (flagger ikke alltid noe).
    let sharp: Vec<f32> = (0..100).map(|i| 0.2 + 0.01 * i as f32).collect();
    assert_eq!(blur_threshold(&sharp), BLUR_ABS_MAX);
    // Uskarpt bibliotek: P10 under ABS_MAX.
    let blurry: Vec<f32> = (0..100).map(|i| 0.0001 * i as f32).collect();
    assert!(blur_threshold(&blurry) < BLUR_ABS_MAX);
    // LOW_CONTRAST teller ikke med.
    let mut mixed = few.clone();
    mixed.extend(std::iter::repeat_n(LOW_CONTRAST, 100));
    assert_eq!(blur_threshold(&mixed), BLUR_ABS_MAX);
}

#[test]
fn blurry_suggestion_rules() {
    assert_eq!(blurry_suggestion(LOW_CONTRAST, 0.006), None);
    assert_eq!(blurry_suggestion(0.006, 0.006), None);
    assert_eq!(blurry_suggestion(f32::NAN, 0.006), None);
    assert_eq!(blurry_suggestion(0.0, 0.006), Some(1.0));
    let c = blurry_suggestion(0.005, 0.006).unwrap();
    assert!((0.5..=0.6).contains(&c));
    assert!(blurry_suggestion(0.001, 0.006).unwrap() > c);
}

#[test]
fn screenshot_suggestion_uses_signals() {
    let s = ShotSignals(ShotSignals::S3_NO_CAMERA_EXIF | ShotSignals::S4_PNG);
    let weak = screenshot_suggestion(AssetId(1), s, false).unwrap();
    assert_eq!(
        (weak.kind, weak.group, weak.confidence),
        (SuggestionKind::Screenshot, None, 0.70)
    );
    assert!(screenshot_suggestion(AssetId(1), s, true).is_none());
    assert!(screenshot_suggestion_default(AssetId(1), s).is_none());
}

#[test]
fn k5_8_rejected_suggestion_is_not_restored() {
    let a = [m(1, 10, 10, 100, 0.2, 1), m(2, 10, 10, 100, 0.2, 2)];
    let new: Vec<_> = suggestions(&[(GroupKind::Exact, &a)])
        .into_iter()
        .flat_map(|g| g.suggestions)
        .collect();
    assert_eq!(new.len(), 1);
    let kept = without_rejected(new.clone(), &[(AssetId(2), SuggestionKind::ExactDup)]);
    assert!(kept.is_empty());
    // Avvisning av en annen type blokkerer ikke.
    assert_eq!(
        without_rejected(new, &[(AssetId(2), SuggestionKind::Blurry)]).len(),
        1
    );
}

#[test]
fn k5_9_every_reason_code_has_i18n_key_in_nb_and_en() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../i18n");
    for loc in ["nb", "en"] {
        let text = std::fs::read_to_string(root.join(format!("{loc}.json"))).unwrap();
        for code in ReasonCode::ALL {
            let key = format!("\"reason.{}\"", code.key());
            assert!(text.contains(&key), "{loc}.json mangler {key}");
        }
    }
}

#[test]
#[cfg(not(debug_assertions))]
fn rank_group_30_members_under_10_us() {
    let mut r = Rng(7);
    let g: Vec<Member> = (0..30)
        .map(|i| {
            m(
                i + 1,
                1000 + r.below(3) as u32,
                1000,
                500_000 + r.below(3),
                0.1 + r.below(5) as f32 * 0.1,
                r.below(5) as i64,
            )
        })
        .collect();
    let t = std::time::Instant::now();
    for _ in 0..1000 {
        std::hint::black_box(rank_group(std::hint::black_box(&g)));
    }
    let per = t.elapsed().as_nanos() / 1000;
    eprintln!("rank_group(30): {per} ns");
    assert!(per < 10_000);
}
