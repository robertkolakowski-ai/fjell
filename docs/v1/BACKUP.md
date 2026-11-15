# Backup, integritetssjekk og gjenoppretting av indeksen

Grenen `backup` (robusthet, se `docs/v1/ROBUSTHET-KONTRAKT.md`). Denne siden beskriver hva som er bygget, hvordan det oppfører seg ved feil, og hva som gjenstår å koble inn.
Status per del står til slutt (kjørt og grønt, kun Linux, ikke verifisert). Ingenting her er verifisert på Windows.

## Hva en kopi er

- En kopi er **bare Fjells indeks** (`index-<hash>.sqlite`), aldri bildefiler. Hver kopi er merket `content=index_only` i `.meta`.
- Kopien kan inneholde **mappestier, albumnavn og personnavn** (det indeksen inneholder). Den ligger lokalt i appens datamappe og forlater aldri maskinen. Appen sender ingenting noe sted. Kopiene er ikke kryptert (åpent spørsmål 5 i kontrakten); vil du ikke ha navn liggende, slett kopiene selv eller bruk diskkryptering.
- Plassering: `<app-data>/backups/<indeksfilnavn>/backup-NNNNNN.db` og `backup-NNNNNN.meta`. Aldri i en bildemappe.
- `.meta` er linjer på formen `nøkkel=verdi`: `format`, `id`, `kind` (`manual`, `auto`, `pre_restore`), `created_at` (Unix-sekunder), `bytes`, `assets`, `schema` (`user_version`), `app_version`, `checksum` (blake3 av `.db`), `reason` (`manual`, `weekly`, `before_migration`, `before_purge`, `pre_restore`) og `content=index_only`.
- En kopi finnes først når `.meta` ligger der. Meta skrives sist og atomisk (`.tmp` så `rename`), så en avbrutt kopi er aldri synlig. Halvferdige `.tmp`-filer ryddes ved neste kopi. Id-er gjenbrukes ikke.

## Hvordan kopien tas

- `VACUUM INTO` via en **egen skrivebeskyttet tilkobling** (`Store::backup_create_at`). Det gir en konsistent kopi av en åpen WAL-database uten ren filkopi, og tar ingen lås på skriveren. `Fjell` holder ikke indekslåsen mens kopien lages; en test (`backup_does_not_block_actions`) sjekker at handlinger ikke venter. Skallet skal likevel kjøre kallet i `spawn_blocking` (kontrakten), slik at det aldri tar UI-tråden.
- Kopien åpnes og kontrolleres (`integrity_check`, `foreign_key_check`, skjema) **før** den får `.meta`. Er kilden ødelagt, feiler kopien med `backup_failed` (årsak `source_corrupt`), og ingen eldre kopi rulles bort.
- WAL-modus fjernes i kopien, så den er én fil uten sidefiler og har stabil sjekksum.

### Rullering og tidspunkter

- Bare `auto`-kopier rulleres: de siste N beholdes (setting `backup_keep_auto`, 1 til 50, standard 5). `manual` og `pre_restore` fjernes aldri av appen. Hvilke `auto`-kopier som ble rullet bort, rapporteres av `backup_create_at` (`BackupCreated::pruned`); ingen stille tømming.
- `Fjell::backup_auto_if_due("weekly")`: tar en `auto`-kopi hvis ingen kopi (annet enn `pre_restore`) er yngre enn 7 døgn. `Fjell::backup_auto_if_due("before_purge")`: samme med 1 time, til bruk før en stor «Tøm». Begge gir `None` når ingenting trengs. Skallet har `Shell::backup_auto(reason)` for dette.
- Før migrering: `Store::backup_before_migration_at` (og `Shell::prepare_index`) tar en `auto`-kopi (`before_migration`) hvis indeksfilen har eldre skjema enn kjernen. Den åpner ikke indeksen som `Store`, så ingenting migreres av kallet.
- Manuelt: «Ta kopi nå» (`backup_create`, `BackupKind::Manual`).

## Integritetssjekk

`Store::check_file(sti, full)` gir `IndexCheck`, som `IndexCheck::status()` avbilder på `IntegrityStatus`. Den bruker egen skrivebeskyttet tilkobling, så den blokkerer ikke og blokkeres ikke av skriveren.

| `IndexCheck` | kode | `IntegrityStatus` | betydning |
| --- | --- | --- | --- |
| `Ok` | `ok` | `ok` | hel |
| `SchemaOld(v)` | `schema_old` | `ok` | eldre skjema, migreres ved åpning |
| `SchemaTooNew(v)` | `schema_too_new` | `unreadable` | laget av en nyere versjon |
| `Degraded` | `degraded` | `degraded` | leses, men `foreign_key_check` har avvik (bare full sjekk) |
| `Corrupt` | `corrupt` | `corrupt` | ikke SQLite, avkortet, ødelagt side, eller tom fil |
| `Unreadable` | `unreadable` | `unreadable` | mangler, låst eller ingen tilgang |

- `full = false` er `quick_check` (oppstart). `full = true` er `integrity_check` og `foreign_key_check` (`FjellApi::integrity_check`, «Sjekk indeksen»).
- En **tom fil regnes som ødelagt**: en avkortet indeks skal ikke stilltiende bli en ny, tom en.
- Svar fra kjernen er koder, aldri tekst. Brukerteksten bor i `i18n` (`backup.integrity.<kode>`).

## Gjenoppretting (angrebar og aldri destruktiv)

`FjellApi::backup_restore(RestoreAck)` krever `RestoreAck`, som bare kan lages med avkrysning. Skallet avviser `acknowledged = false` med `backup_ack_required` før noe kalles. I kjernen (`Store::restore_backup`, under skrivelåsen):

1. Kopien verifiseres helt: `.meta`, størrelse, sjekksum, integritet, skjema. Ødelagt gir `backup_corrupt`, ukjent id gir `backup_not_found`. Ingenting er endret.
2. **`PreRestore`-kopi** av dagens indeks (kan angres senere). Feiler den, er ingenting endret.
3. Kopien kopieres til `<indeks>.restoring` ved siden av indeksen (samme volum), sjekksum og integritet kontrolleres på nytt.
4. `wal_checkpoint(TRUNCATE)`, tilkoblingen lukkes. Den gamle indeksen flyttes til `<indeks>.before-restore` (aldri slettet før den nye er åpnet), sidefiler (`-wal`, `-shm`) fjernes, den nye flyttes på plass (`rename`).
5. Den nye åpnes (migreres). Går det bra, ryddes `.before-restore`; kopien fra før finnes i kopilisten. Går det galt, rulles det tilbake: den gamle settes på plass og åpnes igjen.

`backup_restore_undo(safety)` setter en `pre_restore`-kopi tilbake. Det er samme prosedyre, og den tar selv en ny `pre_restore`-kopi, så angre kan angres. Andre kopier enn `pre_restore` gir `backup_not_found`. Mens en skanning pågår, gir begge `index_busy`.

Skallet holder `flow`-låsen rundt kjerne-kallet (ingen sletting, angre eller «Tøm» midt i byttet), og avstemmer karantenen etterpå (`Core::reconcile`, beslutning 20): filer i karantenen som ikke lenger er «slettet» i den gjenopprettede indeksen legges tilbake. Ingen fil slettes.

### Krasj midt i gjenopprettingen

`Store::recover_interrupted_restore` kjøres **før indeksen åpnes** ved oppstart (`Shell::prepare_index`):

| tilstand ved krasj | oppførsel |
| --- | --- |
| etter kopi (`.restoring` finnes), indeksen urørt | `.restoring` slettes, den gamle brukes |
| indeksen mangler, `.before-restore` finnes | den gamle settes tilbake (`rolled_back`) |
| ny indeks på plass, `.before-restore` finnes, ny er hel | `.before-restore` ryddes (`cleaned_up`) |
| ny indeks på plass, men ikke hel | den ødelagte flyttes til `<indeks>.corrupt-<tid>`, den gamle settes tilbake |

Hvert trinn er testet med både **feil** (rulles tilbake, indeksen urørt) og **simulert krasj** (filene blir liggende, så rettes de) i `store/backup/tests.rs`.

## Korrupt indeks ved oppstart (trygg modus)

Når `Store::open` feiler, finnes ingen `FjellApi`. Skallet løser det uten åpen kjerne:

- `Shell::integrity()` sjekker indeksfilen til sist brukte bibliotek direkte (`quick_check`) og svarer `corrupt` eller `unreadable`. Uten noe bibliotek (første oppstart) er svaret `ok`. UI går da i trygg modus (`RecoveryMode`).
- `Shell::backups()` leser kopilisten direkte fra kopimappen.
- **Valg 1, gjenopprett fra siste gode kopi:** `Shell::backup_restore(id, true)` uten åpen kjerne bruker `Store::restore_closed_at`: kopien verifiseres, den ødelagte indeksen flyttes til `<indeks>.corrupt-<tid>` (slettes aldri), kopien settes på plass og åpnes, og biblioteket åpnes igjen (karantenen avstemmes som ved all åpning). Det finnes ingen `pre_restore`-kopi av en indeks som ikke kunne åpnes, så svaret har `safety = 0` og UI viser ikke «Angre», men sier at den forrige indeksen er lagt til side. `Store::latest_good_backup_at` finner siste kopi som består full kontroll.
- **Valg 2, bygg på nytt:** `Shell::rebuild_index()` flytter den ødelagte indeksen til siden, åpner en ny og starter full skanning. Originaler og karantene røres ikke. Karantenen avstemmes ved åpningen (beslutning 20): filer som lå i karantenen, men ikke er «slettet» i den nye indeksen, legges tilbake på opprinnelig sted (testet: begge duplikatene finnes igjen). **«Nylig slettet» og angrehistorikken fra den gamle indeksen er borte**, og det skal brukeren alltid få vite (`RebuildDto::trash_state_reset`). Kallet avvises (`bad_request`) når indeksen er hel, så en frisk indeks aldri kastes ved en feil.

## Feil og ingen delvis skriving

Feil er `ApiError::Internal(kode)` med kodene fra kontrakten (`backup_not_found`, `backup_corrupt`, `backup_failed`, `index_busy`). På lavere nivå har `BackupError::cause()` en finere årsak til diagnose: `disk_full`, `write_failed`, `locked`, `source_corrupt`, `no_file`, `io`.

- Alt skrives til `.tmp` og flyttes med `rename`. Feiler noe, slettes `.tmp`, og ingen `.meta` finnes. Testet: `backups` er en fil (kan ikke lages), `.tmp`-rester, ødelagt kilde.
- Full disk (`StorageFull`, `ENOSPC`, Windows 39 og 112 og SQLite `SQLITE_FULL`), låst fil (Windows 32 og 33, `ResourceBusy`, `SQLITE_BUSY`/`LOCKED`) og skrivefeil avbildes av `cause_of_io` og `cause_of_sqlite` (enhetstestet med syntetiske feil; ekte full disk og ekte fillås er **ikke** testet).
- Utad får UI `backup_failed` for alle disse (kontraktens kodeliste er låst). Forslag til egne koder (`backup_disk_full`, `backup_locked`) i `integration_notes`.

## Ny PC

Indeksen husker bilder med stien de har (`source_id`). En kopi flyttet til en ny PC virker derfor bare når mappene finnes på **samme sti**. Hvis ikke finner skanningen ikke bildene, og indeksen avstemmer dem som borte fra kilden (`purged`, `cause=external`; ingenting slettes av Fjell). Ledetekst i UI (`backup.newpc`): «velg mappene på nytt, og Fjell skanner dem igjen». Å velge mappene på nytt starter en ny indeks for den nye stien. Det finnes ingen automatisk omskriving av stier, og den er ikke planlagt i denne fasen.

## Hvem kaller hva

| Flate | Kode |
| --- | --- |
| Kjerne | `store/backup.rs` (`impl Store`, filbasert), `api/fjell_backup.rs` (`impl Fjell`, delegert fra `fjell_ops.rs`) |
| Skall | `shell/src/backup.rs`: `backup_create`, `backups`, `backup_restore`, `backup_restore_undo`, `integrity`, `backup_auto`, `prepare_index`, `rebuild_index` |
| Frontend | `SettingsBackup.svelte` (liste, «Ta kopi nå», gjenoppretting med avkrysning, angre bare når en kopi av «før» finnes, personvern- og flyttenotat) |

## Koblet inn (integrasjon)

- `prepare_index` kalles i `open_roots_probe` før `Core::open_roots` (krasjgjenoppretting og migreringskopi).
- `rebuild_index`: Tauri-kommando `backup_rebuild_index`, `CoreBridge.rebuildIndex` og «Bygg på nytt» i `RecoveryMode.svelte`, med avkrysning om at «Nylig slettet» og angrehistorikk nullstilles.
- `backup_auto("weekly")` kjøres i en bakgrunnstråd ved oppstart (`setup` i `fjell-desktop`, ikke bygget her: GTK mangler). `backup_auto("before_purge")` kalles i `Shell::empty` før «Tøm».
- Ikke gjort: UI for antall automatiske kopier (setting `backup_keep_auto`); egne feilkoder for full disk og låst fil.

## Status

| Del | Status |
| --- | --- |
| Kjerne: 23 tester i `store::backup` og 5 i `api::fjell_backup`, clippy `-D warnings` | Kjørt og grønt, kun Linux |
| Skall: `shell/tests/backup.rs` (8 tester, ekte kjerne mot temp-mapper) | Kjørt og grønt, kun Linux |
| Frontend: vitest, svelte-check, kontrast, build, Playwright (`e2e/backup.spec.ts`, `e2e/robusthet.spec.ts`) | Kjørt og grønt, Chromium på Linux |
| Windows: filbytte med `rename` over åpne håndtak (antivirus, indeksering), fillåser, full disk | **Ikke verifisert** |
| Ytelse (`just bench`, kopitid for 50 000+ rader) | **Ikke målt** |
| Narrator og NVDA: bekreftelsesdialogen, trygg modus-varsel, nytt notat | **Ikke testet** (`docs/BETA.md`) |
