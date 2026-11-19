# Windows-testmaskin og VPS: hva verifiseres hvor

Status: arbeidsdokument for eier. Fjell er en Windows-app, og nesten alt som er merket «ikke verifisert» i PR-ene gjelder Windows. CI på `windows-latest` dekker byggingen, enhetstester, røyktest av NSIS-installasjon og oppgradering mens appen kjører, og en Playwright-suite mot MockBridge. Det CI **ikke** kan dekke er en ekte, brukerstyrt maskin: Narrator/NVDA, skalering og høykontrast med ekte skjerm, antivirus, Utforsker som holder filer, og forhøyede rettigheter. Se også [BETA.md](BETA.md) (manuelle tester) og [OPPDATERING.md](OPPDATERING.md).

## 1. Hva en Linux-VPS (Ubuntu) kan og ikke kan

| Oppgave | Ubuntu-VPS | Windows-maskin |
|---|---|---|
| `cargo fmt/clippy/test`, `check-layers`, i18n, `cargo deny`, Vitest, Playwright mot MockBridge | ja | ikke nødvendig |
| `just bench` (ytelsesbudsjett i release) på stabil maskin | ja, for **Linux-tall** | trengs for Windows-tall |
| Modellmålinger (`tools/model-probe/`: tract, ONNX Runtime) | ja, for Linux-tall | trengs for Windows (WIC, DLL) |
| Nattlig kjøring av tunge Linux-jobber (golden-korpus, stort korpus) | ja | – |
| NSIS-installasjon, «Kunne ikke avslutte Fjell», selvoppdatering | **nei** | ja |
| Windows Papirkurv, `windows/`-koden, WIC, `rename`/fillås over åpne håndtak | **nei** | ja |
| Narrator, NVDA, skalering, høykontrast | **nei** | ja (med skjerm og lyd, evt. fjernskrivebord) |

En Linux-VPS gir altså ikke Windows-verifisering. Wine er ikke et alternativ for dette: det oppfører seg ikke som ekte Windows for Restart Manager, filtilgang eller hjelpemidler, så resultatene ville vært misvisende. Den er nyttig til det som alt kjører på Linux (stabile ytelsestall, tunge jobber), ikke til punktene som står åpne.

Skal en VPS brukes som CI-løper (self-hosted runner), må repoet være privat eller kjøringen begrenses til egne grener; ellers kan fremmede PR-er kjøre kode på maskinen. Ingenting av dette er satt opp.

## 2. Reprodusere «Kunne ikke avslutte Fjell» (Windows)

NSIS-hooken (`desktop/src-tauri/installer-hooks.nsh`) skriver nå `%TEMP%\fjell-installer.log` (ren diagnose; endrer ikke oppførselen). Linjene viser: fase (installasjon/avinstallasjon), om installeren kjører som administrator, antall `fjell-desktop.exe`-prosesser før og etter, `taskkill`-kode per forsøk og resultatet av WebView2-oppryddingen. Ingen stier, brukernavn eller filnavn loggføres, og fila forlater aldri maskinen. Den kuttes ved 100 KB.

Prøv hver rad, og noter resultatet og loggen etter hvert forsøk:

| # | Oppsett | Forventet | Hvis feil, se i loggen |
|---|---|---|---|
| W1 | Ren installasjon som vanlig bruker; start appen; installer ny versjon over fra appen («Installer og start på nytt») | Fullføres | `taskkill forsok`-linjer, `prosesser ... etter` skal være 0 |
| W2 | Som W1, men appen startet som administrator, installeren som vanlig bruker | Kan feile | `administrator = False` samtidig som appen er forhøyet: `taskkill` får «tilgang nektet» |
| W3 | Som W1, men installeren som administrator, appen som vanlig bruker | Fullføres eller feiler | `administrator = True`, `taskkill`-kode |
| W4 | Åpne en bildemappe i Utforsker, Appen kjører, installer over | Kan feile hvis Utforsker holder en fil | `prosesser ... etter` > 0 eller kode ≠ 0 |
| W5 | Antivirus aktivt (Defender) og tredjeparts AV hvis tilgjengelig; installer over | Kan feile | tidsavbrudd i `taskkill`/PowerShell |
| W6 | Avinstaller mens appen kjører | Fullføres | `fase: avinstallasjon` |
| W7 | Flere påloggede økter (hurtigbrukerbytte): Fjell kjører i en annen økt | Kan feile | `prosesser før` > 0, men `taskkill` finner ikke prosessen |

Rapporter: Windows-versjon (`winver`), AV-produkt, om kontoen er administrator, og hele `fjell-installer.log`. Hypotesene står i [OPPDATERING.md](OPPDATERING.md) («Når appen ikke lar seg lukke»). Meldingen «Kunne ikke avslutte Fjell» kommer fra Tauris NSIS-mal og er ikke endret.

## 3. Rutine for en Windows-maskin

1. Ren Windows 11 (og helst Windows 10) uten utviklerverktøy. Last ned siste `fjell-desktop-nsis-unsigned` fra CI-kjøringen eller beta-releasen. Installasjonsprogrammet er ikke kodesignert, så SmartScreen advarer.
2. Gå gjennom installasjonsmatrisen i avsnitt 2.
3. Backup og gjenoppretting (robusthet, PR #35): ta en sikkerhetskopi, gjenopprett, og kontroller at «Angre» virker. Test med full disk, og med et antivirus som skanner indeksfilen mens den kopieres. CI fant allerede en Windows-feil her (`sync_all` på en skrivebeskyttet fil), så flere kan finnes.
4. Styrt indeksering (PR #35): batteri-auto-pause på en bærbar (koble fra og til strøm), pause/fortsett/avbryt.
5. Narrator, NVDA, skalering 100–200 % og høykontrast: følg [BETA.md](BETA.md). De nye skjermene fra robusthetsfasen (Innstillinger → Sikkerhetskopi, Indeksering, Diagnose, trygg modus) er ikke testet manuelt ennå.
6. Ytelse: `just bench` er ikke kjørt på Windows. Bruk en maskin uten annen belastning og rapporter p95 mot budsjettene (handling < 100 ms, kjernekall < 10 ms, kald oppstart < 1,5 s).
7. Modellmåling (etter at modellen er koblet inn): kjør `tools/model-probe/` på Windows for tract-tider og, hvis aktuelt, DLL-lasting.

## 4. Hva dette dokumentet ikke gjør

- Det setter ikke opp noen server, runner eller konto, og det endrer ikke CI.
- Det påstår ikke at noe virker på Windows. Alt i avsnitt 3 er ikke kjørt.
- Det flytter ikke noe ut av maskinen: ingen telemetri, ingen opplasting av logger. Loggfila leveres manuelt av testeren.
