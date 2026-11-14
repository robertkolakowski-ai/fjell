# Windows-test: sjekkliste uten utviklerverktøy

For deg med en Windows-maskin og ingen Rust, Node eller Git. Utfyllende versjon for beta: [BETA.md](../BETA.md).

**Status: ingenting i denne listen er prøvd av oss.** Vi har bare kjørt kjerne, skall og grensesnitt på Linux (se `MVP-PLAN.md` 4b). Installasjonsprogrammet, Papirkurv, lange stier, skjermlesere, skalering og høykontrast er aldri sett på ekte Windows. «Forventet» er derfor hva vi tror, ikke noe vi har observert. Rødt er et funn, ikke en feil hos deg: skriv det ned.

Før du starter, noter: Windows-versjon og bygg (Win+R, `winver`), skalering, Narrator/NVDA-versjon, dato, ditt navn.
Bruk en **kopi** av noen bilder (20 til 50, helst med noen like og uskarpe), ikke dine eneste originaler. Kryss av i siste kolonne: `[x]` OK, `[F]` feil (skriv hva du så), `[-]` ikke prøvd.

## 1. Last ned og installer

| # | Gjør | Forventet | |
|---|---|---|---|
| 1 | Be om siste vellykkede kjøring av arbeidsflyten «desktop» på GitHub (Actions) og last ned artefaktet `fjell-desktop-nsis-unsigned`. Pakk ut zip-filen. | Én fil som slutter på `-setup.exe`. | [ ] |
| 2 | Dobbeltklikk `-setup.exe`. | Pakken er usignert: forvent «Windows beskyttet PC-en din» (Mer info, Kjør likevel). Skriv hva du faktisk så. | [ ] |
| 3 | Gjennomfør installasjonen som vanlig bruker. | Ingen spørsmål om administrator (per bruker). Norsk tekst i installasjonsprogrammet er **ikke sett av oss**; skriv hvilket språk du fikk. | [ ] |
| 4 | Mangler Microsoft Edge WebView2 på maskinen, skriv om installasjonen trengte nett. | Appen starter etterpå. Ellers: skriv «hadde WebView2». | [ ] |

## 2. Første start og mappe

| # | Gjør | Forventet | |
|---|---|---|---|
| 5 | Start Fjell fra Start-menyen. Mål tiden til vinduet er brukbart. | Vindu med velkomst (onboarding). Mål er under 1,5 s; **ikke målt av oss**. | [ ] |
| 6 | Velg mappe med testbildene. Mappevelger er ikke koblet: skriv inn hele stien, for eksempel `C:\Users\deg\Bilder\Test`. Bare én mappe støttes; en ny erstatter den forrige. | Skanning starter, antall funnet vises, Hjem viser tall per kategori. | [ ] |
| 7 | Åpne Gjennomgang og velg 3 bilder, trykk «Flytt … til papirkurv». | Bildene forsvinner fra Gjennomgang. Originalene er flyttet, ikke endret. | [ ] |
| 8 | Trykk Angre rett etterpå. | Bildene er tilbake. Sjekk i Utforsker at filene ligger på opprinnelig sted. | [ ] |
| 9 | Flytt tre bilder til papirkurv igjen. Åpne Utforsker, vis skjulte filer: finnes mappen `.fjell-karantene` på samme disk som bildene? | Ja, med filene i en undermappe, og en README. | [ ] |
| 10 | Åpne Papirkurv i Fjell. | De tre står der med dato og «Utløper om … dager». | [ ] |

## 3. Tøm til Windows Papirkurv (viktigste kontroll)

| # | Gjør | Forventet | |
|---|---|---|---|
| 11 | Skriv ned filnavnene til de tre. Trykk «Tøm papirkurv …», velg «Flytt til Windows Papirkurv», bekreft. | Melding: «Flyttet til Windows Papirkurv. Plassen frigjøres når du tømmer den.» Ikke ordet «frigjort» før permanent sletting. | [ ] |
| 12 | Åpne **Windows Papirkurv** på skrivebordet. | De tre filene **ligger der** med riktig navn. Dette er det eneste som beviser at Papirkurv-koden virker. | [ ] |
| 13 | Høyreklikk én fil, Gjenopprett. | Filen er tilbake på opprinnelig sted og kan åpnes. | [ ] |
| 14 | Kontroller at ingenting ble permanent slettet: «Slett permanent» skal ikke ha vært valgt, og krever avkrysning og ekstra bekreftelse. | Ingen fil borte uten at du valgte det. | [ ] |

## 4. Nettverksstasjon og lang sti (Unsupported-arket)

Dette er ikke prøvd av oss på Windows; på Linux er det testet med temp-kopi og falsk Papirkurv.

| # | Gjør | Forventet | |
|---|---|---|---|
| 15 | Legg til en mappe på en nettverksstasjon (`\\server\deling\...` eller tilordnet stasjon) med noen bilder. Skann, flytt noen til papirkurv, tøm. | Enten går det til Windows Papirkurv, eller arket «Noen bilder kan ikke flyttes til Papirkurv» vises. Skriv hvilket. | [ ] |
| 16 | Lag en mappe med sti over 260 tegn (mange nestede mapper med lange navn), legg en kopi av et bilde i den og gjenta. | Samme: Papirkurv eller Unsupported-arket. Aldri krasj. | [ ] |
| 17 | I arket: velg «La ligge i karantenen». | Meldingen sier at bildene ligger urørt. Sjekk i Utforsker at filene finnes. | [ ] |
| 18 | I arket: velg «Slett permanent …» (bare med testkopi). | Må krysse av «Jeg forstår …» før knappen virker. Teksten sier at det ikke kan angres. | [ ] |

## 5. Skymapper (OneDrive, Dropbox, iCloud for Windows)

Ikke støttet i MVP og **ikke prøvd mot ekte skytjeneste av oss**. Vi gjenkjenner bare mappenavn.

| # | Gjør | Forventet | |
|---|---|---|---|
| 19 | Legg til en mappe under `OneDrive`, `Dropbox` eller `iCloud Photos` (testkopi). | Skann og forslag virker. Flytting til papirkurv avvises med melding. Skyplassholdere («bare på nett») lastes ikke ned. Skriv om du fikk advarsel. | [ ] |
| 20 | Gi en synkmappe et eget navn (for eksempel `Mine bilder` i synk). | Vi oppdager den **ikke** (kjent begrensning). Noter hva som skjedde. | [ ] |

## 6. Skjermleser

Gå gjennom Hjem, Gjennomgang, Kortmodus og Papirkurv uten å se på skjermen. Hver skjerm: vindustittel, én hovedoverskrift og navngitte knapper.

| # | Gjør | Forventet opplest | |
|---|---|---|---|
| 21 | **Narrator** (Win+Ctrl+Enter): Hjem | Tittel «Hjem · Fjell», tall per kategori, ringen leses som «… kan ryddes. … i papirkurven». | [ ] |
| 22 | Narrator: Gjennomgang, gå gjennom flisene | Hver flis: nummer av antall, dato, uskarphet; valgt eller ikke. Sum annonseres etter valg. | [ ] |
| 23 | Narrator: Kortmodus, bruk knapper og piltaster | Én melding per beslutning, for eksempel «Flyttet til papirkurv. 9 igjen.» F1 åpner hurtigtaster, Esc lukker. | [ ] |
| 24 | Narrator: Papirkurv og Tøm-arket | Advarselen leses før den destruktive knappen. Fokus er i arket; Esc lukker og fokus går tilbake. | [ ] |
| 25 | **NVDA**: samme fire skjermer | Samme som 21 til 24. Ingenting leses to ganger. Elementliste (NVDA+F7) har unike navn. | [ ] |
| 26 | NVDA: «6 flyttet», «Skanner …» uten fokusflytting | Leses opp. Kjent risiko: status som settes inn med ferdig tekst leses ikke alltid (BETA.md del 8). Skriv hva som uteble. | [ ] |

## 7. Skalering og høykontrast

| # | Gjør | Forventet | |
|---|---|---|---|
| 27 | Skalering 100 %, 150 % og 200 % (Innstillinger, System, Skjerm), start appen på nytt hver gang. Se alle fire skjermer og Tøm-arket, på norsk. | Ingen avkuttet tekst, ingen sidelengs rulling, alle knapper kan nås. | [ ] |
| 28 | Høykontrast: slå på hvert av de fire temaene (Alt+venstre Shift+PrtScn) mens appen kjører. | Appen følger straks. All tekst leselig, knapper har kant, fokus er synlig, valgte bilder skilles uten farge. | [ ] |

## 8. Avinstaller

| # | Gjør | Forventet | |
|---|---|---|---|
| 29 | Apper og funksjoner, Fjell, Avinstaller. | Programmet fjernes. Brukerdata i `%LOCALAPPDATA%\Fjell\` og `.fjell-karantene` **beholdes** (sjekk, og at originalbildene er urørt). | [ ] |
| 30 | Åpne Ressursovervåking (Nettverk) mens Fjell kjørte i punkt 6 til 18. Fantes trafikk fra `Fjell.exe`? | Ingen. (`msedgewebview2.exe` er Microsoft, ikke Fjell.) | [ ] |

## Send tilbake

Utfylt liste, skjermbilder av alt som ble `[F]` (uten private bilder synlige), og Windows-versjon. Kun avkryssede rader teller som prøvd.
