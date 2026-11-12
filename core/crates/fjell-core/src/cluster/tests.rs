//! Tester for K1-7 og K2b-1, 2, 5, 6, 8 (golden-delene K1-3, K2b-3, 4, 7 ligger i
//! `core/testdata/tests/golden_algos.rs`). Tilfeldighet kommer fra en fast xorshift-RNG.

use super::*;
use crate::model::Hash64;
use std::collections::BTreeMap;

/// Deterministisk xorshift64* (proptest er ikke en tillatt avhengighet ennå).
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
}

type Item = (AssetId, Hash64, Hash64);

fn shuffle<T>(v: &mut [T], r: &mut Rng) {
    for i in (1..v.len()).rev() {
        v.swap(i, r.below(i as u64 + 1) as usize);
    }
}

fn flip(mut x: u64, n: usize, r: &mut Rng) -> u64 {
    for _ in 0..n {
        x ^= 1 << r.below(64);
    }
    x
}

/// Hasher i klynger: noen er nær hverandre (0–9 flippede biter), noen er helt tilfeldige.
fn clustered(n: usize, r: &mut Rng) -> Vec<Item> {
    let mut v: Vec<Item> = Vec::new();
    for i in 0..n {
        let (d, p) = if i > 2 && r.below(10) < 6 {
            let (_, bd, bp) = v[r.below(i as u64) as usize];
            (
                flip(bd.0, r.below(9) as usize, r),
                flip(bp.0, r.below(9) as usize, r),
            )
        } else {
            (r.next(), r.next())
        };
        v.push((AssetId(i as u64 + 1), Hash64(d), Hash64(p)));
    }
    v
}

fn brute(items: &[Item], max: u32) -> Vec<(AssetId, AssetId)> {
    let mut out = Vec::new();
    for (i, a) in items.iter().enumerate() {
        for b in &items[i + 1..] {
            if a.1.hamming(b.1) <= max && a.2.hamming(b.2) <= max {
                out.push((a.0.min(b.0), a.0.max(b.0)));
            }
        }
    }
    out.sort();
    out
}

#[test]
fn k1_7_exact_groups_independent_of_order() {
    let mut r = Rng(3);
    let mut items: Vec<(AssetId, [u8; 32])> = (0..300)
        .map(|i| {
            let mut hsh = [0u8; 32];
            hsh[0] = r.below(40) as u8; // mange kollisjoner
            (AssetId(i + 1), hsh)
        })
        .collect();
    let base = exact_groups(&items);
    assert!(base.iter().all(|g| g.members.len() > 1));
    assert!(base
        .iter()
        .all(|g| g.members.windows(2).all(|w| w[0] < w[1])));
    assert!(base.windows(2).all(|w| w[0] < w[1]));
    for _ in 0..20 {
        shuffle(&mut items, &mut r);
        assert_eq!(exact_groups(&items), base);
    }
    assert!(exact_groups(&[]).is_empty());
}

#[test]
fn exact_groups_ignore_singletons() {
    let (a, b) = ([1u8; 32], [2u8; 32]);
    let g = exact_groups(&[(AssetId(3), a), (AssetId(1), a), (AssetId(2), b)]);
    assert_eq!(
        g,
        vec![Group {
            members: vec![AssetId(1), AssetId(3)]
        }]
    );
}

#[test]
fn k2b_1_mih_equals_brute_force() {
    let mut r = Rng(11);
    for trial in 0..6 {
        let n = if trial == 0 { 2000 } else { 400 };
        let items = clustered(n, &mut r);
        for max in [0u32, 3, 6, 7, 9] {
            let p = Params {
                max_dist: max,
                ..Params::default()
            };
            assert_eq!(
                similar_pairs(&items, &p),
                brute(&items, max),
                "trial {trial}, max {max}"
            );
        }
    }
}

#[test]
fn k2b_2_pigeonhole_boundary() {
    // 6 biter i 6 ulike blokker, og 7 biter i 7 ulike blokker (siste blokk er fortsatt lik).
    for bits in [6usize, 7] {
        let mask: u64 = (0..bits).fold(0, |m, blk| m | (1u64 << (blk * 8 + 3)));
        let base = 0xDEAD_BEEF_0123_4567u64;
        let items = [
            (AssetId(1), Hash64(base), Hash64(base)),
            (AssetId(2), Hash64(base ^ mask), Hash64(base ^ mask)),
        ];
        let p = Params {
            max_dist: bits as u32,
            ..Params::default()
        };
        assert_eq!(
            similar_pairs(&items, &p),
            vec![(AssetId(1), AssetId(2))],
            "{bits} biter"
        );
        let tight = Params {
            max_dist: bits as u32 - 1,
            ..Params::default()
        };
        assert!(similar_pairs(&items, &tight).is_empty());
    }
    // 8 biter i 8 blokker finnes ikke via MIH; `max_dist` ≥ 8 bruker full gjennomgang og er eksakt.
    let items = [
        (AssetId(1), Hash64(0), Hash64(0)),
        (
            AssetId(2),
            Hash64(u64::MAX >> 8),
            Hash64(0x0101_0101_0101_0101),
        ),
    ];
    let p = Params {
        max_dist: 8,
        ..Params::default()
    };
    assert_eq!(similar_pairs(&items, &p), brute(&items, 8));
}

#[test]
fn k2b_5_groups_independent_of_order() {
    let mut r = Rng(21);
    let mut items = clustered(600, &mut r);
    let base = visual_groups(&items, &Params::default());
    assert!(!base.is_empty());
    for _ in 0..10 {
        shuffle(&mut items, &mut r);
        assert_eq!(visual_groups(&items, &Params::default()), base);
    }
}

/// Kjede der naboene er `step` biter fra hverandre (nye tilfeldige biter per ledd).
fn chain(len: usize, step: usize, r: &mut Rng) -> Vec<Item> {
    let (mut d, mut p) = (r.next(), r.next());
    (0..len)
        .map(|i| {
            let it = (AssetId(i as u64 + 1), Hash64(d), Hash64(p));
            d = flip(d, step, r);
            p = flip(p, step, r);
            it
        })
        .collect()
}

#[test]
fn k2b_6_chain_protection() {
    let mut r = Rng(31);
    let p = Params::default();
    for (len, step) in [(40usize, 3usize), (60, 4), (80, 6)] {
        let items = chain(len, step, &mut r);
        let groups = visual_groups(&items, &p);
        let hs: BTreeMap<AssetId, (Hash64, Hash64)> =
            items.iter().map(|i| (i.0, (i.1, i.2))).collect();
        for g in &groups {
            assert!(
                g.members.len() <= p.max_group,
                "len {len}: gruppe på {}",
                g.members.len()
            );
            for (i, a) in g.members.iter().enumerate() {
                for b in &g.members[i + 1..] {
                    let (x, y) = (hs[a], hs[b]);
                    let d = x.0.hamming(y.0).max(x.1.hamming(y.1));
                    assert!(
                        d <= p.max_span,
                        "negativt par slått sammen via kjede (avstand {d})"
                    );
                }
            }
        }
    }
}

#[test]
fn identical_hashes_over_limit_are_chunked() {
    let items: Vec<Item> = (1..=70)
        .map(|i| (AssetId(i), Hash64(5), Hash64(9)))
        .collect();
    let ix = {
        let mut ix = VisualIndex::new(Params::default());
        ix.update(&items, &[]);
        ix
    };
    let sizes: Vec<usize> = ix.groups().iter().map(|g| g.members.len()).collect();
    assert_eq!(sizes, vec![30, 30, 10]);
    assert!(ix.stats().forced_chunks >= 1);
}

#[test]
fn identical_hashes_do_not_trip_bucket_cap() {
    // 600 bilder med lik hash er én nøkkel, ikke en bøtte på 600 (og én gruppe, delt av kjedevern).
    let mut items: Vec<Item> = (1..=600)
        .map(|i| (AssetId(i), Hash64(1), Hash64(2)))
        .collect();
    items.push((AssetId(1000), Hash64(1 ^ 1), Hash64(2 ^ 1)));
    let mut ix = VisualIndex::new(Params::default());
    ix.update(&items, &[]);
    assert_eq!(ix.stats().oversized_buckets, 0);
    assert!(!ix.groups().is_empty());
}

#[test]
fn oversized_buckets_are_counted() {
    let p = Params {
        bucket_cap: 5,
        ..Params::default()
    };
    // 20 ulike hasher som alle deler blokk 0 (pHash lav byte = 0).
    let items: Vec<Item> = (0..20u64)
        .map(|i| (AssetId(i + 1), Hash64(i << 20), Hash64(i << 24)))
        .collect();
    let mut ix = VisualIndex::new(p);
    ix.update(&items, &[]);
    assert!(ix.stats().oversized_buckets >= 1);
}

#[test]
fn k2b_8_incremental_equals_full_rebuild() {
    let mut r = Rng(41);
    let pool = clustered(500, &mut r);
    let p = Params::default();
    let mut live: BTreeMap<AssetId, (Hash64, Hash64)> = BTreeMap::new();
    let mut ix = VisualIndex::new(p.clone());
    for step in 0..40 {
        let mut ups: Vec<Item> = Vec::new();
        let mut rem: Vec<AssetId> = Vec::new();
        for _ in 0..(1 + r.below(25)) {
            let it = pool[r.below(pool.len() as u64) as usize];
            match r.below(4) {
                0 if !live.is_empty() => {
                    let id = *live
                        .keys()
                        .nth(r.below(live.len() as u64) as usize)
                        .unwrap();
                    live.remove(&id);
                    ups.retain(|u| u.0 != id);
                    rem.push(id);
                }
                1 if !live.is_empty() => {
                    // Endre hashen til et eksisterende bilde (flytter det mellom komponenter).
                    let id = *live
                        .keys()
                        .nth(r.below(live.len() as u64) as usize)
                        .unwrap();
                    live.insert(id, (it.1, it.2));
                    ups.retain(|u| u.0 != id);
                    ups.push((id, it.1, it.2));
                }
                _ => {
                    live.insert(it.0, (it.1, it.2));
                    rem.retain(|&x| x != it.0);
                    ups.retain(|u| u.0 != it.0);
                    ups.push(it);
                }
            }
        }
        ix.update(&ups, &rem);
        let all: Vec<Item> = live.iter().map(|(&id, &(d, p))| (id, d, p)).collect();
        assert_eq!(ix.groups(), visual_groups(&all, &p), "steg {step}");
    }
}

#[test]
fn clustering_10k_is_within_budget() {
    // Budsjett (ARKITEKTUR del 4): 10 000 bilder < 5 s, også i debugbygg.
    let items = clustered(10_000, &mut Rng(51));
    let t = std::time::Instant::now();
    let g = visual_groups(&items, &Params::default());
    let el = t.elapsed();
    assert!(!g.is_empty());
    assert!(el.as_secs_f64() < 5.0, "klynging tok {el:?}");
}
