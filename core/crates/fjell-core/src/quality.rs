//! Modul `quality` (L1). Se docs/mvp/ARKITEKTUR.md del 2 og PLAN-KJERNE del 5 (K3).
//!
//! Avhenger av: model
//! Grensesnitt (hovedpunkter):
//! - Ren funksjon: `blur_score(&Thumb) -> f32` (høy = skarp, lav = uskarp).
//! - `LOW_CONTRAST`: «ikke vurderbar», aldri uskarp. `BLUR_ABS_MAX`: startverdi for terskelen.
//! - Terskel over biblioteket (P10) og konfidens hører til `suggest`, ikke her.
//!
//! Algoritme: luma, Laplace-kjerne 3×3 på indre piksler, `Var(L) / (Var(I) + 1)`.
//! Alt summeres i heltall (luma ganger 1000) og konverteres til flyttall først helt til slutt,
//! så resultatet er bit for bit likt på alle plattformer (K3-6). Én gjennomgang, tre radbuffere.

use crate::model::{Thumb, ThumbFmt};

/// Score for bilder som ikke kan vurderes (flatt, for lite): aldri uskarpe.
pub const LOW_CONTRAST: f32 = 1000.0;
/// Varians av luma under denne verdien regnes som lav kontrast.
pub const MIN_LUMA_VARIANCE: f64 = 4.0;
/// Startverdi for `ABS_MAX` (PLAN-KJERNE 5.3): uskarpt hvis score under `min(P10, BLUR_ABS_MAX)`.
/// Satt fra golden-korpus (skarpe 256 px-miniatyrer scorer 0,19–0,80, σ ≥ 1 % under 0,006);
/// kalibreres på nytt mot ekte foto i K8. Golden-testen K3-3 bruker denne verdien.
pub const BLUR_ABS_MAX: f32 = 0.006;

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

/// Uskarphetsscore. Mindre enn 3×3, ugyldig datalengde eller flatt bilde gir `LOW_CONTRAST`.
pub fn blur_score(t: &Thumb) -> f32 {
    let (w, h) = (t.w as usize, t.h as usize);
    if w < 3 || h < 3 || t.data.len() < w * h * t.fmt.bytes_per_pixel() {
        return LOW_CONTRAST;
    }
    let mut rows = [vec![0i64; w], vec![0i64; w], vec![0i64; w]];
    fill_row(t, 0, &mut rows[0]);
    fill_row(t, 1, &mut rows[1]);
    // Summer for luma (alle piksler) og Laplace (indre piksler); heltall, eksakte.
    let (mut s_i, mut s_ii) = (0i128, 0i128);
    for &v in rows[0].iter().chain(rows[1].iter()) {
        s_i += v as i128;
        s_ii += v as i128 * v as i128;
    }
    let (mut s_l, mut s_ll) = (0i128, 0i128);
    for y in 1..h - 1 {
        // rows[(y-1)%3] og rows[y%3] er lastet; last rad y+1.
        fill_row(t, y + 1, &mut rows[(y + 1) % 3]);
        for &v in rows[(y + 1) % 3].iter() {
            s_i += v as i128;
            s_ii += v as i128 * v as i128;
        }
        let (up, mid, down) = (&rows[(y - 1) % 3], &rows[y % 3], &rows[(y + 1) % 3]);
        for x in 1..w - 1 {
            let l = (up[x] + down[x] + mid[x - 1] + mid[x + 1] - 4 * mid[x]) as i128;
            s_l += l;
            s_ll += l * l;
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
    (var_l / (var_i + 1.0)) as f32
}

#[cfg(test)]
mod tests;
