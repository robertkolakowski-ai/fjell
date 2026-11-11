//! Modul `api` (L4). Se docs/mvp/ARKITEKTUR.md del 2.
//!
//! Avhenger av: pipeline, actions, store, model
//! Grensesnitt (hovedpunkter):
//! - Eneste offentlige flate og UniFFI-grensen: `Fjell::open`, `start_scan`, `cards`, `decide_many`,
//!   `set_keeper`, `begin_purge`/`confirm_purge`/`finish_purge`, `undo`, `home_summary`, `trash_summary`, `settings`.
//!
//! Skjelett (M0): ingen logikk ennå.
