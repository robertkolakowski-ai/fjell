//! Modul `series` (L2). V1-gren `series`: serier (rekker av nesten like bilder tatt tett i tid) og valg av beste bilde. Se docs/v1/V1-PLAN.md og docs/mvp/ARKITEKTUR.md del 2.
//!
//! Avhenger av: model, store.
//! Grensesnitt: (tomt i kontrakten) forventet: ren gruppering av tidsnære visuelle treff til serier, rangering med `EyeReport`, `ReasonCode::SeriesMember`/`Keeper*`.
//! Eier hele mappen `src/series/` og `store/series.rs`; ingen andre grener rører dem.

#[cfg(test)]
mod tests;
