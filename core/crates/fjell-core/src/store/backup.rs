//! Sikkerhetskopi av indeksen (`store`, L1). Skall i kontraktfasen: grenen som eier filen fyller den.
//!
//! Avhenger av: model (ingen andre moduler). Planlagt grensesnitt (docs/v1/ROBUSTHET-KONTRAKT.md):
//! kopi av indeksfilen via SQLite online-backup til appens egen datamappe (aldri bildemapper), liste over
//! kopier, `integrity_check` (`IntegrityStatus`) og gjenoppretting som alltid tar en `PreRestore`-kopi først.
//! Kopien inneholder bare metadata; originalbilder røres aldri.
