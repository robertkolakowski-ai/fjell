//! Modul `dashboard` (L2). V1-gren `dashboard`: lesemodell for dashboard (tall fra indeksen; ingen påstand om frigjort plass før purge). Se docs/v1/V1-PLAN.md og docs/mvp/ARKITEKTUR.md del 2.
//!
//! Avhenger av: model, store.
//! Grensesnitt:
//! - `read(&Store) -> Result<Dashboard, StoreError>`: alt i ett kall (p95 < 100 ms på 50 000 bilder).
//! - `read_summary(&Store)`: som `read`, men uten fordeling per måned og mappe (billigere).
//! - `DashboardCache`: gjenbruker svaret mens indeksen er uendret (`Store::dash_stamp`); gjenåpning < 20 ms p95.
//! - `data(&Store) -> Result<DashboardData, StoreError>`: bare den delte lesemodellen (kontraktstypen).
//! - `Dashboard { data, insights }`; `Insights`: «kan ryddes» uten beskyttede bilder, papirkurv, permanent
//!   frigjort, `Progress`, `NextStep` og `FreedMonth` (frigjort per måned).
//! - `freed_since(&Store, ts)`: permanent frigjort fra og med tidspunkt (historikklinjen).
//!
//! Eier hele mappen `src/dashboard/` og `store/dashboard.rs`; ingen andre grener rører dem.
//!
//! Regler (CLAUDE.md): modulen leser bare, endrer aldri indeksen, kjenner ingen vei til sletting og har ingen
//! nettverk. Den returnerer koder og tall, aldri tekst; UI oversetter. Appen foreslår: `NextStep` er et forslag
//! med tall bak, aldri en handling som utføres.
//!
//! - **Kan ryddes** (`per_kind`, `Insights::reclaimable_*`): ventende forslag på aktive bilder, uten beskyttede
//!   bilder. Hvert bilde teller under én type, så summen over typer er lik totalen.
//! - **Papirkurv** (`trashed_*`): bilder i «Nylig slettet». Plassen er ikke frigjort før «Tøm».
//! - **Frigjort** (`freed_bytes`, `FreedMonth`): `purge`-rader i `action_log`, aldri `cause = 'external'`
//!   (bilder som forsvant utenfor Fjell). Kjernen påstår aldri frigjort plass før en purge er loggført.
//!   Åpent (ARKITEKTUR #22): `action_log` har ennå ikke kolonne for purge-modus (Papirkurv/permanent). Til
//!   `purge`-rader bærer modus, teller en loggført purge som frigjort; meldes til Integrer, ikke løst her
//!   fordi migreringen er delt.

use crate::model::{DashboardData, FolderStat, KindStat, MonthStat, SuggestionKind};
use crate::store::{Store, StoreError};

#[cfg(test)]
mod tests;

/// Antall mapper i `DashboardData::top_folders`.
pub const TOP_FOLDERS: usize = 10;

/// Fremdrift i gjennomgang av forslag: avgjorte (`accepted`/`rejected`) av alle forslag.
/// `permille` er 0..=1000; uten forslag er den 0 (ingen kunstig seier).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Progress {
    pub decided: u64,
    pub total: u64,
    pub permille: u16,
}

/// Neste beste steg som kode med tall. Et forslag til brukeren; kjernen gjør ingenting.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NextStep {
    /// Gå gjennom forslagstypen som gir mest plass (byte, så antall, så rekkefølgen i `SuggestionKind::ALL`).
    ReviewKind {
        kind: SuggestionKind,
        count: u64,
        bytes: u64,
    },
    /// Ingen forslag igjen, men papirkurven har innhold brukeren kan tømme (med bekreftelse).
    EmptyTrash { count: u64, bytes: u64 },
    /// Ingenting å foreslå nå.
    Nothing,
}

/// Permanent frigjort i en måned (`year_month` = `ÅÅÅÅMM`, UTC). Aldri `cause = 'external'`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FreedMonth {
    pub year_month: u32,
    pub count: u64,
    pub bytes: u64,
}

/// Tall utover kontraktstypen `DashboardData`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Insights {
    /// Antall bilder med ventende forslag, uten beskyttede.
    pub reclaimable_count: u64,
    /// Byte som kan ryddes (summen av `DashboardData::per_kind`).
    pub reclaimable_bytes: u64,
    /// Aktive bilder som er beskyttet (flagg, mappe, album, person, enkeltbilde).
    pub protected_count: u64,
    pub progress: Progress,
    pub next_step: NextStep,
    /// Frigjort per måned, eldst først. Tom til første permanente sletting.
    pub freed_by_month: Vec<FreedMonth>,
}

/// Hele dashboardet fra ett lesetidspunkt.
#[derive(Clone, Debug, PartialEq)]
pub struct Dashboard {
    pub data: DashboardData,
    pub insights: Insights,
}

/// Leser hele dashboardet. Bare `SELECT`; endrer ingenting.
pub fn read(store: &Store) -> Result<Dashboard, StoreError> {
    assemble(store, true)
}

/// Som `read`, men uten fordelingen per måned og mappe (`by_month` og `top_folders` er tomme). Raskt nok til å
/// vises før resten er klart: p95 < 20 ms på 50 000 bilder (se `tests.rs`).
pub fn read_summary(store: &Store) -> Result<Dashboard, StoreError> {
    assemble(store, false)
}

fn assemble(store: &Store, with_distribution: bool) -> Result<Dashboard, StoreError> {
    let prot = store.dash_protected_ids()?;
    let totals = store.dash_totals(&prot)?;
    let (_, freed_bytes) = store.dash_freed_total()?;
    let (series_count, people_count, rules_active) = store.dash_counts()?;

    let reclaim = store.dash_reclaimable(&prot)?;
    let per_kind: Vec<KindStat> = SuggestionKind::ALL
        .iter()
        .map(|&kind| {
            let (count, bytes) = reclaim
                .iter()
                .find(|t| t.0 == kind)
                .map_or((0, 0), |t| (t.1, t.2));
            KindStat { kind, count, bytes }
        })
        .collect();

    let dist = if with_distribution {
        store.dash_distribution(TOP_FOLDERS)?
    } else {
        Default::default()
    };
    let by_month = dist
        .by_month
        .into_iter()
        .map(|(year_month, count, bytes)| MonthStat {
            year_month,
            count,
            bytes,
        })
        .collect();
    let top_folders = dist
        .top_folders
        .into_iter()
        .map(|(path, count, bytes)| FolderStat { path, count, bytes })
        .collect();

    let reclaimable_count = per_kind.iter().map(|k| k.count).sum();
    let reclaimable_bytes = per_kind.iter().map(|k| k.bytes).sum();
    let (decided, total) = store.dash_progress()?;
    let progress = Progress {
        decided,
        total,
        permille: (decided.min(total) * 1000).checked_div(total).unwrap_or(0) as u16,
    };
    let freed_by_month = store
        .dash_freed_by_month()?
        .into_iter()
        .map(|(year_month, count, bytes)| FreedMonth {
            year_month,
            count,
            bytes,
        })
        .collect();

    let next_step = next_step(&per_kind, totals.trashed_count, totals.trashed_bytes);
    Ok(Dashboard {
        data: DashboardData {
            active_count: totals.used_count,
            active_bytes: totals.used_bytes,
            trashed_count: totals.trashed_count,
            trashed_bytes: totals.trashed_bytes,
            freed_bytes,
            per_kind,
            by_month,
            top_folders,
            protected_count: totals.protected,
            series_count,
            people_count,
            rules_active,
        },
        insights: Insights {
            reclaimable_count,
            reclaimable_bytes,
            protected_count: totals.protected,
            progress,
            next_step,
            freed_by_month,
        },
    })
}

/// Gjenbruker siste svar så lenge indeksen er uendret (`Store::dash_stamp`). Å åpne dashboardet igjen uten
/// endringer koster et enkelt pragma-oppslag; etter en endring regnes alt ut på nytt (aldri utdaterte tall).
#[derive(Default)]
pub struct DashboardCache {
    /// (stempel, svar, med fordeling).
    slot: Option<((u64, i64), Dashboard, bool)>,
}

impl DashboardCache {
    pub fn new() -> DashboardCache {
        DashboardCache::default()
    }

    /// Som `read`, men gjenbruker svaret hvis indeksen er uendret.
    pub fn read(&mut self, store: &Store) -> Result<Dashboard, StoreError> {
        self.get(store, true)
    }

    /// Som `read_summary`, men gjenbruker svaret hvis indeksen er uendret (et svar med fordeling duger også).
    pub fn read_summary(&mut self, store: &Store) -> Result<Dashboard, StoreError> {
        self.get(store, false)
    }

    /// Glem svaret (neste lesing regner ut på nytt).
    pub fn clear(&mut self) {
        self.slot = None;
    }

    fn get(&mut self, store: &Store, need_distribution: bool) -> Result<Dashboard, StoreError> {
        // Stempelet leses før utregningen: skriver noen midt i, er stempelet allerede utdatert ved neste lesing.
        let stamp = store.dash_stamp()?;
        if let Some((s, d, full)) = &self.slot {
            if *s == stamp && (*full || !need_distribution) {
                return Ok(d.clone());
            }
        }
        let d = assemble(store, need_distribution)?;
        self.slot = Some((stamp, d.clone(), need_distribution));
        Ok(d)
    }
}

/// Bare den delte lesemodellen (`FjellApi::dashboard`).
pub fn data(store: &Store) -> Result<DashboardData, StoreError> {
    Ok(read(store)?.data)
}

/// (antall, byte) permanent frigjort fra og med `since` (Unix-sekunder). Aldri `cause = 'external'`.
pub fn freed_since(store: &Store, since: i64) -> Result<(u64, u64), StoreError> {
    store.dash_freed_since(since)
}

/// Velger neste steg. Rent og uten I/O. Lik størrelse avgjøres av antall, så av rekkefølgen i `ALL`.
fn next_step(per_kind: &[KindStat], trashed_count: u64, trashed_bytes: u64) -> NextStep {
    let best = per_kind
        .iter()
        .filter(|k| k.count > 0)
        .enumerate()
        .max_by_key(|(i, k)| (k.bytes, k.count, std::cmp::Reverse(*i)))
        .map(|(_, k)| *k);
    match best {
        Some(k) => NextStep::ReviewKind {
            kind: k.kind,
            count: k.count,
            bytes: k.bytes,
        },
        None if trashed_count > 0 => NextStep::EmptyTrash {
            count: trashed_count,
            bytes: trashed_bytes,
        },
        None => NextStep::Nothing,
    }
}
