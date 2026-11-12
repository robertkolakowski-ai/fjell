# Personvernerklæring for Fjell

Sist oppdatert: 2026-11-12 (versjon for Windows-beta). Dette er en åpen, teknisk erklæring: hvert punkt kan kontrolleres i kildekoden og i testene som er nevnt.

## Kort fortalt

- **Bildene dine forlater aldri maskinen.** Ingen opplasting, ingen sky, ingen konto.
- **Ingen telemetri.** Ingen bruksstatistikk, ingen feilrapporter, ingen annonse- eller sporings-ID-er.
- **Ingen nettverkskall fra Fjell.** Appen har ikke tillatelse til å snakke med internett. Dette er testet automatisk (se «Slik kan du kontrollere det»).
- **Originalene dine endres aldri.** Fjell flytter filer til en karantenemappe (som kan angres) og sletter ikke noe uten at du bekrefter.
- Alt Fjell lagrer, ligger lokalt og kan slettes av deg.

## Hva Fjell leser

Bare mappene du selv velger (i oppstarten eller i Innstillinger), inkludert eksterne disker og nettverksstasjoner du legger til. Fjell leser filnavn, størrelse, endringstid, EXIF-metadata (f.eks. opptakstidspunkt) og bildeinnholdet for å regne ut:
- en kontrollsum (BLAKE3) for å finne eksakte kopier,
- korte visuelle fingeravtrykk (64-bit hasher) for å finne like bilder,
- en uskarphetsscore og noen signaler for å gjenkjenne skjermbilder.

Dette skjer på maskinen din, med vanlig kode (ingen KI-modell lastes ned, ingen bilder sendes til en tjeneste). Skyfiler som bare er plassholdere (f.eks. OneDrive «bare på nett») lastes **ikke** ned.

**Skymapper (iCloud, OneDrive, Dropbox):** Fjell kjenner dem igjen på mappenavn og tillater skann og forslag, men flytter ikke filer til papirkurv der, så Fjell ikke utløser sletting i skyen. Fjell leser filene lokalt som i andre mapper; at synkprogrammet ditt laster dem opp eller ned er utenfor Fjell. Gjenkjenningen er en navneheuristikk, ikke bevis på synk, og er ikke testet mot ekte iCloud, OneDrive eller Dropbox.

## Hva Fjell lagrer lokalt

| Hva | Hvor | Innhold | Slette |
|---|---|---|---|
| Indeks | `%LOCALAPPDATA%\Fjell\index.db` (SQLite) | Stier, filstørrelse, endringstid, hasher, scorer, tilstand (aktiv / i papirkurv / tømt), handlingslogg for Angre, innstillinger (språk, papirkurvfrist) | Slett filen (appen bygger opp igjen ved neste skann), eller avinstaller og slett mappen |
| Bildecache | `%LOCALAPPDATA%\Fjell\cache\` | Nedskalerte visningsbilder (miniatyrer) brukt av skjermen. Standard maks 1 GB, eldste fjernes først | Slett mappen uten tap |
| Karantene | `<disk>\.fjell-karantene\<gruppe>\` på samme disk som bildet | Selve filene du har flyttet til «papirkurven» i Fjell (ikke kopier; samme innhold, bare flyttet), pluss `manifest.json` med opprinnelig sti, og en README som forklarer mappen | Gjenopprett i Fjell, eller tøm papirkurven i Fjell |
| Språkvalg i grensesnittet | Skallets innstillingsfil (`shell.json`) når appen kjører i Tauri; reserve og eldre verdi i WebView-lagring (`localStorage`, nøkkel `fjell.language`) | «system», «nb» eller «en» | Avinstaller og slett `%LOCALAPPDATA%\Fjell\` / WebView2-data. (Skallets lagring er testet på Linux; Tauri-kommandoene er ikke kjørt.) |

Det lagres **ikke**: bilder i indeksen (bare hasher og metadata), konto- eller enhets-ID-er, IP-adresser, posisjon, kontakter, passord eller innhold fra andre apper. Fjell skriver ikke loggfiler med filstier ut over det som står over.

## Tøm og sletting

- «Flytt til papirkurv» flytter filen til karantenemappen. Ingenting er slettet; Angre flytter tilbake.
- «Tøm papirkurv» sender som standard filene til **Windows Papirkurv**, der du kan gjenopprette dem.
- **Permanent sletting** krever et eget valg og en ekstra bekreftelse der du ser antallet. Ingenting slettes automatisk.
- Plass regnes som frigjort først ved permanent sletting eller når du tømmer Windows Papirkurv.

## Deling

Fjell deler ikke data med noen. Det finnes ingen server å dele med. Du kan selv kopiere eller sikkerhetskopiere indeksen; den inneholder filstier, så oppbevar den som du ville gjort med en mappeliste.

## Nettverk

- Fjell har ingen nettverkskode, ingen oppdateringsmekanisme og ingen innebygd nettleser mot eksterne sider. Innholdsregelen (CSP) i appvinduet tillater kun intern kommunikasjon mellom grensesnitt og Fjells egen kjerne (`ipc:`), ingen internettadresser, ingen eksterne fonter eller skript.
- **Installasjon:** installasjonsprogrammet kan hente Microsofts WebView2 hvis det mangler på maskinen. Det er installasjonsprogrammet (ikke Fjell) og kontakter bare Microsoft.
- **WebView2** (Microsofts nettleserkomponent) oppdateres av Windows/Microsoft på egen hånd og er ikke en del av Fjell. Fjell sender ikke noe gjennom den.
- **Nettverksstasjoner** (UNC): Fjell leser filer fra stasjonen du selv har valgt, via Windows. Det er ikke et kall til en tjeneste.
- Hvis du bruker Windows Sikkerhetskopiering eller OneDrive-synkronisering på mappene som inneholder `.fjell-karantene`, kan disse tjenestene kopiere karantenefilene som alle andre filer. Det styrer du i Windows.

## Slik kan du kontrollere det

- Kildekoden er åpen. Tredjepartskomponenter og lisenser: [THIRD_PARTY.md](../THIRD_PARTY.md).
- Automatiske tester som feiler hvis noen legger inn nettverkskall:
  - `desktop/src/lib/nonet.test.ts`: ingen `fetch`, `XMLHttpRequest`, `WebSocket`, `sendBeacon`, eksterne URL-er, CDN eller eksterne fonter i frontend-koden; CSP har `connect-src` bare for `ipc:` og `http://ipc.localhost`; Tauri-rettigheter uten http/shell/fil-plugin (eneste unntak: `dialog:allow-open`, Windows mappevalg for «Bla …», som bare gir tilbake en sti); ingen nettverksbiblioteker i Fjells egne Rust-crates.
  - `desktop/e2e/nonet.spec.ts`: kjører alle skjermer i en nettleser og feiler hvis siden gjør én eneste forespørsel utenfor appen.
- Selv: følg med i Windows Ressursovervåking (Nettverk) mens du skanner og rydder, eller blokker `Fjell.exe` i Windows-brannmuren. Appen fungerer likt.

## Barn, samtykke, rettigheter

Fjell samler ikke inn personopplysninger og har derfor ingen behandlingsgrunnlag å oppgi, ingen databehandlere og ingen kontaktskjema. Dataene på maskinen din er dine; GDPR-rettigheter (innsyn, retting, sletting) oppfylles ved at du selv kan se og slette alt som er listet over.

## Endringer

Endringer i hva Fjell lagrer eller sender krever oppdatering av denne filen og av testene over i samme endring. Fjell-teamet skal ikke legge til telemetri uten at dette dokumentet endres synlig i utgivelsesnotatet.
