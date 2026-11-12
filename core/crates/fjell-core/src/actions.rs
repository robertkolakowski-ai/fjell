//! Modul `actions` (L2). Se docs/mvp/ARKITEKTUR.md del 2.
//!
//! Avhenger av: store, model
//! Grensesnitt (hovedpunkter):
//! - `plan`, `commit`, `restore`, `keep`, `set_keeper`, `undo`, `expire_due`, `finish_purge` (PLAN-TRYGGHET).
//! - `Purge` krever typen `Confirmed`, som bare `api` kan lage.
//!
//! Skjelett (M0): ingen logikk ennå.
