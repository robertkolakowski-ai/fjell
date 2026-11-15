# Personvernerklæring for Fjell

Sist oppdatert: 2026-11-15 (V1: lagt til ansiktsindeks, serier, regler og beskyttelse). Dette er en åpen, teknisk erklæring: hvert punkt kan kontrolleres i kildekoden og i testene som er nevnt.

## Kort fortalt

- **Bildene dine forlater aldri maskinen.** Ingen opplasting, ingen sky, ingen konto.
- **Ingen telemetri.** Ingen bruksstatistikk, ingen feilrapporter, ingen annonse- eller sporings-ID-er.
- **Ingen nettverkskall fra Fjell, bortsett fra oppdateringssjekken i beta-kanalen** (bare når du ber om den, eller har slått på «Sjekk automatisk ved oppstart», standard av; se «Oppdateringssjekk»). Bildene, stiene og filnavnene dine sendes aldri. Dette er testet automatisk (se «Slik kan du kontrollere det»).
- **Originalene dine endres aldri.** Fjell flytter filer til en karantenemappe (som kan angres) og sletter ikke noe uten at du bekrefter.
- Alt Fjell lagrer, ligger lokalt og kan slettes av deg.
- **Ansiktsindeks er av som standard.** Fjell analyserer ingen ansikter før du selv slår det på og bekrefter. Se [Ansiktsindeks og personer](#ansiktsindeks-og-personer-valgfritt-av-som-standard).
- **Unntak: oppdateringssjekk i beta-kanalen.** Bare når du trykker «Se etter oppdatering», eller hvis du selv har slått på «Sjekk automatisk ved oppstart» (standard: av), henter Fjell en liten tekstfil fra GitHub. Se [Oppdateringssjekk](#oppdateringssjekk-beta-kanalen).

## Hva Fjell leser

Bare mappene du selv velger (i oppstarten eller i Innstillinger), inkludert eksterne disker og nettverksstasjoner du legger til. Fjell leser filnavn, størrelse, endringstid, EXIF-metadata (f.eks. opptakstidspunkt) og bildeinnholdet for å regne ut: Mappevalget («Bla …») er Windows' egen mappedialog (`dialog:allow-open`, ingen nettverk): den gir bare tilbake stien du velger, og ingenting leses før du bekrefter.
- en kontrollsum (BLAKE3) for å finne eksakte kopier,
- korte visuelle fingeravtrykk (64-bit hasher) for å finne like bilder,
- en uskarphetsscore og noen signaler for å gjenkjenne skjermbilder.

Dette skjer på maskinen din, med vanlig kode (ingen KI-modell lastes ned, ingen bilder sendes til en tjeneste). Ansikter analyseres bare hvis du har slått på ansiktsindeksen (neste avsnitt). Skyfiler som bare er plassholdere (f.eks. OneDrive «bare på nett») lastes **ikke** ned.

**Skymapper (iCloud, OneDrive, Dropbox):** Fjell kjenner dem igjen på mappenavn og tillater skann og forslag, men flytter ikke filer til papirkurv der, så Fjell ikke utløser sletting i skyen. Fjell leser filene lokalt som i andre mapper; at synkprogrammet ditt laster dem opp eller ned er utenfor Fjell. Gjenkjenningen er en navneheuristikk, ikke bevis på synk, og er ikke testet mot ekte iCloud, OneDrive eller Dropbox.

## Hva Fjell lagrer lokalt

| Hva | Hvor | Innhold | Slette |
|---|---|---|---|
| Indeks | `%LOCALAPPDATA%\Fjell\index.db` (SQLite) | Stier, filstørrelse, endringstid, hasher, scorer, tilstand (aktiv / i papirkurv / tømt), handlingslogg for Angre, innstillinger (språk, papirkurvfrist, sensitivitet, ansiktsindeks av/på), beskyttelser, forslagsregler, serier og, **bare hvis du har slått på ansiktsindeksen**, personer og ansikts-embeddings | Slett filen (appen bygger opp igjen ved neste skann), eller avinstaller og slett mappen |
| Bildecache | `%LOCALAPPDATA%\Fjell\cache\` | Nedskalerte visningsbilder (miniatyrer) brukt av skjermen. Standard maks 1 GB, eldste fjernes først | Slett mappen uten tap |
| Karantene | `<disk>\.fjell-karantene\<gruppe>\` på samme disk som bildet | Selve filene du har flyttet til «papirkurven» i Fjell (ikke kopier; samme innhold, bare flyttet), pluss `manifest.json` med opprinnelig sti, og en README som forklarer mappen | Gjenopprett i Fjell, eller tøm papirkurven i Fjell |
| Språkvalg i grensesnittet | Skallets innstillingsfil (`shell.json`) når appen kjører i Tauri; reserve og eldre verdi i WebView-lagring (`localStorage`, nøkkel `fjell.language`) | «system», «nb» eller «en» | Avinstaller og slett `%LOCALAPPDATA%\Fjell\` / WebView2-data. (Skallets lagring er testet på Linux; Tauri-kommandoene er ikke kjørt.) |

Det lagres **ikke**: bilder i indeksen (bare hasher og metadata; heller ingen ansiktsbilder), konto- eller enhets-ID-er, IP-adresser, posisjon, kontakter, passord eller innhold fra andre apper. Fjell skriver ikke loggfiler med filstier ut over det som står over.

## Ansiktsindeks og personer (valgfritt, av som standard)

V1 kan gruppere bilder etter hvem som er på dem («Personer»). Dette er det mest personfølsomme Fjell gjør, så det er styrt av deg.

- **Av som standard.** Innstillingen `face_index_enabled` er `0` i nye og oppgraderte indekser. Så lenge den er av, kjøres ingen ansiktsanalyse og ingenting om ansikter lagres. Dekket av testene i `api/fjell_v1_tests.rs` og `store/migrate_tests.rs` (standard `0`).
- **Samtykke.** Du slår den på i Innstillinger («Ansikter og personer») og må bekrefte en dialog som forklarer hva som lagres, at det ikke forlater enheten, at originalene ikke endres, og at du kan slå av og slette.
- **Hva som lagres (bare når den er på):** for hvert ansikt et sett tall (embedding, et matematisk fingeravtrykk), hvilket bilde det kom fra, og navn du selv gir en person. **Ingen ansiktsbilder lagres**, og originalfilene endres ikke. Dette ligger i samme lokale indeksfil (`index.db`) som resten, i tabellene `person` og `face_embedding`.
- **Kun på enheten.** Embeddings og navn forlater aldri maskinen. Ingen opplasting, ingen server, ingen sky. Det finnes ingen eksport av indeksen i V1; skulle en komme, skal navn og embeddings utelates som standard.
- **Modell:** analysen krever en lokal ansiktsmodell som følger med installasjonsprogrammet; appen laster aldri ned modeller. **Per 2026-11-15 er ingen ansiktsmodell valgt eller inkludert**, så Personer er tom også med indeksen på til en modell er på plass. Dette dokumentet må leses på nytt når en modell kommer inn (lisens, hva modellen lagrer).
- **Slå av:** å slå indeksen av stopper ny analyse. Eksisterende ansiktsdata blir liggende til du sletter dem.
- **Slett ansiktsindeksen («wipe»):** knappen i Innstillinger fjerner alle embeddings og alle personer, inkludert navnene du har gitt. **Dette kan ikke angres**; det er et bevisst unntak fra «alt kan angres», og dialogen sier det. Begrunnelsen er personvern: dataene er avledet og kan bygges opp igjen ved å slå indeksen på igjen, men navn og beskyttelse av personer faller bort. Bildene beholdes. Bytes som slettes overskrives (`secure_delete`) og loggfilen (WAL) tømmes. Beskyttelse av en person gjøres om til beskyttelse av hvert enkelt bilde i samme operasjon, så ingen bilder mister beskyttelsen.
- **Ikke krypteringsbeskyttet:** indeksfilen er ikke kryptert i V1. Den som har tilgang til brukerprofilen din kan lese navn og embeddings. Beskyttelsen er Windows-kontoen og diskkryptering (f.eks. BitLocker), ikke Fjell.
- **Øyne i serier** (lukkede øyne) er et eget signal fra en eventuell øyemodell og lagrer ingen ansiktsindeks eller person. Heller ingen øyemodell er valgt per 2026-11-15; serier rangeres da på skarphet alene.

Også nytt i V1, uten nye personopplysninger: **beskyttelser** (stier, mappe, album eller person du har merket), **forslagsregler** (navn og vilkår du selv skriver) og **dashboard** (tall regnet fra indeksen). Alt ligger lokalt i `index.db` og forlater ikke maskinen. Navn på regler og personer er fritekst du selv har skrevet.

## Tøm og sletting

- «Flytt til papirkurv» flytter filen til karantenemappen. Ingenting er slettet; Angre flytter tilbake.
- «Tøm papirkurv» sender som standard filene til **Windows Papirkurv**, der du kan gjenopprette dem.
- **Permanent sletting** krever et eget valg og en ekstra bekreftelse der du ser antallet. Ingenting slettes automatisk.
- Plass regnes som frigjort først ved permanent sletting eller når du tømmer Windows Papirkurv.

## Oppdateringssjekk (beta-kanalen)

Beta-versjonene av Fjell for Windows kan oppdatere seg selv. Dette er det eneste Fjell gjør på nettet, og det er styrt av deg.

- **Når:** bare når du trykker «Se etter oppdatering» i Innstillinger, eller ved oppstart hvis du har slått på «Sjekk automatisk ved oppstart». Den innstillingen er **av som standard**. Er den av og du aldri trykker, sender Fjell ingenting.
- **Hva som hentes:** filen `latest.json` fra `https://github.com/robertkolakowski-ai/fjell/releases/download/beta/latest.json` (versjonsnummer, endringsnotat, nedlastingsadresse og signatur). Repoet er offentlig, så det kreves ingen innlogging og ingen konto.
- **Hva som sendes:** en vanlig HTTPS-forespørsel (GET). Den inneholder appversjonen og operativsystemet (Windows, x86_64) slik Tauris oppdateringsmodul oppgir dem. Som for alle nettforespørsler ser GitHub også maskinens IP-adresse og tidspunktet, og GitHub kan logge dette etter sine egne vilkår. Fjell sender **aldri** bilder, mappenavn, filstier, filnavn, indeksinnhold, konto- eller enhets-ID.
- **Mottaker:** GitHub (github.com og nedlastingstjenesten bak releases). Ingen andre mottakere, ingen egen server.
- **Installasjon:** skjer aldri automatisk. Du må selv trykke «Installer og start på nytt». Da lastes installasjonspakken ned fra samme release. Pakken er signert med en nøkkel (minisign), og appen avviser pakker der signaturen ikke stemmer med den innebygde offentlige nøkkelen. Fjell installerer ikke mens en skanning pågår.
- **Hva som lagres:** bare valget «Sjekk automatisk ved oppstart» (av/på) i Fjells innstillinger på maskinen din.
- **Slå av:** la valget stå av (standard) og ikke trykk «Se etter oppdatering». Du kan også blokkere `Fjell.exe` i Windows-brannmuren; resten av appen virker likt.
- Beta-pakkene er ikke signert med Authenticode-sertifikat. Det betyr at Windows SmartScreen kan advare ved manuell nedlasting og kjøring av installasjonsfilen. Appens egen nedlasting går ikke via nettleseren og utløser ikke denne advarselen.

## Deling

Fjell deler ikke data med noen. Det finnes ingen server å dele med. Du kan selv kopiere eller sikkerhetskopiere indeksen; den inneholder filstier, så oppbevar den som du ville gjort med en mappeliste.

## Nettverk

- Fjell har ingen egen nettverkskode og ingen innebygd nettleser mot eksterne sider; eneste unntak er oppdateringsprogramtillegget til Tauri (beta-kanalen, se «Oppdateringssjekk»). Innholdsregelen (CSP) i appvinduet tillater kun intern kommunikasjon mellom grensesnitt og Fjells egen kjerne (`ipc:`), ingen internettadresser, ingen eksterne fonter eller skript.
- **Oppdateringssjekk (beta):** se [Oppdateringssjekk](#oppdateringssjekk-beta-kanalen). Dette er et unntak fra at Fjell ellers ikke kontakter noen; det skjer bare på ditt initiativ eller hvis du har slått det på.
- **Installasjon:** installasjonsprogrammet kan hente Microsofts WebView2 hvis det mangler på maskinen. Det er installasjonsprogrammet (ikke Fjell) og kontakter bare Microsoft.
- **WebView2** (Microsofts nettleserkomponent) oppdateres av Windows/Microsoft på egen hånd og er ikke en del av Fjell. Fjell sender ikke noe gjennom den.
- **Nettverksstasjoner** (UNC): Fjell leser filer fra stasjonen du selv har valgt, via Windows. Det er ikke et kall til en tjeneste.
- Hvis du bruker Windows Sikkerhetskopiering eller OneDrive-synkronisering på mappene som inneholder `.fjell-karantene`, kan disse tjenestene kopiere karantenefilene som alle andre filer. Det styrer du i Windows.

## Slik kan du kontrollere det

- Kildekoden er åpen. Tredjepartskomponenter og lisenser: [THIRD_PARTY.md](../THIRD_PARTY.md).
- Automatiske tester som feiler hvis noen legger inn nettverkskall:
  - `desktop/src/lib/nonet.test.ts`: ingen `fetch`, `XMLHttpRequest`, `WebSocket`, `sendBeacon`, eksterne URL-er, CDN eller eksterne fonter i frontend-koden; CSP har `connect-src` bare for `ipc:` og `http://ipc.localhost`; Tauri-rettigheter uten http/shell/fil-plugin (nøyaktig tre unntak utover `core:*`: `dialog:allow-open`, Windows mappevalg for «Bla …» som bare gir tilbake en sti, samt `updater:default` og `process:allow-restart` for oppdateringssjekken); oppdaterings- og omstartspakkene bare i `tauri-bridge.ts` via dynamisk import; ingen nettverksbiblioteker i Fjells egne Rust-crates.
  - `desktop/e2e/nonet.spec.ts`: kjører alle skjermer i en nettleser og feiler hvis siden gjør én eneste forespørsel utenfor appen.
- Selv: følg med i Windows Ressursovervåking (Nettverk) mens du skanner og rydder, eller blokker `Fjell.exe` i Windows-brannmuren. Appen fungerer likt.

## Barn, samtykke, rettigheter

Fjell samler ikke inn personopplysninger og har derfor ingen behandlingsgrunnlag å oppgi, ingen databehandlere og ingen kontaktskjema. Dataene på maskinen din er dine; GDPR-rettigheter (innsyn, retting, sletting) oppfylles ved at du selv kan se og slette alt som er listet over.

## Endringer

Endringer i hva Fjell lagrer eller sender krever oppdatering av denne filen og av testene over i samme endring. Fjell-teamet skal ikke legge til telemetri uten at dette dokumentet endres synlig i utgivelsesnotatet.
