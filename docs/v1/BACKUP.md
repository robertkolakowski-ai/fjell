# V1: Sikkerhetskopi av indeksen og trygg modus

**Status: under arbeid.** Kontrakten er på plass (se [ROBUSTHET-KONTRAKT.md](ROBUSTHET-KONTRAKT.md)); kopiering og gjenoppretting er ikke bygget.

## Hva kontrakten allerede gir

- Kjerne: `backup_create`, `backup_list`, `backup_restore(RestoreAck)`, `backup_restore_undo`, `integrity_check` (`IntegrityStatus`). Standard: tom liste, `Ok` og `not_implemented`.
- Bridge: `listBackups`, `createBackup`, `restoreBackup(id, acknowledged)`, `undoRestoreBackup`, `checkIntegrity`. Mocken har kopier, avkrysning, angre og ødelagt indeks.
- UI: `SettingsBackup.svelte` (Innstillinger) og `RecoveryMode.svelte` (trygg modus, vist fra `App.svelte` når integriteten ikke er `ok`).

## Regler (fra CLAUDE.md og docs/TRYGGHET.md)

- En kopi er bare Fjells egen indeks (metadata), aldri bilder. Originalfiler røres ikke.
- Gjenoppretting krever `RestoreAck` (avkrysning) og tar alltid først en `pre_restore`-kopi, slik at den kan angres.
- Ingen kopi slettes stille. Ingen automatisk gjenoppretting.
- Trygg modus: ingen skanning og ingen endringer før brukeren har gjenopprettet en kopi eller sjekket på nytt.

## Skal skrives

Format og plassering av kopier, plan for automatiske kopier, hva «degraded» betyr, hva som skjer hvis indeksen ikke lar seg åpne ved oppstart (åpent spørsmål i kontrakten), og testene for avbrutt gjenoppretting.
