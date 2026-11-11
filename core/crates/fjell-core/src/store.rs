//! Modul `store` (L1). Se docs/mvp/ARKITEKTUR.md del 2.
//!
//! Avhenger av: model
//! Grensesnitt (hovedpunkter):
//! - SQLite: skjema, migrering, spørringer, transaksjoner.
//! - `Store::open`, `upsert_assets`, `put_fingerprints`, `set_state(batch)`, `next_cards`, `log_batch`, `pop_batch`.
//!
//! Skjelett (M0): ingen logikk ennå.
