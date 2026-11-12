//! Modul `hash` (L1). Se docs/mvp/ARKITEKTUR.md del 2 og PLAN-KJERNE del 3 og 4 (K1, K2a).
//!
//! Avhenger av: model (og `blake3` for filhash). Ingen I/O utover leseren som sendes inn.
//! Grensesnitt (hovedpunkter):
//! - `content_hash(&mut dyn Read) -> io::Result<[u8;32]>`: BLAKE3, strømmende med 64 KiB buffer.
//! - `needs_hash(&[HashCandidate])`: størrelsesforfilter (bare filer med lik størrelse hashes).
//! - `hash_pending(&dyn AssetSource, &[HashCandidate])`: leser bare det forfilteret krever; en
//!   lesefeil på én fil stopper ikke de andre (feilen returneres per fil, så den kan prøves igjen).
//! - `perceptual(&Thumb) -> (Hash64, Hash64)` = (dHash, pHash), `low_info(&Thumb) -> bool`.
//!
//! Alle flyttall-regler er faste (f64, bare +, -, *, / og sqrt; cosinustabellen bygges uten libm),
//! så resultatet er bit-identisk på alle plattformer. Endres noe her: øk `ALGO_VER` i `model`.

use crate::model::{AssetId, AssetSource, Hash64, SourceError, SourceId, Thumb, ThumbFmt};
use std::collections::BTreeMap;
use std::io::{self, Read};

/// Lesebuffer for strømmende hashing.
const BUF_LEN: usize = 64 * 1024;
/// Lavinformasjon: standardavvik for luma under dette (PLAN-KJERNE K2a punkt 4).
const LOW_INFO_STDDEV: f64 = 4.0;
const DCT_N: usize = 32;

#[cfg(test)]
mod tests;

/// BLAKE3 over alle bytes i `r`. Uavhengig av hvor mye hvert `read`-kall gir.
pub fn content_hash(r: &mut dyn Read) -> io::Result<[u8; 32]> {
    let mut h = blake3::Hasher::new();
    let mut buf = vec![0u8; BUF_LEN];
    loop {
        match r.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                h.update(&buf[..n]);
            }
            Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
            Err(e) => return Err(e),
        }
    }
    Ok(*h.finalize().as_bytes())
}

/// Det forfilteret trenger å vite om en aktiv fil.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HashCandidate {
    pub id: AssetId,
    pub source: SourceId,
    pub bytes: u64,
    /// `content_hash` er allerede beregnet.
    pub hashed: bool,
}

/// Uhashede filer som deler størrelse med minst én annen fil i listen (hashet eller ikke).
/// Unike filer får aldri hash, men kommer med hvis en ny fil med lik størrelse dukker opp (lat
/// beregning): da får både den nye og den gamle uhashede filen hash. Sortert på `AssetId`.
pub fn needs_hash(items: &[HashCandidate]) -> Vec<AssetId> {
    let mut by_size: BTreeMap<u64, usize> = BTreeMap::new();
    for it in items {
        *by_size.entry(it.bytes).or_default() += 1;
    }
    let mut out: Vec<AssetId> = items
        .iter()
        .filter(|it| !it.hashed && by_size[&it.bytes] > 1)
        .map(|it| it.id)
        .collect();
    out.sort();
    out.dedup();
    out
}

/// Hasher det `needs_hash` krever, via `AssetSource::read_bytes`. Én feil (f.eks. `Unavailable`)
/// gir `Err` for den filen og stopper ikke resten; kalleren lar `content_hash` stå tom slik at
/// filen forsøkes på nytt neste skanning.
pub fn hash_pending(
    src: &dyn AssetSource,
    items: &[HashCandidate],
) -> Vec<(AssetId, Result<[u8; 32], SourceError>)> {
    let wanted = needs_hash(items);
    let by_id: BTreeMap<AssetId, &HashCandidate> = items.iter().map(|c| (c.id, c)).collect();
    wanted
        .into_iter()
        .map(|id| {
            let res = src.read_bytes(&by_id[&id].source).and_then(|mut r| {
                content_hash(&mut r).map_err(|e| SourceError::Failed(e.to_string()))
            });
            (id, res)
        })
        .collect()
}

/// Luma (0..=255, heltallsvekter 77/150/29 delt på 256) som f64. `None` for ugyldig miniatyr.
fn luma(t: &Thumb) -> Option<Vec<f64>> {
    let (w, h) = (t.w as usize, t.h as usize);
    if w == 0 || h == 0 || t.data.len() != w * h * t.fmt.bytes_per_pixel() {
        return None;
    }
    Some(match t.fmt {
        ThumbFmt::Gray8 => t.data.iter().map(|&v| v as f64).collect(),
        // Alfa ignoreres. Grå RGBA (r=g=b=v) gir nøyaktig v, så Gray8 og RGBA gir lik hash.
        // Lengden er sjekket over (bredde × høyde × 4), så resten etter 4-delingen er alltid tom.
        ThumbFmt::Rgba8 => t
            .data
            .as_chunks::<4>()
            .0
            .iter()
            .map(|p| ((77 * p[0] as u32 + 150 * p[1] as u32 + 29 * p[2] as u32) >> 8) as f64)
            .collect(),
    })
}

/// Vekter for areal-gjennomsnitt langs én akse: for hver målcelle (kildeindeks, vekt), sum 1.
fn axis_weights(src: usize, dst: usize) -> Vec<Vec<(usize, f64)>> {
    let scale = src as f64 / dst as f64;
    (0..dst)
        .map(|d| {
            let (a, b) = (d as f64 * scale, (d + 1) as f64 * scale);
            let first = a as usize;
            let last = (b.ceil() as usize).min(src);
            let mut v: Vec<(usize, f64)> = (first..last)
                .map(|i| {
                    let lo = a.max(i as f64);
                    let hi = b.min((i + 1) as f64);
                    (i, (hi - lo).max(0.0) / scale)
                })
                .filter(|&(_, w)| w > 0.0)
                .collect();
            if v.is_empty() {
                v.push((first.min(src - 1), 1.0));
            }
            v
        })
        .collect()
}

/// Deterministisk areal-gjennomsnitt-nedskalering (også oppskalering ved små miniatyrer).
fn resize_area(px: &[f64], w: usize, h: usize, dw: usize, dh: usize) -> Vec<f64> {
    let (wx, wy) = (axis_weights(w, dw), axis_weights(h, dh));
    let mut rows = vec![0.0; h * dw];
    for y in 0..h {
        for (dx, ws) in wx.iter().enumerate() {
            rows[y * dw + dx] = ws.iter().map(|&(x, k)| px[y * w + x] * k).sum();
        }
    }
    let mut out = vec![0.0; dw * dh];
    for (dy, ws) in wy.iter().enumerate() {
        for dx in 0..dw {
            out[dy * dw + dx] = ws.iter().map(|&(y, k)| rows[y * dw + dx] * k).sum();
        }
    }
    out
}

/// cos(m·π/64) for heltall m uten libm: symmetrireduksjon og Taylor-rekke (bare +, *, /).
fn det_cos(m: usize) -> f64 {
    let m = m % 128;
    let m = if m > 64 { 128 - m } else { m };
    let (m, sign) = if m > 32 { (64 - m, -1.0) } else { (m, 1.0) };
    let x = m as f64 * std::f64::consts::PI / 64.0;
    let (mut term, mut sum) = (1.0f64, 1.0f64);
    for k in 1..=14 {
        term *= -x * x / ((2 * k - 1) as f64 * (2 * k) as f64);
        sum += term;
    }
    sign * sum
}

/// dHash: 9×8-nedskalering, bit = venstre piksel > høyre. Bit for (rad y, kolonne x) ligger på
/// posisjon 63 - (8y + x), så øverst til venstre er mest signifikante bit.
fn dhash(px: &[f64], w: usize, h: usize) -> u64 {
    let s = resize_area(px, w, h, 9, 8);
    let mut bits = 0u64;
    for y in 0..8 {
        for x in 0..8 {
            bits = (bits << 1) | (s[y * 9 + x] > s[y * 9 + x + 1]) as u64;
        }
    }
    bits
}

/// pHash: 32×32-nedskalering, 2D DCT-II (ikke normalisert), lavfrekvent blokk u,v = 1..=8
/// (8×8 uten DC-rad og DC-kolonne), bit = koeffisient > median (snitt av de to midterste av 64).
/// Bitrekkefølge: rad v, kolonne u, første koeffisient på bit 63.
fn phash(px: &[f64], w: usize, h: usize) -> u64 {
    let s = resize_area(px, w, h, DCT_N, DCT_N);
    let cos: Vec<f64> = (0..DCT_N * DCT_N)
        .map(|i| det_cos((i / DCT_N) * (2 * (i % DCT_N) + 1)))
        .collect(); // cos[k * N + n] = cos(π k (2n+1) / 64)
    let mut rows = vec![0.0; DCT_N * 9]; // bare kolonnefrekvens 0..=8 trengs
    for y in 0..DCT_N {
        for u in 0..9 {
            rows[y * 9 + u] = (0..DCT_N)
                .map(|x| s[y * DCT_N + x] * cos[u * DCT_N + x])
                .sum();
        }
    }
    let mut coef = Vec::with_capacity(64);
    for v in 1..=8 {
        for u in 1..=8 {
            coef.push(
                (0..DCT_N)
                    .map(|y| rows[y * 9 + u] * cos[v * DCT_N + y])
                    .sum::<f64>(),
            );
        }
    }
    let mut sorted = coef.clone();
    sorted.sort_by(|a, b| a.total_cmp(b));
    let median = (sorted[31] + sorted[32]) / 2.0;
    coef.iter()
        .fold(0u64, |b, &c| (b << 1) | (c > median) as u64)
}

/// (dHash, pHash) fra miniatyr. Ugyldig miniatyr gir (0, 0) og er også `low_info`. Panikker aldri.
pub fn perceptual(t: &Thumb) -> (Hash64, Hash64) {
    match luma(t) {
        Some(px) => {
            let (w, h) = (t.w as usize, t.h as usize);
            (Hash64(dhash(&px, w, h)), Hash64(phash(&px, w, h)))
        }
        None => (Hash64(0), Hash64(0)),
    }
}

/// Nesten ensfarget bilde (standardavvik for luma < 4) eller ugyldig miniatyr: hashene er
/// ikke meningsfulle og bildet utelates fra visuell klynging.
pub fn low_info(t: &Thumb) -> bool {
    let Some(px) = luma(t) else { return true };
    let n = px.len() as f64;
    let mean = px.iter().sum::<f64>() / n;
    let var = px.iter().map(|v| (v - mean) * (v - mean)).sum::<f64>() / n;
    var.sqrt() < LOW_INFO_STDDEV
}
