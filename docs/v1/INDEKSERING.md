# V1: Indeksering (kontroll og innstillinger)

**Status: under arbeid.** Kontrakten er på plass (se [ROBUSTHET-KONTRAKT.md](ROBUSTHET-KONTRAKT.md)); styringen er ikke bygget.

## Hva kontrakten allerede gir

- Kjerne: `indexing_status` (`IndexingStatus`), `indexing_pause`, `indexing_resume`, `indexing_cancel`, `indexing_settings` og `indexing_settings_set` (`IndexingSettings`: tråder, prioritet, pause på batteri). Standard: ledig status, standardinnstillinger og `not_implemented`.
- `fjell-fs::power`: plattformnøytral `PowerSource` med `FakePower`; Windows-skallet ligger i `windows/power.rs` (svarer `Unknown` til det er fylt ut, og pauser da aldri).
- Bridge: `getIndexingStatus`, `pauseIndexing`, `resumeIndexing`, `cancelIndexing`, `getIndexingSettings`, `setIndexingSettings`.
- UI: `IndexingStatus.svelte` (montert på Hjem) og `SettingsIndexing.svelte` (Innstillinger).

## Regler

- Kjernen kjenner ingen plattform: strømstatus og trådprioritet injiseres fra skallet.
- Indekseringen skriver aldri til originalbilder. Pause og avbryt beholder det som er funnet; gjenopptak utledes fra databasen.
- Ytelse: handlinger < 100 ms og kjernekall < 10 ms (`just bench`).

## Skal skrives

Hvordan pause/fortsett/avbryt kobles til `pipeline`, tråd- og prioritetsstyring, batteripause (og om «Fortsett» overstyrer den), og hva som verifiseres bare på Windows.
