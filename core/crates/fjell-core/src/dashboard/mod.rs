//! Modul `dashboard` (L2). V1-gren `dashboard`: lesemodell for dashboard (tall fra indeksen; ingen påstand om frigjort plass før purge). Se docs/v1/V1-PLAN.md og docs/mvp/ARKITEKTUR.md del 2.
//!
//! Avhenger av: model, store.
//! Grensesnitt: (tomt i kontrakten) forventet: bygge `DashboardData` av spørringene i `store/dashboard.rs`.
//! Eier hele mappen `src/dashboard/` og `store/dashboard.rs`; ingen andre grener rører dem.

#[cfg(test)]
mod tests;
