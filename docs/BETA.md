# Beta: manuell testing og brukbarhet

Gjelder Windows-appen (Tauri 2 + WebView2). Det som kan automatiseres, kjøres i CI (Playwright + axe, `desktop/e2e/`). Det som bare kan sjekkes med ekte skjermleser, ekte Windows-skalering og ekte installasjon, står her og **må kryss av før hver beta** (PLAN-UX-I18N del 8). Ingen avkrysning i denne filen er gjort ennå; fyll ut tabellene i dokumentet «Beta-resultat» (kopi av `docs/BETA.md`, ett per runde).

## 0. Forutsetninger og logg

- Referansemaskin: Windows 11 (mellomklasse, lokal SSD) og en Windows 10 22H2. Skriv build-nummer, Windows-bygg, WebView2-versjon (`edge://version` eller Programmer og funksjoner), Narrator-/NVDA-versjon.
- Testbibliotek: `core/testdata`-generatoren eller en kopi med ca. 2 000 bilder (inkl. duplikater, uskarpe, skjermbilder), minst én ekstern disk og én nettverksstasjon.
- Installer med den signerte (eller usignerte) NSIS-pakken, ikke `tauri dev`.
- Resultatlinje per punkt: **OK / Feil (lenke til sak) / Ikke testet**, dato, tester.

## 1. Narrator (Windows 11, Win+Ctrl+Enter)

Mål: hele flyten kan fullføres uten å se skjermen og uten mus. Skru av skjermen eller se bort.

| # | Handling | Forventet |
|---|---|---|
| N1 | Start appen | Narrator leser vindustittel «Hjem · Fjell» (eller «Onboarding» første gang). Fokus er i vinduet. |
| N2 | Landemerker: `D` / Skift+`D` i skannmodus (Caps Lock+Mellomrom) | Ett banner, én navigasjon «Hovedmeny», ett hovedinnhold. Ingen navnløse eller duplikate regioner. |
| N3 | Overskrifter: `H` / Skift+`H` i skannmodus | Én h1 per skjerm, nivåer uten hopp. |
| N4 | Tab fra toppen | Første stopp er «Hopp til hovedinnhold». Enter flytter til innholdet. |
| N5 | Onboarding: Kom i gang → mapper → Start skanning | Hver ny skjerm annonserer tittelen (fokus går til h1). Fremdrift/«Fant N …» annonseres høflig, ikke for ofte. Fullfør uten mus. |
| N6 | Resultat: «Flytt N til papirkurv» | Knappen har navn med antall og størrelse. Bekreftelsesark (≥ 50 bilder eller ≥ 1 GB): navn leses, fokus inne i arket, Esc lukker, fokus tilbake på utløser. |
| N7 | Gjennomgang: velg flere bilder | Hver flis leses som «Bilde 3 av 12. Tatt …. Uskarpt 82 %.», tilstand «valgt/ikke valgt» (Narrator: «trykket/ikke trykket»). Ny sum annonseres (maks hvert 300 ms). |
| N8 | Gruppevisning: bytt beste, «Behold dette, flytt N til papirkurv», Angre | Utfallet annonseres. Fokus havner et fornuftig sted etter handlingen (ikke på body). |
| N9 | Kortmodus: 10 kort med knappene og med piltaster (fokusmodus: Caps Lock+Mellomrom) | Hver beslutning annonseres én gang («Flyttet til papirkurv. 9 igjen.»). F1 åpner hurtigtaster, lukkes med Esc. |
| N10 | Papirkurv: Gjenopprett, så «Tøm papirkurv …» → Windows Papirkurv | Advarsel står før den destruktive knappen i lesrekkefølgen. Resultatmelding leses. «Plassen frigjøres først når du tømmer Windows Papirkurv» er med. |
| N11 | Tøm → «Slett permanent» | Ekstra trinn med avkrysning; «Slett N permanent» er sist og deaktivert til avkrysning. |
| N12 | Innstillinger: bytt språk nb ↔ en | Språket byttes straks; `lang` på siden følger; ingen halvoversatt tekst. |
| N13 | Alt over ved 200 % (se punkt 3) og i høykontrast (punkt 4) | Samme resultat. |

## 2. NVDA (siste stabile, Firefox-uavhengig; WebView2 = Chromium)

Samme skript N1–N13 som **V1–V13**, med disse tillegg:

| # | Handling | Forventet |
|---|---|---|
| V-a | Bla gjennom i søkemodus (piler), så Tab til knapper | Ingen elementer leses to ganger (f.eks. kort med både `aria-label` og overskrift). |
| V-b | Elementliste (NVDA+F7): landemerker, overskrifter, lenker, knapper | Navn er unike og meningsfulle. |
| V-c | Fokusmodus/søkemodus i kortmodus (NVDA+Mellomrom) | Piltaster virker bare i fokusmodus; knappene alltid. Tilstand annonseres av NVDA. |
| V-d | Statusmeldinger («6 flyttet», «Skanner …») uten fokusflytting | NVDA leser dem. **Kjent risiko:** live-region som settes inn *med* ferdig tekst leses ikke alltid; se `desktop/e2e/aria.spec.ts` og avsnittet «Kjente funn». |
| V-e | Dialog åpen: pil opp/ned i søkemodus | Kommer ikke ut av dialogen (modal). |

## 3. Skalering 100 / 150 / 200 % (Innstillinger → System → Skjerm)

Automatisk dekket (emulert i `desktop/e2e/herding.spec.ts`: devicePixelRatio 1/1,5/2, rot-skriftstørrelse 100/150/200 %, minstevindu 480×520). Manuelt på ekte Windows:

| # | Handling | Forventet |
|---|---|---|
| S1 | Sett skalering 100, 125, 150, 175, 200 % (logg av/på ved behov) og start appen på nytt | Ingen avkuttet tekst, ingen horisontal rulling, alle knapper nås. |
| S2 | «Tilgjengelighet → Tekststørrelse» 150 % og 225 % (bare tekst) | Tekst vokser, layout bryter om, ingenting dekkes av bunnlinjen. |
| S3 | Flytt vinduet mellom to skjermer med ulik skalering | Skarpt etter flytting, ingen layoutfeil. |
| S4 | Minste vindu (480×520) ved 200 % + tekst 150 % | Alle skjermer kan brukes med vertikal rulling. |
| S5 | Norsk (nb) på alle skjermer ved 200 % | Lange tekster brytes, ingen `…` midt i en handlingsknapp. |
| S6 | Zoom i vinduet (Ctrl + `+`/`-`) | Fungerer til 200 % uten tap av funksjon (WCAG 1.4.4, 1.4.10). |

## 4. Høykontrast (Innstillinger → Tilgjengelighet → Kontrasttemaer)

Automatisk dekket: `forced-colors: active` lys/mørk, alle skjermer, kantlinjer, fokusring og valgtilstand uten farge. Manuelt: de fire Windows-temaene (Aquatic, Desert, Dusk, Night sky) og Alt+venstre Shift+PrtScn.

| # | Handling | Forventet |
|---|---|---|
| H1 | Slå på hvert av de fire temaene mens appen kjører | Appen følger umiddelbart, ingen omstart. |
| H2 | Alle skjermer og dialoger i hvert tema | All tekst leselig, knapper har kant, deaktiverte knapper skilles (GrayText), fokus synlig. |
| H3 | Valgte bilder i Gjennomgang | Valgt er synlig uten farge (hake og tykk kant). |
| H4 | Fremdriftsring og begrunnelsesmerker | Ikon + tekst, ikke bare farge. |
| H5 | Primærknappen er skilt fra sekundær | Tykkere kant. |

## 5. Redusert bevegelse

Windows: Innstillinger → Tilgjengelighet → Visuelle effekter → Animasjonseffekter **av**. Appen skal ikke animere (automatisk dekket via `prefers-reduced-motion`). Kryss av: ingen glidende ringfyll, ingen toast-innsvev, ingen kortsvev.

## 6. Installasjonssjekkliste

Se også [RELEASE.md](RELEASE.md). Gjøres på rene Windows 10 22H2 og Windows 11 (VM duger).

| # | Sjekk | Forventet |
|---|---|---|
| I1 | Last ned installasjonsprogrammet; SmartScreen | Signert: ingen advarsel eller kjent utgiver. Usignert: «Windows beskyttet PC-en din» (forventet inntil signering). Dokumenter hva testeren så. |
| I2 | Kjør som vanlig bruker (ikke administrator) | Installeres per bruker uten UAC-spørsmål. |
| I3 | Maskin uten WebView2 (Windows 10 uten Edge-oppdatering) | Installasjonsprogrammet henter/inkluderer WebView2 og appen starter. Noter om nettilgang var nødvendig under installasjonen. |
| I4 | Start fra Start-menyen | Vindu åpnes, kald oppstart < 1,5 s (mål med stoppeklokke; mål i CI er på Mock). |
| I5 | Første kjøring uten nett | Fungerer fullt ut (appen trenger ikke nett). |
| I6 | Brannmur-/nettverksovervåking (Resource Monitor → Nettverk, eller Wireshark) under skann, kortmodus og Tøm | **Ingen trafikk fra `Fjell`-prosessen.** Notér eventuelle WebView2-prosesser separat (`msedgewebview2.exe` kan snakke med Microsoft for egne oppdateringer; det er ikke Fjell). |
| I7 | Data: `%LOCALAPPDATA%\Fjell\` | `index.db` og `cache\` finnes; ingenting i `Dokumenter`, ingen loggfiler med filstier utenfor denne mappen (ikke verifisert på Windows ennå). |
| I8 | Karantene: flytt 5 filer til papirkurv | `.fjell-karantene\` på samme volum; originalinnhold uendret (sjekk hash). Angre flytter tilbake. |
| I9 | Tøm til Windows Papirkurv | Filene ligger i Windows Papirkurv og kan gjenopprettes. |
| I10 | Ekstern disk ut midt i skann; nettverksstasjon frakoblet | Appen krasjer ikke; bilder vises ikke som slettet. |
| I11 | Avinstaller (Apper og funksjoner) | Programfiler fjernes. Brukerdata (`%LOCALAPPDATA%\Fjell\`, karantenemapper) **beholdes**, og avinstallasjon sier det. Ingen original røres. |
| I12 | Oppgrader over eksisterende versjon | Indeks migreres, ingen tap. |
| I13 | Installer på en bruker uten skrivetilgang til Bilder / UNC | Tydelig melding, ingen krasj. |

## 7. Brukbarhetstest (5 deltakere)

**Hvorfor 5:** Nielsen/Landauer-kurven gir om lag 85 % av de store problemene med 5 deltakere i én runde. Etter fiks: ny runde med 3–5 nye.

**Rekruttering:** 5 personer som har 5 000+ bilder på PC og ikke har brukt Fjell. Minst 2 over 55 år, minst 1 som bruker skjermforstørrelse eller høy skalering, gjerne 1 som bruker skjermleser (egen økt, se punkt 1–2). Norsk som hovedspråk. Ingen ansatte eller venner av teamet. Samtykke til notater (ingen opptak av deltakerens bilder; bruk testbiblioteket, ikke deltakerens egne bilder).

**Oppsett:** 30 min per deltaker, referansemaskin, installert app med testbibliotek (tre mapper, ~3 000 bilder). Fasilitator sier lite: «Tenk høyt. Jeg kan ikke hjelpe, vi tester appen, ikke deg.» Observatør noterer tid, feiltrykk og sitater.

**Oppgaver** (rekkefølgen er fast):

| # | Oppgave (leses opp, uten appens ord) | Suksess | Terskel |
|---|---|---|---|
| T1 | «Du vil rydde i bildene dine. Kom i gang.» (onboarding fra første start) | Fullfører til resultatskjermen uten hjelp | ≤ 60 s, 5 av 5 |
| T2 | «Fjern de eksakte kopiene.» | Flytter til papirkurv (med bekreftelse hvis vist) | 5 av 5 uten hjelp |
| T3 | «Du ombestemte deg. Få dem tilbake.» | Bruker Angre eller Gjenopprett | 4 av 5 uten hjelp; 5 av 5 med ett hint |
| T4 | **Forståelse:** «Er plassen på disken frigjort nå?» | Svarer **nei** og forklarer at papirkurven må tømmes | 5 av 5 (PLAN-UX-I18N del 8) |
| T5 | «Finn de uskarpe bildene og fjern tre av dem.» | Finner Gjennomgang → Uskarpe, velger 3, flytter | 4 av 5 |
| T6 | «Ta noen raske avgjørelser på bilder én og én.» (kortmodus, 10 kort) | Bruker knapper eller taster; forstår Behold/Flytt | 4 av 5; ingen utilsiktet «Flytt» (spør etterpå) |
| T7 | «Frigjør plassen for fullt.» (Tøm) | Åpner Tøm, velger Windows Papirkurv, forstår forskjellen til permanent | 5 av 5 velger ikke permanent ved uhell |
| T8 | «Hvor ligger bildene dine nå? Har appen endret noen av dem?» | Sier at originalene er uendret/ligger i papirkurv | 4 av 5 |
| T9 | «Bytt språk til engelsk.» | Finner Innstillinger | 4 av 5 |

**Mål (alle må holde for å gå til bred beta):**
- 0 deltakere mister et bilde de bryr seg om (alle slettede er gjenopprettbare i økten).
- T4: 5/5 riktig. T7: 0 utilsiktet permanent sletting.
- SUS-skjema etter økten: snitt ≥ 80, ingen enkeltscore < 68.
- Hver feil som rammer ≥ 2 av 5 får en sak og rettes før neste runde. Funn rapporteres med tall, ikke vurdert bort.

**Rapport per deltaker:** tid per oppgave, antall hint, feiltrykk, sitater, SUS. Samlet tabell: oppgave × deltaker. Ærlighet: oppgaver som feiler rapporteres som feil, uavhengig av mål.

## 8. Kjente funn fra ARIA-gjennomgangen (UX-6)

Rettet i `desktop/src/lib/components`: se commit-meldingen. Gjenstår i `desktop/src/screens` (utenfor UX-6-omfanget, meldes til skjermeier):

- `role="status"`/`role="alert"` settes inn **sammen med teksten** i flere skjermer (f.eks. «Laster …», «N gjenopprettet»). Narrator og NVDA annonserer ofte ikke slike. Løsning: en vedvarende `LiveRegion` per skjerm (komponenten finnes) og bytt teksten.
- `Toast` har fått forsinket tekst slik at regionen finnes før teksten, men er ikke i bruk på skjermene ennå.

## 9. Beta-utgang

Beta kan åpnes når: alle punkter i 1–6 er OK eller har akseptert og dokumentert unntak, brukbarhetsmålene i 7 er nådd, `just check`, `just desktop-check`, `npx playwright test` og CI-jobben `desktop` er grønne, og [RELEASE.md](RELEASE.md) sjekklisten er gått gjennom.
