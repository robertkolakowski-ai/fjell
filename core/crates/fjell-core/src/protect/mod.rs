//! Modul `protect` (L2). V1-gren `protect`: beskyttelse per bilde, mappe, album og person (alltid ack før sletting). Se docs/v1/V1-PLAN.md og docs/mvp/ARKITEKTUR.md del 2.
//!
//! Avhenger av: model, store.
//! Grensesnitt: (tomt i kontrakten) forventet: sette/fjerne `ProtectScope`, beregne effektiv beskyttelse for `actions`-planen (`ConsequenceCode::ProtectedByScope`).
//! Eier hele mappen `src/protect/` og `store/protection.rs`; ingen andre grener rører dem.

#[cfg(test)]
mod tests;
