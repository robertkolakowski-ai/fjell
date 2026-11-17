# Robusthet: kontrakt (backup, indeksering, diagnose)

Kontraktfasen låser grensesnittet og registrerer tomme filer, slik at tre grener kan bygge parallelt uten å røre delte filer.
Hver gren (`backup`, `indexing`, `diag`) eier sine egne filer (del 3) og endrer ikke noe i del 2 uten en ny kontraktrunde.
Rekkefølgen i `docs/VEIKART.md` gjelder fortsatt; dette er ikke nye funksjoner utover det som står her.

Status: kontrakt, standardimplementasjoner, mock og plassholderskjermer er på plass. **Ingen ekte backup, indeksstyring eller diagnose er bygget.**
Kjørt på Linux (se del 5). Ingenting er verifisert på Windows.

## 1. Regler som gjelder alle grener

- CLAUDE.md «Ufravikelig». Spesielt: originalbilder endres aldri; en kopi er bare Fjells egen indeks (metadata); gjenoppretting er angrebar og går bare med avkrysning (`RestoreAck`); ingenting forlater maskinen; ingen telemetri; ingen nye nettverkskall eller Tauri-capabilities uten beslutning fra eier.
- Kjernen sender koder og tall, aldri tekst. Eneste unntak er `DiagReport::text`, en teknisk blokk med `kode=verdi`-linjer (ingen stier, filnavn eller navn). All brukertekst bor i `i18n/nb.json` og `en.json` (nøkkelnavnerom `diag.*`, `backup.*` (også trygg modus: `backup.recovery.*`), `indexing.*`, `privacy.*`, `error.<kode>`).
- Feil er stabile snake_case-koder i `ApiError::Internal(kode)` (`ROBUST_ERROR_CODES`): `backup_not_found`, `backup_ack_required`, `backup_corrupt`, `backup_failed`, `index_busy`, `indexing_not_running`, `indexing_invalid_settings`, `diag_failed`, `not_implemented`. Skallet sender dem som `{ code: "core_error", detail: <kode> }`; `normalizeError` i `tauri-bridge.ts` løfter dem til `code`.
- Standardimplementasjonene i `FjellApi` er den eneste oppførselen `MemApi` og `Fjell` har i dag: tom liste, ledig status, `IntegrityStatus::Ok`, tom diagnose, ellers `not_implemented`. Når en gren overstyrer en metode i `Fjell`, oppdaterer den samme gren mocken og konformitetstesten om semantikken avviker.

## 2. Grensesnitt (låst)

### Kjerne (`fjell-core`)

Typer i `model::robust` (L0): `BackupId`, `BackupKind` (`manual`, `auto`, `pre_restore`), `IntegrityStatus` (`ok`, `degraded`, `corrupt`, `unreadable`), `RestoreAck` (kan bare lages med avkrysning: `RestoreAck::new(id, checked) -> Option<_>`), `IndexState` (`idle`, `running`, `paused`, `paused_on_battery`), `IndexPriority` (`low`, `normal`), `DiagMetric`, `DiagReport` (`new`, `empty`, `render`), `ROBUST_ERROR_CODES`.
Typer i `api::types_robust` (L4): `BackupEntry`, `RestoreReport`, `IndexingStatus`, `IndexingSettings` (`threads` 0 = automatisk, høyst `MAX_INDEX_THREADS` = 64; `priority`; `pause_on_battery`), alle med standardverdier.

| Metode på `FjellApi` | Standard | Merknad |
| --- | --- | --- |
| `backup_create() -> BackupEntry` | `not_implemented` | Tar `manual`-kopi. `backup_failed` om indeksen ikke er hel |
| `backup_list() -> Vec<BackupEntry>` | tom | Nyeste først |
| `backup_restore(RestoreAck) -> RestoreReport` | `not_implemented` | Tar først en `pre_restore`-kopi (`safety`). `backup_not_found`, `backup_corrupt`, `index_busy` |
| `backup_restore_undo(safety: BackupId) -> RestoreReport` | `not_implemented` | Setter `safety` tilbake. Ingen avkrysning (det er selve angre). `backup_not_found` hvis id ikke er en `pre_restore`-kopi |
| `integrity_check() -> IntegrityStatus` | `Ok` | Alt annet enn `Ok` gir trygg modus i UI |
| `indexing_status() -> IndexingStatus` | ledig | `state`, `phase` (som `ScanPhase`), `done`, `total`, `rate_per_sec`, `eta_secs` |
| `indexing_pause`, `indexing_resume`, `indexing_cancel` | `not_implemented` | `indexing_not_running` når det ikke er noe å styre |
| `indexing_settings()`, `indexing_settings_set(..)` | standard (0, `low`, pause på batteri på) / `not_implemented` | `indexing_invalid_settings` |
| `diag_preview() -> DiagReport` | tom rapport | Samme innhold som eksport |
| `diag_export() -> DiagReport` | `not_implemented` | Se åpent spørsmål 1 |

Plattformnøytral strømstatus: `fjell-fs::power` (`PowerStatus`: `ac`, `battery`, `unknown`; `PowerSource`; `FakePower`). `Unknown` regnes som nettstrøm (pauser aldri). Kjernen får svaret som kode; `pipeline` kjenner ingen plattform (injiseres via skallet).

### Skall (`fjell-shell`) og Tauri

DTO-er i `dto_robust.rs` (camelCase, koder): `BackupDto`, `RestoreReportDto`, `IndexingStatusDto`, `IndexingSettingsDto` (begge retninger), `DiagDto`/`DiagMetricDto`.
Metoder på `Shell` (i `backup.rs`, `indexing.rs`, `diag.rs`) og Tauri-kommandoer (alle registrert i `desktop/src-tauri/src/lib.rs`, ingen nye capabilities):

| `Shell` | Tauri-kommando (argumenter) | `CoreBridge` (TS) |
| --- | --- | --- |
| `backup_create` | `backup_create` | `createBackup()` |
| `backups` | `backup_list` | `listBackups()` |
| `backup_restore(id, acknowledged)` | `backup_restore` (`id`, `acknowledged`) | `restoreBackup(id, acknowledged)` |
| `backup_restore_undo(safety)` | `backup_restore_undo` (`safety`) | `undoRestoreBackup(safety)` |
| `integrity` | `integrity_check` | `checkIntegrity()` |
| `indexing_status` | `indexing_status` | `getIndexingStatus()` |
| `indexing_pause` / `_resume` / `_cancel` | `indexing_pause` / `indexing_resume` / `indexing_cancel` | `pauseIndexing()` / `resumeIndexing()` / `cancelIndexing()` |
| `indexing_settings` / `set_indexing_settings` | `get_indexing_settings` / `set_indexing_settings` (`settings`) | `getIndexingSettings()` / `setIndexingSettings(s)` |
| `diag_preview` / `diag_export` | `diag_preview` / `diag_export` | `previewDiagnostics()` / `exportDiagnostics()` |

`backup_restore` uten `acknowledged = true` svarer `backup_ack_required` og kaller ikke kjernen. `integrity` uten bibliotek svarer `ok`.
Backup og integritetssjekk kjører i `spawn_blocking`. Statiske tester (`shell/tests/v1_kommandolag.rs`) sjekker at hver Tauri-kommando er registrert, er et tynt omslag uten fil/nett, og at `TauriBridge` bare kaller registrerte kommandoer.

### Frontend

- Typer: `desktop/src/bridge/types-robust.ts`. Feilkodene ligger også i `BridgeErrorCode` (`types.ts`).
- Mock: `desktop/src/bridge/mock-robust.ts` (kopier med avkrysning, `pre_restore` og angre; ødelagt kopi kan ikke gjenopprettes; indeksering følger mockens skann, pauses og fortsetter, og står stille på batteri; diagnose uten stier). Dev-parametre: `?mockIntegrity=corrupt|degraded|unreadable`, `?mockBattery=1`.
- Feilkode til tekst: `screens/robustErrors.ts` (`error.<kode>`, ukjent gir `err.generic`).
- Flyt og tilstand per gren: `backupFlow.ts`, `indexingFlow.ts`, `diagFlow.ts`; tester i `screens/robust.test.ts` og `bridge/robust.test.ts`.

## 3. Eierskap per fil

Filer under «Delte» eies av kontraktfasen og er ferdige: grenene endrer dem ikke. Trenger en gren en endring der, tas det en ny kontraktrunde (eller, hvis det er en ren feil, meldes det).

| Gren | Eier (kan fylle og endre fritt) |
| --- | --- |
| `backup` | `core/crates/fjell-core/src/store/backup.rs`, `core/crates/fjell-core/src/api/fjell_backup.rs` (felles med `indexing` og `diag` for hver sin del, se under), `desktop/src-tauri/shell/src/backup.rs`, `desktop/src-tauri/shell/tests/backup.rs`, `desktop/src/screens/SettingsBackup.svelte`, `RecoveryMode.svelte`, `backupFlow.ts`, `docs/v1/BACKUP.md` |
| `indexing` | `core/crates/fjell-core/src/pipeline/control.rs`, `core/crates/fjell-fs/src/windows/power.rs` (og trait-tester i `power.rs`), `desktop/src-tauri/shell/src/indexing.rs`, `desktop/src-tauri/shell/tests/indexing.rs`, `desktop/src/screens/IndexingStatus.svelte`, `SettingsIndexing.svelte`, `indexingFlow.ts`, `docs/v1/INDEKSERING.md` |
| `diag` | `desktop/src-tauri/shell/src/diag.rs`, `desktop/src-tauri/shell/tests/diag.rs`, `desktop/src/screens/SettingsDiagnostics.svelte`, `diagFlow.ts`, `docs/v1/DIAGNOSE.md`, `docs/juridisk/README.md` (og personverntekst i `docs/PERSONVERN.md`) |

`api/fjell_backup.rs` er én fil i kjernen. Hver gren legger sine `impl`-blokker i egne fil-deler (f.eks. `impl Fjell { pub(super) fn backup_*(..) }`, `indexing_*`, `diag_*`); koblingen fra `impl FjellApi for Fjell` (i `fjell_ops.rs`, delt) gjøres av grenen som først trenger den og er en ren delegering på én linje per metode. Konflikter her løses ved merge, ikke ved å flytte kode.

Delte (eies av kontrakten):

- Kjerne: `model/robust.rs`, `model/mod.rs`, `api/types_robust.rs`, `api/mod.rs` (trait og standardimplementasjoner), `api/mock.rs`, `core/crates/fjell-fs/src/power.rs`, `lib.rs`-registreringene.
- Skall: `shell/src/dto_robust.rs`, `shell/src/lib.rs`, `desktop/src-tauri/src/lib.rs` (kommandoene), `desktop/src-tauri/capabilities/` (uendret).
- Frontend: `bridge/{types-robust,mock-robust,core-bridge,tauri-bridge,mock-bridge,bridge,types}.ts`, `screens/{Settings,Home}.svelte`, `App.svelte`, `screens/robustErrors.ts`.
- i18n: `i18n/nb.json` og `i18n/en.json` er delt. Grenene legger nye nøkler under sitt eget navnerom (`backup.*`, `indexing.*`, `diag.*`/`privacy.*`) og kjører `python3 tools/gen-i18n/gen_i18n.py`. De genererte filene i `desktop/src/lib/i18n/strings/` regenereres ved merge (konflikter løses ved å kjøre generatoren på nytt, aldri for hånd).
- Docs: `docs/v1/ROBUSTHET-KONTRAKT.md`, `README.md`-indeksen.

Modulgrafen i `docs/mvp/ARKITEKTUR.md` og `tools/check-layers/layers.toml` er uendret: det er nye undermoduler (`store::backup`, `pipeline::control`, `model::robust`, `api::types_robust`, `api::fjell_backup`), ikke nye moduler eller lag. `backup` (L1, `store`) bruker bare `model`; `control` (L3, `pipeline`) bruker `store`, `model` og de andre `pipeline`-delene.

## 4. Åpne spørsmål (til eier)

1. **Hvor havner diagnoseeksporten?** Frontend har ikke filsystem (CSP, capabilities) og `dialog:allow-save` er ikke gitt. Alternativer: (a) bare vise teksten i appen så brukeren kopierer selv (plassholderen gjør dette), (b) utklippstavle fra frontend, (c) en Rust-kommando som åpner en lagre-dialog og skriver filen (krever beslutning om dialog-tillatelse), (d) skrive i appens datamappe og vise stien. Ingen av dem sender noe over nett. `diag_export` er derfor `not_implemented` til dette er avgjort.
2. **Korrupt indeks ved oppstart.** Hvis `Store::open` feiler, finnes ingen `FjellApi` å spørre. Skal skallet svare `unreadable` fra `integrity` og åpne trygg modus uten kjerne (gjenoppretting må da skje uten åpen indeks)? Kontrakten antar at `backup_restore` kan kalles når indeksen er `corrupt`, men sier ikke hvordan det skjer når den ikke lar seg åpne.
3. **Hva er `degraded`?** Forslag: `integrity_check` ok, men `foreign_key_check` eller avstemming gir avvik. Skal `degraded` gi hele trygg modus eller bare et banner? Dagens `App.svelte` sperrer for alt annet enn `ok`. Konsekvens: uten brukbar kopi er «Bygg på nytt» eneste vei videre, og den nullstiller «Nylig slettet» og angrehistorikk. Ikke avklart av eier; tatt inn som vurderingspunkt i `BETA.md` 8b.
4. **Automatiske kopier.** Plan (ukentlig? ved oppstart? før migrering?), antall som beholdes, og hvem som rydder. «Ingen stille tømming» tilsier at utrenskning av gamle kopier må være synlig og angrebar eller konfigurerbar.
5. **Plassering og størrelse.** Bare appens datamappe, aldri bildemapper? Skal kopiene krypteres (de inneholder stier og regelnavn)?
6. **Batteripause og «Fortsett».** Skal «Fortsett» overstyre batteripausen for denne økten, eller bare slå av innstillingen? `indexing_resume` i tilstanden `paused_on_battery` er udefinert i kontrakten.
7. **Prioritet og tråder utenfor Windows.** Trådprioritet er bare definert for Windows (`windows/power.rs` dekker strøm; prioritet trenger egen plattformkode). Skal `priority` ignoreres på Linux/macOS (bonusbygg)?
8. **FFI.** De 13 nye `FjellApi`-metodene står i `NOT_MIRRORED_API` i `fjell-ffi` (paritetstesten krever det) med begrunnelsen «bare desktop i MVP». Skal de speiles når mobil kommer, eller holdes utenfor for godt? Ingen mobilkode bygges nå.
9. **Ytelse.** `integrity_check` på store indekser (50 000+ rader) må måles mot kjernebudsjettet (< 10 ms). Sannsynlig svar: kjør den i bakgrunnen og cache resultatet; `integrity_check` i kontrakten er derfor kalt ved oppstart og på «Sjekk på nytt» i UI, ikke ved hvert kall.
10. **Narrator/NVDA.** Ny manuell test trengs for trygg modus (varsel `role="alert"` ved oppstart), bekreftelsesdialogen for gjenoppretting og statuslinjen for indeksering (`docs/BETA.md`). Ikke gjort.

## 5. Hva som er kjørt

| Del | Status |
| --- | --- |
| Rust (hele `core/`-workspace): fmt, clippy `-D warnings`, test, `core/testdata`, lagregler med selvtest | Kjørt og grønt, kun Linux |
| Skall (`fjell-shell`) inkludert statiske kommandotester | Kjørt og grønt, kun Linux |
| `fjell-desktop` (Tauri-kommandoene i `src-tauri/src/lib.rs`) | **Ikke bygget** (krever WebView2/webkit2gtk). Kommandoene følger eksisterende mønster og er kontrollert statisk av skalltestene |
| `fjell-fs::windows::power` | **Ikke verifisert** (cfg(windows), ikke kompilert her) |
| i18n (paritet, lint, selvtester), vitest (461), svelte-check, contrast, build | Kjørt og grønt |
| Playwright mot MockBridge (220 grønne, 23 hoppet over som før; ny `e2e/robusthet.spec.ts` med axe, pseudo og trygg modus) | Kjørt og grønt, Chromium på Linux |
| `cargo deny`, `just bench` | Ikke kjørt (ingen nye avhengigheter; ingen varm sti endret). `gen_third_party.py --check` er grønn |
| Narrator, NVDA, skalering, høykontrast på de nye skjermene | **Ikke testet** |
