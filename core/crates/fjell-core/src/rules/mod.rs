//! Modul `rules` (L2). V1-gren `rules`: brukerdefinerte regler som foreslår (aldri sletter): evaluering, forhåndsvisning og treff. Se docs/v1/V1-PLAN.md og docs/mvp/ARKITEKTUR.md del 2.
//!
//! Avhenger av: model, store.
//! Grensesnitt: (tomt i kontrakten) forventet: koding av `RuleSpec` til `rule.spec`, evaluering mot indeksen, `ReasonCode::Rule`.
//! Eier hele mappen `src/rules/` og `store/rules.rs`; ingen andre grener rører dem.

#[cfg(test)]
mod tests;
