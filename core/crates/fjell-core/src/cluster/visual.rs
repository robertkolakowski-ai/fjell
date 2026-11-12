//! Visuell klynging (K2b): multi-index hashing (MIH) i minnet + union-find + kjedevern.
//!
//! Grensesnitt:
//! - `Params`: terskler (startverdier fra ARKITEKTUR del 4: Hamming ≤ 6 på både dHash og pHash).
//! - `VisualIndex::{new, update, groups, stats}`: indeksen eier hashene og komponentene. `update`
//!   bygger bare om de sammenhengende komponentene som berøres (K2b-8). Full bygging er `update`
//!   med alle bilder, så `visual_groups` og inkrementell oppdatering deler samme kode.
//! - `similar_pairs`: alle par som holder terskelen (brukes av testene mot brute force).
//!
//! MIH: pHash deles i 8 blokker à 8 bit. To hasher med Hamming ≤ 7 deler minst én blokk (pigeonhole),
//! så kandidatene er alle med lik blokk i en posisjon; kandidatene verifiseres på dHash og pHash.
//! For `max_dist` ≥ 8 gjelder ikke garantien, og vi går over alle hasher (alltid eksakt).
//!
//! Bilder med lik (dHash, pHash) slås sammen til én nøkkel, så en serie like hasher ikke fyller
//! bøttene. Bøtter med flere nøkler enn `bucket_cap` hoppes over og telles i `Stats` (degenererte
//! hasher). Det kan miste par som bare deles via en slik bøtte.

use super::Group;
use crate::model::{AssetId, Hash64};
use std::collections::{BTreeMap, BTreeSet, HashMap};

type Key = (u64, u64); // (phash, dhash)

/// Terskler for visuell klynging.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Params {
    /// Høyeste Hamming på både dHash og pHash for at to bilder henger sammen.
    pub max_dist: u32,
    /// Kjedevern: største tillatte gruppe.
    pub max_group: usize,
    /// Kjedevern: største tillatte avstand mellom to medlemmer i en gruppe.
    pub max_span: u32,
    /// Skjerpet terskel ved ombygging av en komponent som bryter kjedevernet.
    pub tight_dist: u32,
    /// Bøtter med flere nøkler enn dette hoppes over (avvik, telles).
    pub bucket_cap: usize,
}

impl Default for Params {
    fn default() -> Params {
        Params {
            max_dist: 6,
            max_group: 30,
            max_span: 10,
            tight_dist: 4,
            bucket_cap: 500,
        }
    }
}

/// Metrikker fra indeksen.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Stats {
    pub keys: usize,
    /// Bøtter over `bucket_cap` (hoppet over).
    pub oversized_buckets: usize,
    /// Komponenter som måtte klynges om med skjerpet terskel.
    pub chain_splits: usize,
    /// Grupper som til slutt ble delt etter `AssetId` (identiske hasher over `max_group`).
    pub forced_chunks: usize,
}

fn dist(a: Key, b: Key) -> u32 {
    (a.0 ^ b.0).count_ones().max((a.1 ^ b.1).count_ones())
}

fn block(ph: u64, i: usize) -> u8 {
    (ph >> (8 * i)) as u8
}

/// Inkrementell indeks over visuelle grupper.
pub struct VisualIndex {
    p: Params,
    items: BTreeMap<AssetId, Key>,
    keys: BTreeMap<Key, BTreeSet<AssetId>>,
    buckets: Vec<HashMap<u8, BTreeSet<Key>>>,
    comp_of: BTreeMap<Key, u64>,
    comps: BTreeMap<u64, (BTreeSet<Key>, Vec<Group>)>,
    next_comp: u64,
    stats: Stats,
}

impl VisualIndex {
    pub fn new(p: Params) -> VisualIndex {
        VisualIndex {
            p,
            items: BTreeMap::new(),
            keys: BTreeMap::new(),
            buckets: vec![HashMap::new(); 8],
            comp_of: BTreeMap::new(),
            comps: BTreeMap::new(),
            next_comp: 0,
            stats: Stats::default(),
        }
    }

    fn insert_key(&mut self, id: AssetId, k: Key) {
        self.items.insert(id, k);
        if self.keys.entry(k).or_default().insert(id) && self.keys[&k].len() == 1 {
            for (i, b) in self.buckets.iter_mut().enumerate() {
                b.entry(block(k.0, i)).or_default().insert(k);
            }
        }
    }

    /// Fjerner `id`; returnerer nøkkelen den hadde.
    fn remove_id(&mut self, id: AssetId) -> Option<Key> {
        let k = self.items.remove(&id)?;
        let set = self.keys.get_mut(&k)?;
        set.remove(&id);
        if set.is_empty() {
            self.keys.remove(&k);
            for (i, b) in self.buckets.iter_mut().enumerate() {
                if let Some(s) = b.get_mut(&block(k.0, i)) {
                    s.remove(&k);
                    if s.is_empty() {
                        b.remove(&block(k.0, i));
                    }
                }
            }
        }
        Some(k)
    }

    /// Naboer (andre nøkler) innen `max_dist`, sortert, med avstand.
    fn neighbors(&self, k: Key) -> Vec<(Key, u32)> {
        let mut cand: BTreeSet<Key> = BTreeSet::new();
        if self.p.max_dist >= 8 {
            cand.extend(self.keys.keys().copied());
        } else {
            for (i, b) in self.buckets.iter().enumerate() {
                if let Some(s) = b.get(&block(k.0, i)) {
                    if s.len() <= self.p.bucket_cap {
                        cand.extend(s.iter().copied());
                    }
                }
            }
        }
        cand.into_iter()
            .filter(|&c| c != k)
            .filter_map(|c| {
                let d = dist(k, c);
                (d <= self.p.max_dist).then_some((c, d))
            })
            .collect()
    }

    /// Legger til/endrer `upserts` og fjerner `removed`, og bygger om bare berørte komponenter.
    pub fn update(&mut self, upserts: &[(AssetId, Hash64, Hash64)], removed: &[AssetId]) {
        let mut dirty: BTreeSet<u64> = BTreeSet::new();
        let mut seeds: BTreeSet<Key> = BTreeSet::new();
        let mut gone: Vec<AssetId> = removed.to_vec();
        for &(id, dh, ph) in upserts {
            let k = (ph.0, dh.0);
            match self.items.get(&id) {
                Some(&old) if old == k => {}
                Some(_) => gone.push(id),
                None => {}
            }
        }
        for id in gone {
            if let Some(k) = self.remove_id(id) {
                dirty.extend(self.comp_of.get(&k));
            }
        }
        for &(id, dh, ph) in upserts {
            let k = (ph.0, dh.0);
            if self.items.get(&id) != Some(&k) {
                self.insert_key(id, k);
                seeds.insert(k);
                dirty.extend(self.comp_of.get(&k));
            }
        }
        // Alle nøkler i berørte komponenter bygges om (nøkler som er borte hoppes over).
        for c in &dirty {
            if let Some((ks, _)) = self.comps.get(c) {
                seeds.extend(ks.iter().filter(|k| self.keys.contains_key(k)));
            }
        }
        // Steg 1: finn hele regionen (sammenhengende via naboer) og kantene i den.
        let mut region: BTreeSet<Key> = BTreeSet::new();
        let mut edges: BTreeMap<Key, Vec<(Key, u32)>> = BTreeMap::new();
        let mut stack: Vec<Key> = seeds.into_iter().collect();
        while let Some(k) = stack.pop() {
            if !region.insert(k) {
                continue;
            }
            let nb = self.neighbors(k);
            stack.extend(nb.iter().map(|&(n, _)| n).filter(|n| !region.contains(n)));
            edges.insert(k, nb);
        }
        // Steg 2: fjern gamle komponenter som regionen rører, og bygg nye.
        let mut old: BTreeSet<u64> = dirty;
        old.extend(region.iter().filter_map(|k| self.comp_of.get(k)).copied());
        for c in old {
            if let Some((ks, _)) = self.comps.remove(&c) {
                for k in ks {
                    self.comp_of.remove(&k);
                }
            }
        }
        let mut seen: BTreeSet<Key> = BTreeSet::new();
        for &start in &region {
            if seen.contains(&start) {
                continue;
            }
            let mut comp = BTreeSet::new();
            let mut st = vec![start];
            while let Some(k) = st.pop() {
                if comp.insert(k) {
                    st.extend(edges[&k].iter().map(|&(n, _)| n));
                }
            }
            seen.extend(comp.iter().copied());
            let groups = self.protect(&comp, &edges);
            let cid = self.next_comp;
            self.next_comp += 1;
            for &k in &comp {
                self.comp_of.insert(k, cid);
            }
            self.comps.insert(cid, (comp, groups));
        }
        self.stats.keys = self.keys.len();
    }

    fn size(&self, ks: &[Key]) -> usize {
        ks.iter().map(|k| self.keys[k].len()).sum()
    }

    fn to_group(&self, ks: &[Key]) -> Option<Group> {
        let mut m: Vec<AssetId> = ks
            .iter()
            .flat_map(|k| self.keys[k].iter().copied())
            .collect();
        m.sort();
        (m.len() > 1).then_some(Group { members: m })
    }

    /// Kjedevern: komponent > `max_group` eller med to medlemmer lenger fra hverandre enn
    /// `max_span` klynges om med terskel `tight_dist`, og videre nedover til alle deler holder.
    fn protect(
        &mut self,
        comp: &BTreeSet<Key>,
        edges: &BTreeMap<Key, Vec<(Key, u32)>>,
    ) -> Vec<Group> {
        let all: Vec<Key> = comp.iter().copied().collect();
        if self.ok(&all) {
            return self.to_group(&all).into_iter().collect();
        }
        self.stats.chain_splits += 1;
        let mut out = Vec::new();
        self.split(&all, edges, self.p.tight_dist, &mut out);
        out
    }

    fn ok(&self, ks: &[Key]) -> bool {
        self.size(ks) <= self.p.max_group
            && ks
                .iter()
                .all(|&a| ks.iter().all(|&b| dist(a, b) <= self.p.max_span))
    }

    fn split(
        &mut self,
        ks: &[Key],
        edges: &BTreeMap<Key, Vec<(Key, u32)>>,
        t: u32,
        out: &mut Vec<Group>,
    ) {
        let set: BTreeSet<Key> = ks.iter().copied().collect();
        let mut seen: BTreeSet<Key> = BTreeSet::new();
        for &s in ks {
            if seen.contains(&s) {
                continue;
            }
            let mut sub = BTreeSet::new();
            let mut st = vec![s];
            while let Some(k) = st.pop() {
                if sub.insert(k) {
                    st.extend(
                        edges[&k]
                            .iter()
                            .filter(|&&(n, d)| d <= t && set.contains(&n))
                            .map(|&(n, _)| n),
                    );
                }
            }
            seen.extend(sub.iter().copied());
            let sub: Vec<Key> = sub.into_iter().collect();
            if self.ok(&sub) {
                out.extend(self.to_group(&sub));
            } else if t > 0 {
                self.split(&sub, edges, t - 1, out);
            } else {
                // Identiske hasher over `max_group`: deles etter AssetId så ingen gruppe blir for stor.
                self.stats.forced_chunks += 1;
                let mut ids: Vec<AssetId> = sub
                    .iter()
                    .flat_map(|k| self.keys[k].iter().copied())
                    .collect();
                ids.sort();
                for c in ids.chunks(self.p.max_group.max(2)) {
                    if c.len() > 1 {
                        out.push(Group {
                            members: c.to_vec(),
                        });
                    }
                }
            }
        }
    }

    /// Alle grupper, sortert på første medlem (uavhengig av rekkefølgen de ble lagt inn i).
    pub fn groups(&self) -> Vec<Group> {
        let mut g: Vec<Group> = self
            .comps
            .values()
            .flat_map(|(_, g)| g.iter().cloned())
            .collect();
        g.sort();
        g
    }

    pub fn stats(&self) -> Stats {
        let mut s = self.stats.clone();
        s.oversized_buckets = self
            .buckets
            .iter()
            .flat_map(|b| b.values())
            .filter(|v| v.len() > self.p.bucket_cap)
            .count();
        s
    }

    /// Alle par (a < b) innen terskelen, med lik hash regnet som avstand 0.
    fn pairs(&self) -> Vec<(AssetId, AssetId)> {
        let mut out = Vec::new();
        for (&k, ids) in &self.keys {
            let v: Vec<AssetId> = ids.iter().copied().collect();
            for (i, &a) in v.iter().enumerate() {
                out.extend(v[i + 1..].iter().map(|&b| (a, b)));
            }
            for (n, _) in self.neighbors(k).into_iter().filter(|&(n, _)| n > k) {
                for &a in ids {
                    out.extend(self.keys[&n].iter().map(|&b| (a.min(b), a.max(b))));
                }
            }
        }
        out.sort();
        out
    }
}

/// Visuelle grupper for `items` = (id, dHash, pHash). Uavhengig av rekkefølge.
pub fn visual_groups(items: &[(AssetId, Hash64, Hash64)], p: &Params) -> Vec<Group> {
    let mut ix = VisualIndex::new(p.clone());
    ix.update(items, &[]);
    ix.groups()
}

/// Alle par innen terskelen via MIH, sortert (a < b).
pub fn similar_pairs(items: &[(AssetId, Hash64, Hash64)], p: &Params) -> Vec<(AssetId, AssetId)> {
    let mut ix = VisualIndex::new(p.clone());
    ix.update(items, &[]);
    ix.pairs()
}
