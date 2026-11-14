# Oppdatering via appen: e2e på Windows

Status: **scenario 1 (oppdatering via appen) har kjørt grønt** på `windows-latest` (PR #29, kjøring 43525263587). Feilen fra brukerens maskin lot seg altså **ikke** reprodusere på en ren runner. Scenario 2 og 3 (hengt instans, M22) og NSIS-hookene er **skrevet, ikke kjørt**: laget på Linux uten `makensis`, så første kjøring i CI er også første kompilering av `installer-hooks.nsh`.

## Hvorfor

Brukerfeil: når appen oppdaterer seg selv (Tauri updater starter NSIS-installeren med `/P /UPDATE` og kaller `exit(0)`), viser NSIS «Installasjonen er avbrutt – Kunne ikke avslutte Fjell». Isolert installer-oppgradering mens appen kjører (`desktop.yml`) er grønn, og hele flyten via appen er også grønn i CI. Rotårsaken er derfor maskinspesifikk og **ukjent** (mulige årsaker: gjenværende `fjell-desktop.exe` eller WebView2-barn fra et tidligere forsøk, antivirus eller Utforsker som holder exe-filen, en instans startet som administrator). Denne testen kjører hele flyten slik brukeren gjør den, og verner mottiltaket (NSIS-hookene, se [OPPDATERING.md](../OPPDATERING.md), «Når appen ikke lar seg lukke»).

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

7. **Hengt instans, installasjon (M22):** appen startes og suspenderes (`scripts/update-e2e/suspend.ps1`, `NtSuspendProcess`): den svarer ikke, men holder exe-filen låst. Så kjøres v2-installeren med `/P /UPDATE`, slik oppdatereren gjør. Krav: ingen feilvindu, installeren avslutter med kode 0, den hengte prosessen er borte, og appen starter og viser 0.0.2.
8. **Hengt instans, avinstallering (M22):** samme oppsett, så `uninstall.exe /P`. Krav: ingen feilvindu, exe-filen og avinstallasjonsnøkkelen er borte, den hengte prosessen er borte. Dette er siste test (appen er avinstallert etterpå).

### Hva scenario 7 og 8 beviser, og ikke

**Vi har ingen realistisk reprodusent av brukerfeilen.** En suspendert prosess er det nærmeste vi kommer en instans som «ikke lar seg lukke» på en ren runner, men Restart Manager med tvang (`RmForceShutdown`, som malen bruker) klarer trolig å avslutte en suspendert prosess også uten hookene. Scenarioene er derfor et **vern**, ikke et bevis på at brukerfeilen er løst:

- hookene kompilerer og kjører i både installer og avinstaller,
- hookene dreper ikke installeren selv (i scenario 1 er installeren et barn av appen; derfor brukes ikke `taskkill /T`),
- en instans som ikke svarer, stopper verken installasjon eller avinstallasjon.

Tilfeller hookene **ikke** dekker, og som heller ikke er testet: Fjell kjører som administrator mens installeren kjører som vanlig bruker (da får verken `taskkill` eller Restart Manager lov til å avslutte den), og et annet program (antivirus, Utforsker) som holder `fjell-desktop.exe` åpen. Da gjelder reserveveien i appen og rådene i OPPDATERING.md.

`taskkill`-prosessen fra hooken logges i `watch.log` hvis overvåkeren (500 ms) rekker å se den; det er en indikasjon, ikke et krav i testen.

## Logger ved feil (artefakt `update-e2e-logger`)

- `watch.log`: tidslinje over prosesser (start/slutt, kommandolinje, foreldre-pid) og tekst i installervinduer; `watch.log`-PNG-ene er skjermbilder når et nytt vindu dukket opp.
- `server.log`: hva appen hentet (latest.json, setup.exe).
- `watch-sammendrag.txt`, `relaunch.txt`, `hengt-instans.txt` (scenario 7 og 8), `feil-app.png`, Playwright-rapport.
- `Get-Process.txt`, `prosesser-kommandolinje.txt`, `eventlog-application.txt` (Restart Manager, Application Error/Hang), `eventlog-restartmanager.txt`, `installert-mappe.txt`, `registry-avinstaller.txt`, `temp-filer.txt`.

NSIS-pakken som Tauri bygger har ingen egen loggfil; vindustekst og prosesstidslinje er erstatningen.

## Kjente forbehold

- Selve `Kunne ikke avslutte`-dialogen er en NSIS-meldingsboks; om UIA ser den avhenger av at runneren har en interaktiv skrivebordsøkt (windows-latest har det). Henger installeren uten synlig dialog, fanges det av tidsavbruddet og prosesslisten.
- Runneren er administrator; `currentUser`-installasjon krever ikke heving, men brukerens miljø (antivirus, flere WebView2-prosesser, OneDrive-mappe) kan avvike.
- `tauri signer generate --ci` lager nøkkel med tom passfrase (bekreftet i PR #29).
- CDP-porten: WebView2 ignorerer `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS` når wry setter egne argumenter. Workflowen setter derfor policy-nøkkelen `AdditionalBrowserArguments` i registeret (HKCU og HKLM, for `fjell-desktop.exe` og `*`), kun på runneren.
- Tauri ignorerer `APPDATA`-overstyring (se `launch.ts`); appdata er derfor runnerens ekte, som i `e2e-windows.yml`.
- Spec-filen ligger i `desktop/e2e-real` og matches også av `playwright.real.config.ts`, men hoppes over uten `FJELL_UPDATE_E2E=1`.

## Kjøre lokalt

Kun i CI (`CI=true`-vern). Se miljøvariablene i workflowen: `FJELL_EXE` (installert v1), `FJELL_UPDATE_WWW`, `FJELL_UPDATE_PORT`, `FJELL_UPDATE_FROM`, `FJELL_UPDATE_TO`, `FJELL_UPDATE_OUT`.
