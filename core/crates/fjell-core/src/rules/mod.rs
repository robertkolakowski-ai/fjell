//! Modul `rules` (L2). V1-gren `rules`: brukerdefinerte regler som foreslår (aldri sletter): evaluering,
//! forhåndsvisning og treff. Se docs/v1/V1-PLAN.md og docs/mvp/ARKITEKTUR.md del 2.
//!
//! Avhenger av: model, store.
//! Grensesnitt:
//! - `eval::scan`: vilkårsevaluering i ett lesepass for alle regler.
//! - `spec::{encode, decode}`: stabil, versjonert tekst i `rule.spec` (`v1;screenshot;older=30;...`).
//! - `save`, `toggle`, `delete`, `restore`: endrer `rule`-tabellen og evaluerer på nytt etterpå.
//!   `delete` er soft-sletting (`deleted_at`) og angres med `restore`; ingenting fjernes hardt.
//! - `list`, `list_deleted`: lesemodell (`RuleItem`) for UI («Nylig slettet» for regler).
//! - `preview(store, &RuleSpec, now)`: antall, bytes og utvalg uten å lagre eller endre noe.
//! - `evaluate(store, now)`: kjører alle aktive regler mot indeksen, skriver `rule_hit` og legger til
//!   ventende forslag (`SuggestionKind::Rule`, `ReasonCode::Rule`, detalj `ReasonDetail::Rule` via `reason_detail`).
//! - `reason_detail(store, asset)`: regelen bak et forslag.
//!
//! Regler foreslår bare: ingenting her rører `asset.state` eller `action_log`, og et forslag blir aldri
//! `accepted` uten at brukeren velger det. Beskyttede bilder, av-slåtte og slettede regler gir aldri treff,
//! og en tom regel treffer ingenting. Kostnad: ett lesepass over indeksen uansett antall regler (mål: 10 regler,
//! 50 000 bilder, p95 under 50 ms; testet i `tests.rs`).
//!
//! Eier hele mappen `src/rules/` og `store/rules.rs`; ingen andre grener rører dem.

use crate::model::{AssetId, ReasonDetail, RuleCondition, RuleId, RuleSpec};
use crate::store::{Store, StoreError};

pub mod eval;
pub mod spec;

#[cfg(test)]
mod tests;

/// Høyeste antall bilder i `Preview::sample`.
pub const SAMPLE_LIMIT: usize = 24;
/// Lengste regelnavn (tegn).
pub const MAX_NAME_CHARS: usize = 80;

/// Feilkode fra regeloperasjoner (kjernen returnerer koder, aldri tekst).
#[derive(Debug)]
pub enum RuleError {
    /// Tom regel avvises: den ville ikke truffet noe.
    EmptySpec,
    EmptyName,
    NameTooLong,
    /// Vilkår uten innhold (tom mappe eller MIME-type).
    InvalidCondition,
    /// Regelen finnes ikke (eller er slettet og kan ikke endres).
    NotFound,
    Store(StoreError),
}

impl From<StoreError> for RuleError {
    fn from(e: StoreError) -> Self {
        RuleError::Store(e)
    }
}

impl std::fmt::Display for RuleError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RuleError::Store(e) => write!(f, "lagring: {e}"),
            other => write!(f, "{other:?}"),
        }
    }
}

impl std::error::Error for RuleError {}

/// Regel slik UI leser den.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RuleItem {
    pub id: RuleId,
    pub name: String,
    pub spec: RuleSpec,
    pub enabled: bool,
    /// Antall bilder regelen treffer nå (0 når den er av).
    pub hits: u64,
    /// Lagret spesifikasjon kunne ikke leses (ukjent versjon); regelen treffer da ingenting.
    pub corrupt: bool,
    /// Tidspunkt for soft-sletting; `None` for aktive regler.
    pub deleted_at: Option<i64>,
}

/// Forhåndsvisning av en regel.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct Preview {
    pub hits: u64,
    pub bytes: u64,
    /// Høyst 24 treff til miniatyrvisning.
    pub sample: Vec<AssetId>,
}

/// Resultat av en evaluering.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct EvalReport {
    pub rules_evaluated: u32,
    /// Regler som ble hoppet over fordi spesifikasjonen ikke kunne leses eller var tom.
    pub rules_skipped: u32,
    pub hits: u64,
    pub suggestions_added: u64,
    pub suggestions_removed: u64,
}

/// Sjekker en spesifikasjon: ikke tom, og ingen vilkår uten innhold.
pub fn validate(spec: &RuleSpec) -> Result<(), RuleError> {
    if spec.is_empty() {
        return Err(RuleError::EmptySpec);
    }
    let blank = |s: &str| s.trim().is_empty();
    for c in &spec.all_of {
        match c {
            RuleCondition::InFolder(s) | RuleCondition::Mime(s) if blank(s) => {
                return Err(RuleError::InvalidCondition)
            }
            _ => {}
        }
    }
    Ok(())
}

fn clean_name(name: &str) -> Result<String, RuleError> {
    let n = name.trim();
    if n.is_empty() {
        return Err(RuleError::EmptyName);
    }
    if n.chars().count() > MAX_NAME_CHARS {
        return Err(RuleError::NameTooLong);
    }
    Ok(n.to_string())
}

/// Oppretter (`id = None`) eller oppdaterer en regel, og evaluerer på nytt. Nye regler er aktive.
pub fn save(
    store: &mut Store,
    id: Option<RuleId>,
    name: &str,
    spec: &RuleSpec,
    now: i64,
) -> Result<RuleId, RuleError> {
    validate(spec)?;
    let name = clean_name(name)?;
    let text = spec::encode(spec);
    let id = match id {
        None => store.rule_insert(&name, &text, now)?,
        Some(id) => {
            if !store.rule_update(id, &name, &text)? {
                return Err(RuleError::NotFound);
            }
            id
        }
    };
    evaluate(store, now)?;
    Ok(id)
}

/// Slår en regel av eller på og evaluerer på nytt. Av-slåtte regler gir ingen treff og ingen nye forslag.
pub fn toggle(
    store: &mut Store,
    id: RuleId,
    enabled: bool,
    now: i64,
) -> Result<EvalReport, RuleError> {
    if !store.rule_set_enabled(id, enabled)? {
        return Err(RuleError::NotFound);
    }
    evaluate(store, now)
}

/// Soft-sletter en regel (angres med `restore`) og evaluerer på nytt. Allerede slettet er ikke en feil.
pub fn delete(store: &mut Store, id: RuleId, now: i64) -> Result<EvalReport, RuleError> {
    if !store.rule_set_deleted(id, Some(now))? {
        return Err(RuleError::NotFound);
    }
    evaluate(store, now)
}

/// Henter en slettet regel tilbake (med samme av/på-tilstand som før) og evaluerer på nytt.
pub fn restore(store: &mut Store, id: RuleId, now: i64) -> Result<EvalReport, RuleError> {
    if !store.rule_set_deleted(id, None)? {
        return Err(RuleError::NotFound);
    }
    evaluate(store, now)
}

fn items(store: &Store, deleted: bool) -> Result<Vec<RuleItem>, RuleError> {
    let counts = store.rule_hit_counts()?;
    let mut out = Vec::new();
    for row in store.rule_rows()? {
        if row.deleted_at.is_some() != deleted {
            continue;
        }
        let decoded = spec::decode(&row.spec);
        let hits = counts
            .iter()
            .find(|(id, _)| *id == row.id)
            .map_or(0, |(_, n)| *n);
        out.push(RuleItem {
            id: row.id,
            name: row.name,
            corrupt: decoded.is_err(),
            spec: decoded.unwrap_or_default(),
            enabled: row.enabled,
            hits: if row.enabled && row.deleted_at.is_none() {
                hits
            } else {
                0
            },
            deleted_at: row.deleted_at,
        });
    }
    Ok(out)
}

/// Aktive (ikke slettede) regler, eldste først.
pub fn list(store: &Store) -> Result<Vec<RuleItem>, RuleError> {
    items(store, false)
}

/// Slettede regler som kan hentes tilbake.
pub fn list_deleted(store: &Store) -> Result<Vec<RuleItem>, RuleError> {
    items(store, true)
}

/// Hva `spec` ville truffet nå. Lagrer og endrer ingenting; tom regel gir null treff.
pub fn preview(store: &Store, spec: &RuleSpec, now: i64) -> Result<Preview, RuleError> {
    let hits = eval::scan(store, &[spec], now)?.pop().unwrap_or_default();
    Ok(Preview {
        hits: hits.len() as u64,
        bytes: hits.iter().map(|h| h.bytes).sum(),
        sample: hits.iter().take(SAMPLE_LIMIT).map(|h| h.asset).collect(),
    })
}

/// Alle bilder `spec` treffer nå, stigende id.
pub fn match_ids(store: &Store, spec: &RuleSpec, now: i64) -> Result<Vec<AssetId>, RuleError> {
    let hits = eval::scan(store, &[spec], now)?.pop().unwrap_or_default();
    Ok(hits.into_iter().map(|h| h.asset).collect())
}

/// Kjører alle aktive regler mot indeksen og oppdaterer `rule_hit` og ventende `rule`-forslag.
/// Beskyttede bilder, av-slåtte og slettede regler gir ingen treff. Endrer aldri `asset` eller `action_log`.
pub fn evaluate(store: &mut Store, now: i64) -> Result<EvalReport, RuleError> {
    let mut report = EvalReport::default();
    let mut live: Vec<(RuleId, RuleSpec)> = Vec::new();
    for row in store.rule_rows()? {
        if !row.enabled || row.deleted_at.is_some() {
            continue;
        }
        match spec::decode(&row.spec) {
            Ok(s) if !s.is_empty() => live.push((row.id, s)),
            _ => report.rules_skipped += 1,
        }
    }
    let specs: Vec<&RuleSpec> = live.iter().map(|(_, s)| s).collect();
    let found = eval::scan(store, &specs, now)?;
    report.rules_evaluated = live.len() as u32;
    let hits: Vec<(RuleId, Vec<AssetId>)> = live
        .iter()
        .zip(found)
        .map(|((id, _), h)| (*id, h.into_iter().map(|h| h.asset).collect()))
        .collect();
    let stats = store.rule_apply(&hits, now)?;
    report.hits = stats.hits;
    report.suggestions_added = stats.suggestions_added;
    report.suggestions_removed = stats.suggestions_removed;
    Ok(report)
}

/// Detaljen bak begrunnelsesmerket «regel»: regelen med lavest id som treffer bildet.
pub fn reason_detail(store: &Store, asset: AssetId) -> Result<ReasonDetail, RuleError> {
    Ok(store
        .rule_hit_for_asset(asset)?
        .map_or(ReasonDetail::None, ReasonDetail::Rule))
}
