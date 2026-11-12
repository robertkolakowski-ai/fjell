//! Modul `suggest` (L2). Se docs/mvp/ARKITEKTUR.md del 2 og PLAN-KJERNE del 7 (K5), TRYGGHET 4.2.
//!
//! Avhenger av: model, quality, screenshot. Rene funksjoner, ingen I/O og ingen lagring.
//! Grensesnitt (hovedpunkter):
//! - `rank_group(&[Member]) -> Option<Ranked>`: keeper og begrunnelse (`None` bare for tom gruppe).
//! - `group_suggestions(GroupKind, &[Member]) -> Option<GroupResult>` og `suggestions(..)`: alle
//!   ikke-keepere får `exact_dup`/`visual_dup`. Keeper får aldri forslag.
//! - `blur_threshold(&[f32])`, `blurry_suggestion(..)`, `screenshot_suggestion(..)`: forslag uten gruppe.
//! - `without_rejected(..)`: nye forslag overstyrer aldri en avvisning (`UNIQUE(asset_id, kind)`).
//!
//! Kjernen setter aldri `accepted` og forhåndsvelger ingenting; valg skjer i UI.
//! Rangeringen er et sett av filtre i fast rekkefølge (ikke parvis sammenligning), så resultatet
//! er uavhengig av medlemsrekkefølge selv om skarphetsregelen (> 10 %) ikke er transitiv.

use crate::model::{AssetId, ProtectFlags, ReasonCode, SuggestionKind};
use crate::quality::{BLUR_ABS_MAX, LOW_CONTRAST};
use crate::screenshot::ShotSignals;

#[cfg(test)]
mod tests;

/// Minste relative skarphetsforskjell som teller (10 %).
const SHARPER_FACTOR: f64 = 1.1;
/// Minste antall vurderbare bilder før P10 brukes (ellers bare `BLUR_ABS_MAX`).
const MIN_SCORES_FOR_P10: usize = 50;

/// Gruppemedlem slik rangeringen ser det.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Member {
    pub id: AssetId,
    pub width: u32,
    pub height: u32,
    pub bytes: u64,
    pub blur_score: f32,
    pub created_at: i64,
    pub protect_flags: ProtectFlags,
}

impl Member {
    fn pixels(&self) -> u64 {
        self.width as u64 * self.height as u64
    }
    /// Vurderbar skarphet (ikke `LOW_CONTRAST`, ikke NaN).
    fn sharpness(&self) -> Option<f64> {
        (self.blur_score < LOW_CONTRAST).then_some(self.blur_score as f64)
    }
}

/// Resultat av rangering: valgt keeper og hvorfor den vant over nest beste.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ranked {
    pub keeper: AssetId,
    pub reason: ReasonCode,
}

/// Høyest `bytes / piksler` mellom a og b, eksakt (kryssmultiplikasjon).
fn bpp_cmp(a: &Member, b: &Member) -> std::cmp::Ordering {
    (a.bytes as u128 * b.pixels() as u128).cmp(&(b.bytes as u128 * a.pixels() as u128))
}

/// Behold bare kandidatene som er best etter `key` (størst verdi).
fn keep_max<K: Ord>(c: &mut Vec<&Member>, key: impl Fn(&Member) -> K) {
    if let Some(best) = c.iter().map(|m| key(m)).max() {
        c.retain(|m| key(m) == best);
    }
}

/// Velger keeper med steg 0–5 fra PLAN-KJERNE 7.2. `c` må ikke være tom.
fn pick<'a>(members: impl Iterator<Item = &'a Member>) -> Option<&'a Member> {
    let mut c: Vec<&Member> = members.collect();
    // Steg 0: beskyttede foretrekkes (ingen egen begrunnelse).
    if c.iter().any(|m| m.protect_flags.is_protected()) {
        c.retain(|m| m.protect_flags.is_protected());
    }
    // Steg 1: flest piksler.
    keep_max(&mut c, |m| m.pixels());
    // Steg 2: minst komprimering (bytes per piksel).
    if let Some(&first) = c.first() {
        let best = c
            .iter()
            .fold(first, |b, &m| if bpp_cmp(m, b).is_gt() { m } else { b });
        c.retain(|m| bpp_cmp(m, best).is_eq());
    }
    // Steg 3: skarpest, bare mot vurderbare bilder og bare ved > 10 % forskjell.
    let best = c
        .iter()
        .filter_map(|m| m.sharpness())
        .fold(f64::MIN, f64::max);
    c.retain(|m| m.sharpness().is_none_or(|s| s * SHARPER_FACTOR >= best));
    // Steg 4 og 5: eldst, deretter lavest id.
    c.into_iter().min_by_key(|m| (m.created_at, m.id))
}

/// Første kriterium som skiller keeper fra nest beste (steg 0 gir ingen begrunnelse).
fn reason_vs(k: &Member, r: &Member) -> ReasonCode {
    if k.pixels() > r.pixels() {
        ReasonCode::HigherResolution
    } else if k.pixels() == r.pixels() && bpp_cmp(k, r).is_gt() {
        ReasonCode::LessCompressed
    } else if k.pixels() == r.pixels() && bpp_cmp(k, r).is_eq() && sharper(k, r) {
        ReasonCode::Sharper
    } else {
        ReasonCode::Older
    }
}

fn sharper(a: &Member, b: &Member) -> bool {
    matches!((a.sharpness(), b.sharpness()), (Some(x), Some(y)) if x > y * SHARPER_FACTOR)
}

/// Rangerer en gruppe (K5). Deterministisk og uavhengig av medlemsrekkefølge.
/// Én keeper per gruppe; en gruppe med ett medlem gir `Older`. `None` for tom gruppe.
pub fn rank_group(members: &[Member]) -> Option<Ranked> {
    let keeper = pick(members.iter())?;
    let reason = match pick(members.iter().filter(|m| m.id != keeper.id)) {
        Some(runner_up) => reason_vs(keeper, runner_up),
        None => ReasonCode::Older,
    };
    Some(Ranked {
        keeper: keeper.id,
        reason,
    })
}

/// Gruppetype med den informasjonen konfidensen trenger.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GroupKind {
    Exact,
    /// Største Hamming-avstand (dHash/pHash) mellom to medlemmer i gruppen.
    Visual {
        max_hamming: u32,
    },
}

/// Forslag til ett bilde. `group` er gruppens indeks i kallerens liste (`None` for uskarp/skjermbilde).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Suggestion {
    pub asset_id: AssetId,
    pub kind: SuggestionKind,
    pub group: Option<usize>,
    pub reason: ReasonCode,
    pub confidence: f32,
    /// Skjermbildesignaler (bitmaske, 0 for andre typer); grunnlag for begrunnelseskodene.
    pub signals: ShotSignals,
}

/// Rangert gruppe: keeper med begrunnelse, og forslag for alle andre medlemmer.
#[derive(Clone, Debug, PartialEq)]
pub struct GroupResult {
    pub keeper: AssetId,
    pub keeper_reason: ReasonCode,
    pub suggestions: Vec<Suggestion>,
}

/// Visuell konfidens: `1 − maxHamming/7`, avrundet ned til 0,05-steg (heltallsregning).
pub fn visual_confidence(max_hamming: u32) -> f32 {
    let h = max_hamming.min(7);
    let pct = (7 - h) * 100 / 7;
    (pct / 5 * 5) as f32 / 100.0
}

/// Forslag for én gruppe. Eksakt gruppe: keeper får alltid `Older`, konfidens 1,0.
pub fn group_suggestions(kind: GroupKind, members: &[Member]) -> Option<GroupResult> {
    group_at(0, kind, members).map(|mut r| {
        r.suggestions.iter_mut().for_each(|s| s.group = None);
        r
    })
}

fn group_at(index: usize, kind: GroupKind, members: &[Member]) -> Option<GroupResult> {
    let ranked = rank_group(members)?;
    let (sk, reason, confidence) = match kind {
        GroupKind::Exact => (SuggestionKind::ExactDup, ReasonCode::ExactCopy, 1.0),
        GroupKind::Visual { max_hamming } => (
            SuggestionKind::VisualDup,
            ReasonCode::VisualDuplicate,
            visual_confidence(max_hamming),
        ),
    };
    let mut suggestions: Vec<Suggestion> = members
        .iter()
        .filter(|m| m.id != ranked.keeper)
        .map(|m| Suggestion {
            asset_id: m.id,
            kind: sk,
            group: Some(index),
            reason,
            confidence,
            signals: ShotSignals::default(),
        })
        .collect();
    suggestions.sort_by_key(|s| s.asset_id);
    let keeper_reason = if kind == GroupKind::Exact {
        ReasonCode::Older
    } else {
        ranked.reason
    };
    Some(GroupResult {
        keeper: ranked.keeper,
        keeper_reason,
        suggestions,
    })
}

/// Forslag for mange grupper; `Suggestion::group` er indeksen i `groups`. Tomme grupper hoppes over.
pub fn suggestions(groups: &[(GroupKind, &[Member])]) -> Vec<GroupResult> {
    groups
        .iter()
        .enumerate()
        .filter_map(|(i, (k, m))| group_at(i, *k, m))
        .collect()
}

/// Uskarphetsterskel: `min(P10, BLUR_ABS_MAX)` ved minst 50 vurderbare scorer, ellers `BLUR_ABS_MAX`.
/// `LOW_CONTRAST` og ugyldige scorer teller ikke med.
pub fn blur_threshold(scores: &[f32]) -> f32 {
    let mut v: Vec<f32> = scores
        .iter()
        .copied()
        .filter(|s| *s < LOW_CONTRAST)
        .collect();
    if v.len() < MIN_SCORES_FOR_P10 {
        return BLUR_ABS_MAX;
    }
    v.sort_by(f32::total_cmp);
    v[(v.len() - 1) / 10].min(BLUR_ABS_MAX)
}

/// Uskarpt hvis vurderbar og under terskelen. Konfidens 0,5 ved terskelen, 1,0 ved score 0,
/// lineært og avrundet ned til 0,05-steg (startverdi, kalibreres i K8).
pub fn blurry_suggestion(score: f32, threshold: f32) -> Option<f32> {
    if score.is_nan() || score >= LOW_CONTRAST || score >= threshold || threshold <= 0.0 {
        return None;
    }
    let frac = (1.0 - score.max(0.0) / threshold).clamp(0.0, 1.0);
    let pct = (50.0 + 50.0 * frac) as u32;
    Some((pct / 5 * 5) as f32 / 100.0)
}

/// Skjermbildeforslag (konfidens fra signalantall). Standard er `require_strong = true`
/// (`screenshot::DEFAULT_REQUIRE_STRONG`, K4-3); bruk `screenshot_suggestion_default` for den.
pub fn screenshot_suggestion_default(
    asset_id: AssetId,
    signals: ShotSignals,
) -> Option<Suggestion> {
    screenshot_suggestion(asset_id, signals, crate::screenshot::DEFAULT_REQUIRE_STRONG)
}

/// Som `screenshot_suggestion` med eksplisitt `require_strong`.
pub fn screenshot_suggestion(
    asset_id: AssetId,
    signals: ShotSignals,
    require_strong: bool,
) -> Option<Suggestion> {
    let confidence = signals.suggestion(require_strong)?;
    Some(Suggestion {
        asset_id,
        kind: SuggestionKind::Screenshot,
        group: None,
        reason: ReasonCode::Screenshot,
        confidence,
        signals,
    })
}

/// Forslag uskarpt for ett bilde (uten gruppe).
pub fn blurry_for(asset_id: AssetId, score: f32, threshold: f32) -> Option<Suggestion> {
    let confidence = blurry_suggestion(score, threshold)?;
    Some(Suggestion {
        asset_id,
        kind: SuggestionKind::Blurry,
        group: None,
        reason: ReasonCode::Blurry,
        confidence,
        signals: ShotSignals::default(),
    })
}

/// Filtrerer bort nye forslag der brukeren allerede har avvist `(asset, kind)` (K5-8).
/// `rejected` er listen fra lagringen; selve `UNIQUE`-regelen håndheves også i `store` (K6).
pub fn without_rejected(
    new: Vec<Suggestion>,
    rejected: &[(AssetId, SuggestionKind)],
) -> Vec<Suggestion> {
    new.into_iter()
        .filter(|s| !rejected.contains(&(s.asset_id, s.kind)))
        .collect()
}
