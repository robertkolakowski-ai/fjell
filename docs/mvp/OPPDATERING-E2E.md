# Oppdatering via appen: e2e på Windows

Status: **skrevet, ikke kjørt.** Laget på Linux; YAML og TypeScript er validert, selve kjøringen på `windows-latest` er ikke prøvd. Regn med å justere velgere, tidsavbrudd og deteksjon etter første CI-kjøring.

## Hvorfor

Brukerfeil: når appen oppdaterer seg selv (Tauri updater starter NSIS-installeren med `/P /UPDATE` og kaller `exit(0)`), viser NSIS «Installasjonen er avbrutt – Kunne ikke avslutte Fjell». Isolert installer-oppgradering mens appen kjører (`desktop.yml`) er grønn, så feilen ligger i samspillet mellom appens egen avslutning og installeren. Denne testen kjører hele flyten slik brukeren gjør den.

## Hva testen gjør

`.github/workflows/update-e2e.yml` (`workflow_dispatch`, nattlig 03:43 UTC, og PR som berører testen, skallet eller oppdateringskoden):

1. Genererer en testnøkkel (`tauri signer generate --ci`). Nøkkelen finnes bare i runnerens temp og skrives aldri til logg.
2. Bygger 0.0.1 og 0.0.2 med `--config`-overstyring (pubkey = testnøkkelen, endpoint `http://127.0.0.1:18765/latest.json`, `dangerousInsecureTransportProtocol`, `createUpdaterArtifacts`). Versjon settes med `tools/set-version.mjs` i arbeidskopien, uten commit. Produksjonskonfig, CSP og capabilities er uendret.
3. Lager `latest.json` (`windows-x86_64`: url + signatur) og legger v2-`setup.exe` i serverrota.
4. Installerer v1 stille (`/S /D=...`, `currentUser`, uten heving).
5. `desktop/e2e-real/update.spec.ts` starter en lokal server (`scripts/update-e2e/serve.mjs`) og en overvåker (`scripts/update-e2e/watch.ps1`), starter v1 via CDP-harnessen (`launch.ts`), åpner Innstillinger, trykker «Se etter oppdatering» og «Installer og start på nytt».
6. Siden forsvinner når appen avslutter seg selv, så resten følges utenfra (timeout 180 s, `FJELL_UPDATE_TIMEOUT_MS`):
   - ingen installervindu med avbruddstekst (UI Automation: `avbrutt`, `aborted`, `kunne ikke avslutte`, ...),
   - installeren ble startet med `/P` og `/UPDATE`,
   - gammel prosess er borte og installeren henger ikke,
   - exe-versjon og avinstallasjonsnøkkel viser 0.0.2,
   - appen starter igjen (ny CDP-økt) og viser «Versjon 0.0.2».

Testen er **rød så lenge feilen finnes**. Det er meningen: den skal reprodusere feilen og bevise rettelsen.

## Logger ved feil (artefakt `update-e2e-logger`)

- `watch.log`: tidslinje over prosesser (start/slutt, kommandolinje, foreldre-pid) og tekst i installervinduer; `watch.log`-PNG-ene er skjermbilder når et nytt vindu dukket opp.
- `server.log`: hva appen hentet (latest.json, setup.exe).
- `watch-sammendrag.txt`, `relaunch.txt`, `feil-app.png`, Playwright-rapport.
- `Get-Process.txt`, `prosesser-kommandolinje.txt`, `eventlog-application.txt` (Restart Manager, Application Error/Hang), `eventlog-restartmanager.txt`, `installert-mappe.txt`, `registry-avinstaller.txt`, `temp-filer.txt`.

NSIS-pakken som Tauri bygger har ingen egen loggfil; vindustekst og prosesstidslinje er erstatningen.

## Kjente forbehold

- Selve `Kunne ikke avslutte`-dialogen er en NSIS-meldingsboks; om UIA ser den avhenger av at runneren har en interaktiv skrivebordsøkt (windows-latest har det). Henger installeren uten synlig dialog, fanges det av tidsavbruddet og prosesslisten.
- Runneren er administrator; `currentUser`-installasjon krever ikke heving, men brukerens miljø (antivirus, flere WebView2-prosesser, OneDrive-mappe) kan avvike.
- `tauri signer generate --ci` forutsettes å lage nøkkel med tom passfrase.
- Tauri ignorerer `APPDATA`-overstyring (se `launch.ts`); appdata er derfor runnerens ekte, som i `e2e-windows.yml`.
- Spec-filen ligger i `desktop/e2e-real` og matches også av `playwright.real.config.ts`, men hoppes over uten `FJELL_UPDATE_E2E=1`.

## Kjøre lokalt

Kun i CI (`CI=true`-vern). Se miljøvariablene i workflowen: `FJELL_EXE` (installert v1), `FJELL_UPDATE_WWW`, `FJELL_UPDATE_PORT`, `FJELL_UPDATE_FROM`, `FJELL_UPDATE_TO`, `FJELL_UPDATE_OUT`.
