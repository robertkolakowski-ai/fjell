# V1: Diagnose

**Status: under arbeid.** Kontrakten er på plass (se [ROBUSTHET-KONTRAKT.md](ROBUSTHET-KONTRAKT.md)); selve diagnosen er ikke bygget.

## Hva kontrakten allerede gir

- Kjerne: `FjellApi::diag_preview` og `diag_export` returnerer `DiagReport` (tall, koder og en redigert tekstblokk). Standard: tom rapport og `not_implemented`.
- Bridge: `previewDiagnostics()` og `exportDiagnostics()`. Mocken lager en troverdig rapport uten stier.
- UI: `SettingsDiagnostics.svelte` (plassholder, montert i Innstillinger).

## Regler (fra CLAUDE.md og docs/PERSONVERN.md)

- Bare tall og koder. Aldri bilder, stier, filnavn eller navn (personer, regler, mapper).
- Brukeren ser rapporten før noe eksporteres. Appen sender den aldri noe sted selv; ingen nye nettverkskall uten beslutning fra eier.

## Skal skrives

Hvilke målinger som tas med, hvordan teksten redigeres, hvor eksporten havner (åpent spørsmål i kontrakten) og hvordan det testes at ingenting kan lekke.
