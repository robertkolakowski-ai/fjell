//! Robusthetsflaten for `Fjell` (L4): sikkerhetskopi av indeksen, integritet, indeksering og diagnose.
//! Del av `api`. Skall i kontraktfasen: `Fjell` bruker standardimplementasjonene i `FjellApi`
//! (`not_implemented`/tom verdi) til grenen som eier denne filen kobler den til `store::backup`
//! og `pipeline::control`. Se docs/v1/ROBUSTHET-KONTRAKT.md (eierskap og regler).
//!
//! Avhenger av: `super::fjell` (`Fjell`), store, pipeline, model. Grensesnitt: `pub(super)` metoder på `Fjell`
//! som `impl FjellApi for Fjell` (i `fjell_ops`) delegerer til når grenene kobler dem inn.
//!
//! Regler: gjenoppretting går bare med `RestoreAck` og tar alltid en `PreRestore`-kopi først (angrebar);
//! ingen kall her skriver til originalbilder; feil er koder (`ROBUST_ERROR_CODES`), aldri tekst.
