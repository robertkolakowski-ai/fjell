//! Modul `people` (L2). V1-gren `people`: personer fra ansiktsindeks (av som standard), navngiving, sammenslåing og beskyttelse. Se docs/v1/V1-PLAN.md og docs/mvp/ARKITEKTUR.md del 2.
//!
//! Avhenger av: model, store.
//! Grensesnitt: (tomt i kontrakten) forventet: klynging av `Embedding` til `PersonId`, navn, sammenslåing (angrebar via `person.merged_into`).
//! Eier hele mappen `src/people/` og `store/people.rs`; ingen andre grener rører dem.

#[cfg(test)]
mod tests;
