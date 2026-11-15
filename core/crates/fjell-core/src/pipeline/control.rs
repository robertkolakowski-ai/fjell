//! Styring av indeksering (`pipeline`, L3). Skall i kontraktfasen: grenen som eier filen fyller den.
//!
//! Avhenger av: pipeline-moduler, model. Planlagt grensesnitt (docs/v1/ROBUSTHET-KONTRAKT.md): pause, fortsett og
//! avbryt for skanningen, `IndexingStatus` fra `Progress`, trådantall og prioritet fra `IndexingSettings`, og
//! pause på batteri via en strømkilde (`fjell-fs::power`, injisert; pipeline kjenner ingen plattform).
//! Gjenopptak skjer fordi arbeidssettet utledes fra databasen (se `pipeline`).
