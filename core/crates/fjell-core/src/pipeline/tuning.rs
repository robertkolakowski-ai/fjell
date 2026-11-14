//! Terskler fra sensitivitet (V1). Del av `pipeline` (L3).
//!
//! Avhenger av: cluster (`Params`), series (`SeriesParams`), model (`SuggestionKind`). Grensesnitt:
//! - `Tuning { visual, blur_scale, series }` og `Tuning::default()` (= `Normal` overalt = MVP-verdiene).
//! - `tuning(level)`: bygger `Tuning` fra en funksjon `SuggestionKind -> Option<kode>` der koden er
//!   `"low" | "normal" | "high"` (samme koder som `setting.sensitivity.<type>`). Ukjent eller manglende
//!   kode gir `Normal`, så eldre databaser leses uendret.
//! - `tuning_from_store(&Store)`: leser `setting` og kaller `tuning`.
//!
//! Sensitivitet styrer terskler, aldri hva som er lov: appen foreslår bare. Verdiene er startverdier som
//! ikke er kalibrert mot golden-korpus ennå (docs/v1/V1-PLAN.md, åpent spørsmål 3).

use crate::cluster::Params;
use crate::model::SuggestionKind;
use crate::series::SeriesParams;
use crate::store::Store;

/// Effektive terskler for gruppering og serier.
#[derive(Clone, Debug, PartialEq)]
pub struct Tuning {
    /// Visuell klynging (`max_dist` følger sensitiviteten for `visual_dup`).
    pub visual: Params,
    /// Faktor på uskarphetsterskelen (`blur_threshold`); 1,0 = MVP.
    pub blur_scale: f32,
    /// Serier (`gap_secs`, `max_dist` og `eyes_min_confidence` følger sensitiviteten for `series`).
    pub series: SeriesParams,
}

impl Default for Tuning {
    fn default() -> Tuning {
        Tuning {
            visual: Params::default(),
            blur_scale: 1.0,
            series: SeriesParams::default(),
        }
    }
}

/// Lav, normal eller høy fra lagret kode.
fn step(code: Option<&str>) -> i32 {
    match code {
        Some("low") => -1,
        Some("high") => 1,
        _ => 0,
    }
}

/// Bygger terskler. `level` gir lagret kode per forslagstype (`None` = ikke satt = normal).
pub fn tuning(level: impl Fn(SuggestionKind) -> Option<String>) -> Tuning {
    let get = |k| step(level(k).as_deref());
    let mut t = Tuning::default();
    let v = get(SuggestionKind::VisualDup);
    t.visual.max_dist = (t.visual.max_dist as i32 + 2 * v).max(1) as u32;
    t.blur_scale = match get(SuggestionKind::Blurry) {
        -1 => 0.7,
        1 => 1.3,
        _ => 1.0,
    };
    let s = get(SuggestionKind::Series);
    t.series.gap_secs = match s {
        -1 => 5,
        1 => 20,
        _ => t.series.gap_secs,
    };
    t.series.max_dist = (t.series.max_dist as i32 + 2 * s).max(1) as u32;
    t.series.eyes_min_confidence = match s {
        -1 => 0.9,
        1 => 0.7,
        _ => t.series.eyes_min_confidence,
    };
    t
}

/// Leser `sensitivity.<type>` fra `setting`. Lesefeil gir normal (aldri strengere enn MVP).
pub fn tuning_from_store(store: &Store) -> Tuning {
    tuning(|k| {
        store
            .setting(&format!("sensitivity.{}", k.as_str()))
            .ok()
            .flatten()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_normal_everywhere() {
        assert_eq!(tuning(|_| None), Tuning::default());
        assert_eq!(tuning(|_| Some("normal".into())), Tuning::default());
        assert_eq!(tuning(|_| Some("rubbish".into())), Tuning::default());
    }

    #[test]
    fn low_is_stricter_and_high_looser() {
        let low = tuning(|_| Some("low".into()));
        let high = tuning(|_| Some("high".into()));
        let n = Tuning::default();
        assert!(
            low.visual.max_dist < n.visual.max_dist && n.visual.max_dist < high.visual.max_dist
        );
        assert!(low.blur_scale < 1.0 && high.blur_scale > 1.0);
        assert!(
            low.series.gap_secs < n.series.gap_secs && n.series.gap_secs < high.series.gap_secs
        );
        assert!(low.series.eyes_min_confidence > high.series.eyes_min_confidence);
    }

    #[test]
    fn each_kind_only_moves_its_own_knobs() {
        let t = tuning(|k| (k == SuggestionKind::Blurry).then(|| "high".into()));
        assert_eq!(t.visual, Params::default());
        assert_eq!(t.series, SeriesParams::default());
        assert!(t.blur_scale > 1.0);
    }
}
