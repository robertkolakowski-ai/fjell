//! Tester for `series`: gruppering, rangering, øyesignal, determinisme og korpus (recall/presisjon).

use super::*;

fn it(id: u64, t: i64, h: u64, blur: f32) -> SeriesItem {
    SeriesItem {
        id: AssetId(id),
        taken_at: t,
        dhash: Hash64(h),
        phash: Hash64(h),
        blur,
        eyes: None,
    }
}

fn eyes(faces: u8, closed: u8, confidence: f32) -> Option<EyeReport> {
    Some(EyeReport {
        faces,
        closed,
        confidence,
    })
}

fn ids(s: &Series) -> Vec<u64> {
    s.members.iter().map(|m| m.id.0).collect()
}

#[test]
fn groups_by_time_gap_and_similarity() {
    let p = SeriesParams::default();
    let items = [
        it(1, 100, 0, 0.3),
        it(2, 103, 0b11, 0.3),
        it(3, 106, 0, 0.3),
        // glipp > 10 s: ny rekke, selv om bildet er likt
        it(4, 200, 0, 0.3),
        it(5, 202, 0, 0.3),
        // tett i tid, men helt ulikt bilde: ikke med
        it(6, 203, u64::MAX, 0.3),
    ];
    let s = build_series(&items, &p);
    assert_eq!(s.len(), 2);
    assert_eq!(s[0].ids(), vec![AssetId(1), AssetId(2), AssetId(3)]);
    assert_eq!(s[1].ids(), vec![AssetId(4), AssetId(5)]);
}

#[test]
fn single_images_and_empty_make_no_series() {
    let p = SeriesParams::default();
    assert!(build_series(&[], &p).is_empty());
    assert!(build_series(&[it(1, 0, 0, 0.3)], &p).is_empty());
    assert!(build_series(&[it(1, 0, 0, 0.3), it(1, 1, 0, 0.3)], &p).is_empty());
    assert!(rank_series(&[it(1, 0, 0, 0.3)], &p).is_none());
}

#[test]
fn chain_through_time_neighbours_but_not_over_threshold() {
    let p = SeriesParams {
        max_dist: 4,
        ..Default::default()
    };
    // 1-2 og 2-3 henger sammen (3 biter hver), 1-3 er 6 biter fra hverandre: kjeding via 2 gir én serie.
    let items = [
        it(1, 0, 0b0000_0000, 0.3),
        it(2, 1, 0b0000_0111, 0.3),
        it(3, 2, 0b0011_1111, 0.3),
    ];
    assert_eq!(build_series(&items, &p).len(), 1);
    let far = [it(1, 0, 0, 0.3), it(2, 1, 0xFF, 0.3)];
    assert!(build_series(&far, &p).is_empty());
}

#[test]
fn long_run_is_split_and_singletons_dropped() {
    let p = SeriesParams {
        max_len: 4,
        ..Default::default()
    };
    // 9 like bilder tett i tid: 4 + 4 + 1 (den siste blir ikke en serie).
    let items: Vec<SeriesItem> = (1..=9).map(|i| it(i, i as i64, 0, 0.3)).collect();
    let s = build_series(&items, &p);
    assert_eq!(s.len(), 2);
    assert!(s.iter().all(|x| x.members.len() == 4));
}

#[test]
fn keeper_is_sharpest_with_reason_and_positions() {
    let p = SeriesParams::default();
    let items = [it(1, 0, 0, 0.1), it(2, 1, 0, 0.4), it(3, 2, 0, 0.2)];
    let s = rank_series(&items, &p).unwrap();
    assert_eq!(ids(&s), vec![2, 3, 1]);
    assert_eq!(s.keeper, AssetId(2));
    assert_eq!(s.keeper_reason, ReasonCode::KeeperBestOverall);
    assert_eq!(s.members[0].detail, ReasonDetail::None);
    assert_eq!(s.members[1].reason, ReasonCode::SeriesMember);
    assert_eq!(
        s.members[1].detail,
        ReasonDetail::Position { index: 2, of: 3 }
    );
    assert!((s.members[0].score - 1.0).abs() < 1e-6);
    assert!(s.members.windows(2).all(|w| w[0].score >= w[1].score));
}

#[test]
fn closed_eyes_demote_sharpest_and_give_eyes_open_reason() {
    let p = SeriesParams::default();
    let mut a = it(1, 0, 0, 0.5);
    a.eyes = eyes(2, 1, 0.95);
    let mut b = it(2, 1, 0, 0.3);
    b.eyes = eyes(2, 0, 0.95);
    let s = rank_series(&[a, b], &p).unwrap();
    assert_eq!(s.keeper, AssetId(2));
    assert_eq!(s.keeper_reason, ReasonCode::KeeperEyesOpen);
    assert_eq!(s.members[1].reason, ReasonCode::ClosedEyes);
    assert_eq!(s.members[1].detail, ReasonDetail::Count(1));
    assert!(s.members[1].score <= 0.5 && s.members[0].score >= 0.5);
}

#[test]
fn unsure_or_missing_eye_analysis_is_never_closed() {
    let p = SeriesParams::default();
    let mut a = it(1, 0, 0, 0.5);
    a.eyes = eyes(2, 2, 0.3); // for lav sikkerhet
    let b = it(2, 1, 0, 0.3); // ikke analysert
    let s = rank_series(&[a, b], &p).unwrap();
    assert_eq!(s.keeper, AssetId(1));
    assert_eq!(s.keeper_reason, ReasonCode::KeeperBestOverall);
    assert!(s.members.iter().all(|m| m.reason != ReasonCode::ClosedEyes));
    // faces = 0 med closed > 0 er meningsløst og regnes ikke som lukket.
    let mut c = it(3, 2, 0, 0.9);
    c.eyes = eyes(0, 1, 1.0);
    assert_eq!(rank_series(&[c, b], &p).unwrap().keeper, AssetId(3));
}

#[test]
fn all_closed_picks_sharpest_without_eyes_claim() {
    let p = SeriesParams::default();
    let mut a = it(1, 0, 0, 0.5);
    a.eyes = eyes(1, 1, 0.9);
    let mut b = it(2, 1, 0, 0.3);
    b.eyes = eyes(1, 1, 0.9);
    let s = rank_series(&[a, b], &p).unwrap();
    assert_eq!(s.keeper, AssetId(1));
    assert_eq!(s.keeper_reason, ReasonCode::KeeperBestOverall);
}

#[test]
fn low_contrast_and_nan_count_as_zero_sharpness_and_ties_use_lowest_id() {
    let p = SeriesParams::default();
    let items = [
        it(5, 0, 0, crate::quality::LOW_CONTRAST),
        it(4, 1, 0, f32::NAN),
        it(3, 2, 0, 0.2),
    ];
    let s = rank_series(&items, &p).unwrap();
    assert_eq!(ids(&s), vec![3, 4, 5]);
    let flat = [it(9, 0, 0, 1000.0), it(8, 1, 0, 1000.0)];
    assert_eq!(rank_series(&flat, &p).unwrap().keeper, AssetId(8));
}

#[test]
fn result_is_independent_of_input_order() {
    let p = SeriesParams::default();
    let mut items: Vec<SeriesItem> = (1..=12)
        .map(|i| {
            let t = (i as i64) * 3 + if i > 6 { 100 } else { 0 };
            it(i, t, i % 3, 0.1 * i as f32)
        })
        .collect();
    let a = build_series(&items, &p);
    items.reverse();
    assert_eq!(a, build_series(&items, &p));
    items.rotate_left(5);
    assert_eq!(a, build_series(&items, &p));
}

#[test]
fn with_keeper_moves_choice_first_without_best_claim() {
    let p = SeriesParams::default();
    let mut c = it(3, 2, 0, 0.2);
    c.eyes = eyes(1, 0, 0.9);
    let items = [it(1, 0, 0, 0.1), it(2, 1, 0, 0.4), c];
    let s = rank_series(&items, &p).unwrap();
    let m = s.with_keeper(AssetId(1), &items, &p).unwrap();
    assert_eq!(m.keeper, AssetId(1));
    assert_eq!(ids(&m), vec![1, 2, 3]);
    assert_eq!(m.keeper_reason, ReasonCode::SeriesMember);
    assert_eq!(
        m.members[1].detail,
        ReasonDetail::Position { index: 2, of: 3 }
    );
    let e = s.with_keeper(AssetId(3), &items, &p).unwrap();
    assert_eq!(e.keeper_reason, ReasonCode::KeeperEyesOpen);
    assert_eq!(s.with_keeper(AssetId(2), &items, &p).unwrap(), s);
    assert!(s.with_keeper(AssetId(99), &items, &p).is_none());
}

/// Små deterministiske tall uten eksterne avhengigheter (xorshift).
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
}

/// Syntetisk korpus: 40 «sanne» serier (3-6 bilder, hash med få biters støy, 1-4 s mellom bilder) skilt med
/// lange pauser, pluss enkeltbilder med tilfeldig hash. Måler recall og presisjon på par.
#[test]
fn corpus_recall_and_precision() {
    let p = SeriesParams::default();
    let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
    let mut items = Vec::new();
    let mut truth: BTreeMap<u64, u64> = BTreeMap::new(); // id -> sann serie (0 = enkeltbilde)
    let (mut id, mut t) = (1u64, 1_000i64);
    for s in 1..=40u64 {
        let base = rng.next();
        for _ in 0..3 + (rng.next() % 4) {
            let mut h = base;
            for _ in 0..(rng.next() % 3) {
                h ^= 1 << (rng.next() % 64);
            }
            items.push(it(id, t, h, 0.05 + (rng.next() % 100) as f32 / 200.0));
            truth.insert(id, s);
            id += 1;
            t += 1 + (rng.next() % 4) as i64;
        }
        t += 60;
        for _ in 0..2 {
            items.push(it(id, t, rng.next(), 0.3));
            truth.insert(id, 0);
            id += 1;
            t += 60;
        }
    }
    let series = build_series(&items, &p);
    let (mut tp, mut fp) = (0u64, 0u64);
    for s in &series {
        let m: Vec<u64> = s.members.iter().map(|m| m.id.0).collect();
        for (i, a) in m.iter().enumerate() {
            for b in &m[i + 1..] {
                if truth[a] != 0 && truth[a] == truth[b] {
                    tp += 1;
                } else {
                    fp += 1;
                }
            }
        }
    }
    let mut total = 0u64;
    for s in 1..=40u64 {
        let n = truth.values().filter(|&&v| v == s).count() as u64;
        total += n * (n - 1) / 2;
    }
    let recall = tp as f64 / total as f64;
    let precision = tp as f64 / (tp + fp) as f64;
    assert!(recall >= 0.95, "recall {recall}");
    assert!(precision >= 0.98, "presisjon {precision}");
}

#[test]
fn large_run_stays_fast() {
    // 20 000 forskjellige bilder tett i tid: bare nabovinduet sammenlignes.
    let p = SeriesParams::default();
    let mut rng = Rng(7);
    let items: Vec<SeriesItem> = (1..=20_000u64)
        .map(|i| it(i, i as i64, rng.next(), 0.2))
        .collect();
    let t = std::time::Instant::now();
    let _ = build_series(&items, &p);
    assert!(t.elapsed().as_millis() < 1000);
}
