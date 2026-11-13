# Ekte-app-e2e på Windows

Status: **skrevet, ikke kjørt.** Alt under er laget på Linux. Fixture-generatoren, TypeScript-typesjekken og YAML-parsingen er prøvd her; selve CDP-kjøringen mot `fjell-desktop.exe` er ikke prøvd før første kjøring på `windows-latest`. Regn med å justere velgere og tidsavbrudd etter første CI-kjøring.

## Hvordan det virker

Den vanlige e2e-en (`desktop/e2e`) kjører mot vite dev med MockBridge. Den sier ingenting om ekte Tauri-IPC, Rust-kjernen, miniatyrer eller Windows Papirkurv. Denne varianten starter den bygde appen og styrer WebView2 fra Playwright:

1. Appen startes med `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=9222`.
2. Playwright kobler til med `chromium.connectOverCDP('http://127.0.0.1:9222')` og bruker appvinduet som en vanlig `page`.
3. Hver kjøring har egen `APPDATA` og `LOCALAPPDATA` (temp), så ingenting lekker mellom kjøringer og førstegangs oppstart er ekte.
4. Fixture-biblioteket lages deterministisk (fast seed) i en temp-mappe.

Filer:

| Fil | Innhold |
|---|---|
| `desktop/scripts/real/fixtures.mjs` | Fixture-generator (jpeg-js + node:zlib, ingen native moduler) |
| `desktop/e2e-real/launch.ts` | Starter/avslutter appen, CDP-tilkobling |
| `desktop/e2e-real/ps.ts` | PowerShell: leser Windows Papirkurv (`Shell.Application`, Namespace 10) |
| `desktop/e2e-real/strings.ts` | Tekster appen viser (nb og en; språket følger Windows) |
| `desktop/e2e-real/real.spec.ts` | Scenariene |
| `desktop/e2e-real/playwright.real.config.ts` | Egen konfig, adskilt fra `desktop/playwright.config.ts` |
| `.github/workflows/e2e-windows.yml` | `workflow_dispatch` + nattlig (02:17 UTC), ikke på PR |

Fixture-biblioteket (18 filer): 3 eksakte duplikatpar (ulike mapper og navn), en serie på 4 like bilder, ett tydelig uskarpt bilde, ett skjermbilde-aktig PNG, 5 unike bilder og én korrupt JPEG. Alle filnavn har prefikset `fjelltest-`.

## Kjøre lokalt (Windows)

```powershell
cd desktop
npm ci
npx tauri build --no-bundle --config '{"bundle":{"createUpdaterArtifacts":false}}'
npx playwright test -c e2e-real/playwright.real.config.ts
```

- `FJELL_EXE` peker på en annen exe (standard: `src-tauri/target/release/fjell-desktop.exe`).
- `FJELL_CDP_PORT` bytter port (standard 9222). Lukk andre WebView2-apper som bruker samme port.
- `FJELL_REAL_OUT` er mappen for `real-perf.json` og feil-HTML (standard `e2e-real/.results`).
- Fixtures alene: `node scripts/real/fixtures.mjs C:\temp\fjell-fixtures` (fungerer også på Linux/macOS).
- **Vern:** testene kjører bare med `CI=true` eller `E2E_ALLOW_RECYCLE=1`. Ellers stopper `beforeAll` før noe røres.
- Tauri finner datamappen via `SHGetKnownFolderPath`, ikke `APPDATA`-miljøvariabelen, så isolasjonen over er ikke garantert. På CI slettes `%APPDATA%\no.fjell.desktop` før hver ren start. Lokalt nekter testen å slette en eksisterende ekte datamappe (flytt den selv først). WebView2-data isoleres med `WEBVIEW2_USER_DATA_FOLDER`.
- Papirkurven tømmes aldri helt. Bare elementer som heter `fjelltest-*` fjernes, før og etter kjøringen.

## Dekket

1. Førstegangs oppstart: onboarding vises, datamappen (`%APPDATA%\no.fjell.desktop`) opprettes (fanget en ekte feil tidligere).
2. Mappe legges til ved å skrive stien, skann ferdig, forventet antall duplikatgrupper.
3. Ekte miniatyrer: `img.naturalWidth > 0` i Gruppe.
4. Flytt gruppe, angre, flytt igjen. Filene sjekkes på disk (SHA-256 uendret etter angre).
5. Omstart: biblioteket og papirkurvtilstanden (antall i «Papirkurv (N)») bevares, onboarding vises ikke igjen.
6. «Tøm» til Windows Papirkurv: filene finnes i Papirkurven (PowerShell), alle andre filer har uendret SHA-256, nøyaktig ett medlem beholdt per gruppe.
7. Skymappe (sti med `OneDrive`): advarsel vises, papirkurv-handling er sperret, ingen filer endres.
8. Måling: kald oppstart til første synlige skjerm (første start og omstart) og handlings-p95 skrives til `real-perf.json`.

## Ikke dekket (ennå)

- Ytelsesporter: tallene er bare rapport. Kald start inkluderer CDP-tilkobling og er en øvre grense. Utvalgene er små, så p95 er grov.
- «Slett permanent», nettverksstasjon/UNC/lang sti (ikke-støttet Papirkurv), kortmodus, innstillinger, oppdatering.
- Visuelle og a11y-sjekker i ekte WebView2 (axe kjører bare i mock-e2e).
- Fixture-kvalitet for «like bilder», «uskarp» og «skjermbilde» er syntetisk. Testene hevder ikke at de klassifiseres, bare at skanning og eksakte duplikater fungerer.
- Skymappe-sperren testes ved å se at ingen filer endres, ikke ved en bestemt feilmelding.
- Bare `windows-latest` (engelsk visningsspråk): norske tekster dekkes av `strings.ts`, men ikke kjørt i nb.
- Ingen produktkode er endret. Eneste endring i `desktop/package.json` er devDependencies `jpeg-js` og `@types/node` (typer for testene). `nonet.test.ts` er uendret og grønn.

## Feilsøking

- «Fikk ikke CDP-forbindelse»: port i bruk eller WebView2 ignorerte argumentet. Sjekk at ingen `fjell-desktop.exe` ligger igjen.
- Test feiler: se `skjermbilde ved feil` og `feil-*.html` i artefakten `real-e2e-resultater`.
- Velger finner ikke tekst: sammenlign `strings.ts` med `desktop/src/lib/i18n/strings/`.
