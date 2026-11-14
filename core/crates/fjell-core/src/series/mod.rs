//! Modul `series` (L2). V1-gren `series`: serier (rekker av nesten like bilder tatt tett i tid) og valg av beste bilde. Se docs/v1/V1-PLAN.md og docs/mvp/ARKITEKTUR.md del 2.
//!
//! Avhenger av: model, quality (bare `LOW_CONTRAST`). Ren funksjon, ingen I/O og ingen lagring.
//! Grensesnitt:
//! - `SeriesItem`/`SeriesParams`: ett kandidatbilde (tid, dHash, pHash, skarphet, valgfri `EyeReport`) og terskler.
//! - `build_series(&[SeriesItem], &SeriesParams) -> Vec<Series>`: deler på tidsglipp (`gap_secs`), kobler
//!   bilder med Hamming <= `max_dist` på både dHash og pHash (union-find, bare mot de `max_len` nærmeste naboene i
//!   tid), og deler serier over `max_len` i tidsrekkefølge. Serier har minst to bilder. Resultatet er uavhengig av
//!   inndatarekkefølgen.
//! - `rank_series(&[SeriesItem], &SeriesParams) -> Option<Series>`: rangerer ett kjent sett (ingen gruppering).
//! - `Series::with_keeper(id, &[SeriesItem], &SeriesParams)`: samme serie med brukerens valg som keeper.
//! - `Series { keeper, keeper_reason, members }`: medlemmer beste først med score, begrunnelse og detalj.
//!
//! Rangering (fast rekkefølge, ikke parvis sammenligning): 1) bilder med lukkede øyne (sikkerhet >=
//! `eyes_min_confidence`) rangeres under alle andre, 2) høyere skarphet først (relativt til seriens skarpeste;
//! `LOW_CONTRAST` teller som 0), 3) lavest `AssetId`. Uten øyeanalyse (`None`, for lav sikkerhet) regnes øynene
//! som ukjente, aldri som lukkede. Keeper får `KeeperEyesOpen` bare når øyne avgjorde valget (skarpeste bilde
//! hadde lukkede øyne), ellers `KeeperBestOverall`. Øvrige får `SeriesMember` (`Position`) eller `ClosedEyes`
//! (`Count` = ansikter med lukkede øyne). Appen foreslår; keeper får aldri forslag, valget kan byttes av brukeren.
//!
//! Eier hele mappen `src/series/` og `store/series.rs`; ingen andre grener rører dem.

use crate::model::{AssetId, EyeReport, Hash64, ReasonCode, ReasonDetail};
use crate::quality::LOW_CONTRAST;
use std::collections::BTreeMap;

#[cfg(test)]
mod tests;

/// Ett kandidatbilde. Kalleren leverer bare aktive bilder av typen foto som ikke er `low_info`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SeriesItem {
    pub id: AssetId,
    /// Opptakstid i Unix-sekunder (`asset.created_at`).
    pub taken_at: i64,
    pub dhash: Hash64,
    pub phash: Hash64,
    /// `quality::blur_score` (høy = skarp). `LOW_CONTRAST` og ikke-endelige verdier teller som 0.
    pub blur: f32,
    /// `None` = ikke analysert eller analysator utilgjengelig.
    pub eyes: Option<EyeReport>,
}

/// Terskler for serier.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SeriesParams {
    /// Størst tidsavstand (sekunder) mellom nabobilder i en serie.
    pub gap_secs: u32,
    /// Høyeste Hamming på både dHash og pHash. Løsere enn visuelle duplikater (serier har bevegelse).
    pub max_dist: u32,
    /// Største serie (minst 2).
    pub max_len: usize,
    /// Minste sikkerhet for å regne øynene som lukkede.
    pub eyes_min_confidence: f32,
}

impl Default for SeriesParams {
    fn default() -> SeriesParams {
        SeriesParams {
            gap_secs: 10,
            max_dist: 10,
            max_len: 30,
            eyes_min_confidence: 0.8,
        }
    }
}

/// Ett rangert medlem.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RankedMember {
    pub id: AssetId,
    /// 0,0..=1,0. Bilder med lukkede øyne får høyst 0,5, de øvrige minst 0,5.
    pub score: f32,
    pub reason: ReasonCode,
    pub detail: ReasonDetail,
}

/// En serie med minst to bilder. `members[0]` er keeper.
#[derive(Clone, Debug, PartialEq)]
pub struct Series {
    pub keeper: AssetId,
    pub keeper_reason: ReasonCode,
    /// Beste først.
    pub members: Vec<RankedMember>,
}

impl Series {
    /// Sorterte medlems-ID-er (nøkkel for gjenbruk av lagrede serier).
    pub fn ids(&self) -> Vec<AssetId> {
        let mut v: Vec<AssetId> = self.members.iter().map(|m| m.id).collect();
        v.sort();
        v
    }

    /// Samme serie med `keeper` først (brukerens valg). `None` hvis `keeper` ikke er medlem eller `items` ikke
    /// dekker serien. Keeper får `KeeperEyesOpen` bare når øynene er bekreftet åpne, ellers `SeriesMember`
    /// (appen påstår ikke «best» for et manuelt valg). Øvrige beholder rekkefølgen.
    pub fn with_keeper(
        &self,
        keeper: AssetId,
        items: &[SeriesItem],
        p: &SeriesParams,
    ) -> Option<Series> {
        let pos = self.members.iter().position(|m| m.id == keeper)?;
        if pos == 0 {
            return Some(self.clone());
        }
        let by_id: BTreeMap<AssetId, &SeriesItem> = items.iter().map(|i| (i.id, i)).collect();
        let of = self.members.len() as u32;
        let mut order: Vec<RankedMember> = self.members.clone();
        let k = order.remove(pos);
        order.insert(0, k);
        let keeper_reason = if eyes_open(by_id.get(&keeper)?, p) {
            ReasonCode::KeeperEyesOpen
        } else {
            ReasonCode::SeriesMember
        };
        for (i, m) in order.iter_mut().enumerate() {
            let it = by_id.get(&m.id)?;
            if i == 0 {
                m.reason = keeper_reason;
                m.detail = if keeper_reason == ReasonCode::SeriesMember {
                    ReasonDetail::Position { index: 1, of }
                } else {
                    ReasonDetail::None
                };
            } else {
                (m.reason, m.detail) = member_reason(it, i as u32 + 1, of, p);
            }
        }
        Some(Series {
            keeper,
            keeper_reason,
            members: order,
        })
    }
}

fn closed(it: &SeriesItem, p: &SeriesParams) -> bool {
    it.eyes
        .is_some_and(|e| e.faces > 0 && e.closed > 0 && e.confidence >= p.eyes_min_confidence)
}

fn eyes_open(it: &SeriesItem, p: &SeriesParams) -> bool {
    it.eyes
        .is_some_and(|e| e.faces > 0 && e.closed == 0 && e.confidence >= p.eyes_min_confidence)
}

fn sharp(it: &SeriesItem) -> f32 {
    if it.blur.is_finite() && it.blur < LOW_CONTRAST {
        it.blur.max(0.0)
    } else {
        0.0
    }
}

fn member_reason(
    it: &SeriesItem,
    index: u32,
    of: u32,
    p: &SeriesParams,
) -> (ReasonCode, ReasonDetail) {
    if closed(it, p) {
        let n = it.eyes.map_or(0, |e| e.closed) as u32;
        (ReasonCode::ClosedEyes, ReasonDetail::Count(n))
    } else {
        (
            ReasonCode::SeriesMember,
            ReasonDetail::Position { index, of },
        )
    }
}

/// Rangerer ett kjent sett bilder. `None` for færre enn to bilder. Duplikate ID-er telles én gang.
pub fn rank_series(items: &[SeriesItem], p: &SeriesParams) -> Option<Series> {
    let mut uniq: BTreeMap<AssetId, SeriesItem> = BTreeMap::new();
    for it in items {
        uniq.entry(it.id).or_insert(*it);
    }
    if uniq.len() < 2 {
        return None;
    }
    let max_sharp = uniq.values().map(sharp).fold(0.0f32, f32::max);
    let norm = |it: &SeriesItem| {
        if max_sharp > 0.0 {
            sharp(it) / max_sharp
        } else {
            0.0
        }
    };
    let mut v: Vec<(SeriesItem, bool, f32)> = uniq
        .values()
        .map(|it| (*it, closed(it, p), norm(it)))
        .collect();
    // Skarpeste uten hensyn til øyne (stigende id, så første høyeste = lavest id).
    let mut sharpest = v[0];
    for x in &v[1..] {
        if x.2 > sharpest.2 {
            sharpest = *x;
        }
    }
    v.sort_by(|a, b| {
        a.1.cmp(&b.1)
            .then(b.2.total_cmp(&a.2))
            .then(a.0.id.cmp(&b.0.id))
    });
    let of = v.len() as u32;
    let keeper = v[0].0.id;
    let keeper_reason = if keeper != sharpest.0.id {
        ReasonCode::KeeperEyesOpen
    } else {
        ReasonCode::KeeperBestOverall
    };
    let members = v
        .iter()
        .enumerate()
        .map(|(i, (it, is_closed, n))| {
            let score = if *is_closed { 0.5 * n } else { 0.5 + 0.5 * n };
            let (reason, detail) = if i == 0 {
                (keeper_reason, ReasonDetail::None)
            } else {
                member_reason(it, i as u32 + 1, of, p)
            };
            RankedMember {
                id: it.id,
                score,
                reason,
                detail,
            }
        })
        .collect();
    Some(Series {
        keeper,
        keeper_reason,
        members,
    })
}

struct Dsu(Vec<usize>);

impl Dsu {
    fn find(&mut self, x: usize) -> usize {
        let mut r = x;
        while self.0[r] != r {
            r = self.0[r];
        }
        let mut c = x;
        while self.0[c] != r {
            let n = self.0[c];
            self.0[c] = r;
            c = n;
        }
        r
    }

    fn union(&mut self, a: usize, b: usize) {
        let (a, b) = (self.find(a), self.find(b));
        if a != b {
            self.0[a.max(b)] = a.min(b);
        }
    }
}

fn near(a: &SeriesItem, b: &SeriesItem, max: u32) -> bool {
    a.dhash.hamming(b.dhash) <= max && a.phash.hamming(b.phash) <= max
}

/// Grupperer kandidater til serier og rangerer hver. Sortert på første medlem i tid (deretter id).
pub fn build_series(items: &[SeriesItem], p: &SeriesParams) -> Vec<Series> {
    let max_len = p.max_len.max(2);
    let mut sorted: Vec<SeriesItem> = items.to_vec();
    sorted.sort_by_key(|i| (i.taken_at, i.id));
    sorted.dedup_by_key(|i| i.id);
    let mut out = Vec::new();
    let mut start = 0;
    while start < sorted.len() {
        let mut end = start + 1;
        while end < sorted.len()
            && sorted[end]
                .taken_at
                .saturating_sub(sorted[end - 1].taken_at)
                <= p.gap_secs as i64
        {
            end += 1;
        }
        series_in_run(&sorted[start..end], max_len, p, &mut out);
        start = end;
    }
    out
}

fn series_in_run(run: &[SeriesItem], max_len: usize, p: &SeriesParams, out: &mut Vec<Series>) {
    if run.len() < 2 {
        return;
    }
    let mut dsu = Dsu((0..run.len()).collect());
    for i in 0..run.len() {
        for j in i + 1..run.len().min(i + 1 + max_len) {
            if near(&run[i], &run[j], p.max_dist) {
                dsu.union(i, j);
            }
        }
    }
    // Komponenter i tidsrekkefølge (første forekomst), medlemmer i tidsrekkefølge.
    let mut comps: Vec<Vec<usize>> = Vec::new();
    let mut slot: BTreeMap<usize, usize> = BTreeMap::new();
    for i in 0..run.len() {
        let r = dsu.find(i);
        let s = *slot.entry(r).or_insert_with(|| {
            comps.push(Vec::new());
            comps.len() - 1
        });
        comps[s].push(i);
    }
    for comp in comps {
        for chunk in comp.chunks(max_len) {
            let set: Vec<SeriesItem> = chunk.iter().map(|&i| run[i]).collect();
            if let Some(s) = rank_series(&set, p) {
                out.push(s);
            }
        }
    }
}
