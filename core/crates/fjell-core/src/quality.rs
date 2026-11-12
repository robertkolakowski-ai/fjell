//! Modul `quality` (L1). Se docs/mvp/ARKITEKTUR.md del 2 og PLAN-KJERNE del 5 (K3) og del 9 (K8).
//!
//! Avhenger av: model
//! Grensesnitt (hovedpunkter):
//! - Ren funksjon: `blur_score(&Thumb) -> f32` (høy = skarp, lav = uskarp, alltid > 0).
//! - `LOW_CONTRAST`: «ikke vurderbar», aldri uskarp. `BLUR_ABS_MAX`: terskelen (K8-kalibrert).
//! - Terskel over biblioteket (P10) og konfidens hører til `suggest`, ikke her.
//!
//! Algoritme (v1, K8): fem støyrobuste trekk fra retningsbestemte gradientkorrelasjoner, slått
//! sammen lineært og kalibrert på treningsdelen av korpuset (`fjell-testdata::blurset`).
//! For hver av fire retninger d (0°, 90°, 45°, 135°) er `g(p) = I(p+d) − I(p)` og
//! `q_k = E[g(p)·g(p+k·d)]` (middel over gyldige par). Hvit sensorstøy bidrar bare til q₀ og q₁,
//! så `ρ₁ = q₁/q₀` og `ρ₂ = q₂/q₀` sier hvor «glatt» gradientfeltet er i retningen uten at støy
//! skjuler uskarphet. Uskarphet langs én retning (bevegelse) gir høy ρ i den retningen, fokusfeil
//! i alle. `E = q₀/Var(I)` er normalisert gradientenergi (kontrastuavhengig), og `lap` er den
//! gamle Laplace-målingen `Var(L)/(Var(I)+1)` (følsom for lett uskarphet på finkornet innhold).
//! Trekkene: min ρ₂, min ρ₁, maks ρ₁ over retninger, tredje minste E, og `lap/(lap+0,1)`.
//! `z = Σ wᵢ·fᵢ + b`; uskarpt hvis `z ≥ Z_THR`. Scoren er sammensatt: uskarp side
//! `BLUR_ABS_MAX/(1 + m/D)` med `m = z − Z_THR` (faller mot 0 jo tydeligere uskarpt), skarp side
//! `BLUR_ABS_MAX·(1 + lap/LAP_REF)` (rangerer skarpe bilder etter fin-skala-energi, som
//! nabo-/keeper-rangeringen og K3-2 forutsetter). Scoren er alltid > 0 og strengt monoton innenfor
//! hver side; klassifiseringen (score < `BLUR_ABS_MAX`) følger bare z.
//! Alt summeres i heltall (luma ganger 1000) og konverteres til flyttall først helt til slutt;
//! bare +, −, ×, ÷ og sammenligning på f64 (IEEE-eksakt), ingen exp/ln, så resultatet er bit for
//! bit likt på alle plattformer (K3-6). Én gjennomgang, fire radbuffere.

use crate::model::{Thumb, ThumbFmt};

/// Score for bilder som ikke kan vurderes (flatt, for lite): aldri uskarpe.
pub const LOW_CONTRAST: f32 = 1000.0;
/// Varians av luma under denne verdien regnes som lav kontrast.
pub const MIN_LUMA_VARIANCE: f64 = 4.0;
/// Minste side (piksler) for å kunne vurdere skarphet; mindre gir `LOW_CONTRAST`.
pub const MIN_SIDE: usize = 8;
/// Terskel (PLAN-KJERNE 5.3): uskarpt hvis score under `min(P10, BLUR_ABS_MAX)`. Scoren er
/// normalisert slik at kalibrert beslutningsgrense (`Z_THR`) gir nøyaktig denne verdien (K8).
pub const BLUR_ABS_MAX: f32 = 0.006;

/// Kalibrerte vekter (logistisk regresjon på treningsdelen, K8), i rekkefølgen
/// min ρ₂, min ρ₁, maks ρ₁, tredje minste E, lap/(lap+`LAP_SAT`).
const WEIGHTS: [f64; 5] = [9.855, -11.2, 11.55, -1.575, 4.448];
const BIAS: f64 = -5.378;
/// Beslutningsgrense for `z`: høyest recall med presisjon ≥ 0,96 på treningsdelen (recall 0,957);
/// alle ti K3-2-motivene er monotone ved denne verdien.
const Z_THR: f64 = -0.4;
/// Fin-skala-energi (`lap`) som dobler scoren over terskelen på skarp side.
const LAP_REF: f64 = 0.3;
const LAP_SAT: f64 = 0.1;
/// Marginen (i z-enheter) som halverer scoren under terskelen.
const MARGIN_SCALE: f64 = 1.5;
/// Øvre grense slik at en vurderbar score alltid er under `LOW_CONTRAST`.
const SCORE_CAP: f64 = 500.0;

/// Luma ganger 1000 (heltall, BT.601: 299/587/114).
fn luma_milli(t: &Thumb, x: usize, y: usize) -> i64 {
    let w = t.w as usize;
    match t.fmt {
        ThumbFmt::Gray8 => t.data[y * w + x] as i64 * 1000,
        ThumbFmt::Rgba8 => {
            let o = (y * w + x) * 4;
            299 * t.data[o] as i64 + 587 * t.data[o + 1] as i64 + 114 * t.data[o + 2] as i64
        }
    }
}

fn fill_row(t: &Thumb, y: usize, buf: &mut [i64]) {
    for (x, v) in buf.iter_mut().enumerate() {
        *v = luma_milli(t, x, y);
    }
}

/// Retninger (dx, dy): 0° (H), 90° (V), 45° og 135°.
const DIRS: [(isize, usize); 4] = [(1, 0), (0, 1), (1, 1), (-1, 1)];

/// Summer og antall for `q_k` (k = 0, 1, 2) per retning.
#[derive(Default)]
struct Acc {
    s: [[i128; 3]; 4],
    n: [[u64; 3]; 4],
}

/// Legger til `g(x, y0)·g(x + k·dx, y0 + k·dy)` for alle gyldige x. Radene `y0 … y0 + k·dy + dy`
/// må ligge i ringbufferen (rad r ligger i `rows[r % 4]`).
fn add_products(acc: &mut Acc, rows: &[Vec<i64>; 4], w: usize, y0: usize, d: usize, k: usize) {
    let (dx, dy) = DIRS[d];
    let (ki, wi) = (k as isize, w as isize);
    let lo = [0, -dx, -ki * dx, -(ki + 1) * dx]
        .into_iter()
        .max()
        .unwrap();
    let hi = [wi, wi - dx, wi - ki * dx, wi - (ki + 1) * dx]
        .into_iter()
        .min()
        .unwrap();
    let (a0, a1) = (&rows[y0 % 4], &rows[(y0 + dy) % 4]);
    let (b0, b1) = (&rows[(y0 + k * dy) % 4], &rows[(y0 + k * dy + dy) % 4]);
    let (mut sum, mut n) = (0i128, 0u64);
    for x in lo..hi {
        let g1 = a1[(x + dx) as usize] - a0[x as usize];
        let xk = x + ki * dx;
        let g2 = b1[(xk + dx) as usize] - b0[xk as usize];
        sum += (g1 * g2) as i128;
        n += 1;
    }
    acc.s[d][k] += sum;
    acc.n[d][k] += n;
}

/// z = Σ wᵢ·fᵢ + b (høy = uskarp) fra de fem trekkene.
fn blur_z(acc: &Acc, var_i: f64, lap: f64) -> f64 {
    let q = |d: usize, k: usize| {
        if acc.n[d][k] == 0 {
            0.0
        } else {
            acc.s[d][k] as f64 / acc.n[d][k] as f64
        }
    };
    let (mut r1, mut r2, mut e) = ([0.0f64; 4], [0.0f64; 4], [0.0f64; 4]);
    for d in 0..4 {
        let q0 = q(d, 0);
        if q0 > 0.0 {
            r1[d] = q(d, 1) / q0;
            r2[d] = q(d, 2) / q0;
            e[d] = q0 / var_i;
        }
    }
    let fmin = |v: &[f64; 4]| v.iter().copied().fold(f64::INFINITY, f64::min);
    let fmax = |v: &[f64; 4]| v.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    e.sort_by(f64::total_cmp);
    let f = [fmin(&r2), fmin(&r1), fmax(&r1), e[2], lap / (lap + LAP_SAT)];
    f.iter().zip(WEIGHTS).map(|(a, b)| a * b).sum::<f64>() + BIAS
}

/// Uskarphetsscore. Mindre enn `MIN_SIDE`, ugyldig datalengde eller flatt bilde gir `LOW_CONTRAST`.
/// Ellers: `BLUR_ABS_MAX` ved beslutningsgrensen, under den = uskarpt, over den = skarpt.
pub fn blur_score(t: &Thumb) -> f32 {
    let (w, h) = (t.w as usize, t.h as usize);
    if w < MIN_SIDE || h < MIN_SIDE || t.data.len() < w * h * t.fmt.bytes_per_pixel() {
        return LOW_CONTRAST;
    }
    let mut rows = [vec![0i64; w], vec![0i64; w], vec![0i64; w], vec![0i64; w]];
    let mut acc = Acc::default();
    // Summer for luma (alle piksler) og Laplace (indre piksler); heltall, eksakte.
    let (mut s_i, mut s_ii, mut s_l, mut s_ll) = (0i128, 0i128, 0i128, 0i128);
    for y in 0..h {
        fill_row(t, y, &mut rows[y % 4]);
        for &v in rows[y % 4].iter() {
            s_i += v as i128;
            s_ii += v as i128 * v as i128;
        }
        // Vannrette produkter (alle lag) på rad y.
        for k in 0..3 {
            add_products(&mut acc, &rows, w, y, 0, k);
        }
        // Ikke-vannrette retninger: anker y0 = y − 1 − k for lag k (siste rad som trengs er y).
        for k in 0..3 {
            if y > k {
                for d in 1..4 {
                    add_products(&mut acc, &rows, w, y - 1 - k, d, k);
                }
            }
        }
        if y >= 2 {
            let (up, mid, down) = (&rows[(y - 2) % 4], &rows[(y - 1) % 4], &rows[y % 4]);
            for x in 1..w - 1 {
                let l = (up[x] + down[x] + mid[x - 1] + mid[x + 1] - 4 * mid[x]) as i128;
                s_l += l;
                s_ll += l * l;
            }
        }
    }
    let n_i = (w * h) as f64;
    let n_l = ((w - 2) * (h - 2)) as f64;
    // Varians i enheten (luma*1000)²; del på 1e6 gir luma-enheter.
    let var_i = (s_ii as f64 / n_i - (s_i as f64 / n_i).powi(2)) / 1e6;
    if var_i < MIN_LUMA_VARIANCE {
        return LOW_CONTRAST;
    }
    let var_l = (s_ll as f64 / n_l - (s_l as f64 / n_l).powi(2)) / 1e6;
    let lap = var_l / (var_i + 1.0);
    // q-ene er i (luma*1000)²; E = q0 / Var(I) skal være dimensjonsløs, så bruk Var(I) i samme enhet.
    let z = blur_z(&acc, var_i * 1e6, lap);
    if z.is_nan() {
        return LOW_CONTRAST;
    }
    let t_abs = BLUR_ABS_MAX as f64;
    let s = if z >= Z_THR {
        t_abs / (1.0 + (z - Z_THR) / MARGIN_SCALE)
    } else {
        t_abs * (1.0 + lap / LAP_REF)
    };
    s.min(SCORE_CAP) as f32
}

#[cfg(test)]
mod tests;
