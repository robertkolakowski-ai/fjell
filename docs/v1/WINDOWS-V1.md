# V1: Windows-verifisering

Manuell testprotokoll for Robert (Narrator, NVDA, høykontrast, skalering) for hver V1-skjerm, pluss oversikt over de automatiserte tilleggene som kjører i Chromium og på `windows-latest`. Bygger på [WINDOWS-TEST.md](../mvp/WINDOWS-TEST.md) (MVP-sjekklisten) og [E2E-WINDOWS.md](../mvp/E2E-WINDOWS.md) (ekte-app-e2e). Skjermene er beskrevet i [SKJERMER.md](SKJERMER.md), kjernen i [V1-PLAN.md](V1-PLAN.md).

## 1. Status og dekning

**Ingenting i del 3 til 6 er prøvd av oss.** Vi har aldri kjørt V1 på ekte Windows, med Narrator, med NVDA eller med et ekte kontrasttema. «Forventet» i tabellene er hva koden og testene tilsier, ikke noe vi har observert. Rødt er et funn, ikke en feil hos deg: skriv det ned i del 7.

| Område | Automatisk (Chromium, MockBridge, kjører her og på `windows-latest`) | Bare manuelt (denne protokollen) |
|---|---|---|
| Høykontrast | `v1-win-hoykontrast.spec.ts`: `forced-colors: active` (lys og mørk basis), kant på knapper, outline på hvert fokusstopp, tilstand uten farge (På/Av, ikon), ingen `forced-color-adjust: none` på tekst, kontrast 4,5:1 på beregnede stiler | De fire ekte Windows-temaene (Aquatic, Desert, Dusk, Night sky), bytte mens appen kjører, WebView2s egen håndtering |
| Skalering | `v1-win-skalering.spec.ts`: 200 % (960x540 og minstevindu 480x520, rot-skrift 200 %), pseudo-lokale (+30 til 40 %), dialoger, kontrollstørrelse minst 24x24 px, WCAG 1.4.12 tekstavstand | 125 %, 150 %, 175 % og 200 % i ekte Windows-skalering, flere skjermer med ulik skalering, flytte vinduet mellom dem |
| Tastatur | `v1-win-tastatur.spec.ts`: Tab-gjennomgang av alle V1-skjermer (alt nåbart, ingen felle, navn, synlig fokus), flyter for Serie, Regler, Personer, Kortforklaring og Innstillinger med bare tastatur, dialoger som fanger og gir tilbake fokus | At WebView2 og Windows gir samme rekkefølge, F6 og andre vindustaster, skjermleserens egne taster |
| axe | `v1-win-axe.spec.ts`: alle V1-skjermer og dialoger i lys, mørk og høykontrast (to basisskjemaer), nb og en | Alt axe ikke ser: opplesningsrekkefølge, mening, støy |
| Skall og kjerne | `desktop/src-tauri/shell/tests/v1_kommandolag.rs`: V1-kommandoene gjennom skallets kommandolag mot ekte Fjell-kjerne, JSON-koder, feilkoder, capabilities, CSP, kommandoflate, 100 ms | Kjører i en ekte Tauri-app på Windows med ekte IPC |
| Skjermleser | (ingen) | Hele del 3: Narrator og NVDA |

Emulert `forced-colors` i Chromium er ikke det samme som et Windows-kontrasttema. Chromium bruker en fast palett; Windows lar brukeren velge farger. Derfor står høykontrast i del 4 også som manuell sjekk.

## 2. Forberedelser og data

Noter først: Windows-versjon og bygg (`winver`), skalering, Narrator- og NVDA-versjon, tastaturoppsett, dato, ditt navn, og hvilket bygg som er testet (artefakt `fjell-desktop-nsis-unsigned` fra arbeidsflyten «desktop», eller bygg fra `e2e-windows`).

Bruk en **kopi** av bilder, ikke dine eneste originaler. Gode testdata for V1:

- 40 til 100 bilder, med noen **serier** (3 til 6 bilder tatt tett i tid, gjerne med noen som har lukkede øyne) og noen nesten like bilder.
- 5 til 10 skjermbilder (PNG) og noen store filer, så regler har treff (for eksempel «Er et skjermbilde» eller «Større enn 5 MB»).
- Bilder med ansikter (bare hvis bygget har ansiktsmodell). **Ikke verifisert:** om bygget har en ekte ansiktsmodell; finnes den ikke, er Personer tom også med indeksen på. Skriv hva du så, ikke mer.
- Én mappe du kan beskytte i Innstillinger.

Skjermleseroppsett:

- **Narrator:** Win+Ctrl+Enter slår av og på. Narrator-tast er Caps Lock eller Insert. Les alt: Narrator+Ctrl+Piltast ned i skannemodus. Elementliste: Narrator+F5. Skannemodus av og på: Narrator+Mellomrom.
- **NVDA:** Ctrl+Alt+N starter. Elementliste: NVDA+F7. Les alt: NVDA+Piltast ned. Fokusmodus og bla-modus: NVDA+Mellomrom.
- Slå på talehistorikk i NVDA (Verktøy, Talevisning) og kopier det du hører ved funn.

Kryss av: `[x]` OK, `[F]` funn (skriv i del 7), `[-]` ikke prøvd. Skriv nummer fra tabellen i funnskjemaet.

## 3. Per skjerm: fokusrekkefølge og opplesning

Felles for alle skjermer (kryss av én gang per skjermleser, så bare avvik videre):

| # | Gjør | Forventet | N | NVDA |
|---|---|---|---|---|
| F1 | Åpne skjermen med lenken i navigasjonen (Enter). | Vindustittel «<Skjerm> · Fjell». Fokus står på hovedoverskriften (nivå 1) og den leses opp. Navigasjonslenken er «gjeldende side». | [ ] | [ ] |
| F2 | Trykk Tab fra toppen av siden. | Første stopp er «Hopp til hovedinnhold». Enter flytter fokus til innholdet. | [ ] | [ ] |
| F3 | Trykk Tab gjennom hele skjermen, deretter Shift+Tab tilbake. | Hver kontroll får fokus én gang, i leserekkefølge. Fokusrammen er alltid synlig. Ingen fokusfelle utenom i åpne dialoger. | [ ] | [ ] |
| F4 | Trykk F1. | Dialogen «Hurtigtaster» åpnes, fokus er i den, Esc lukker og fokus går tilbake. | [ ] | [ ] |
| F5 | Gjør en handling (se skjermen under). | Resultatet annonseres uten at fokus flyttes (statusmelding), og ingenting leses to ganger. | [ ] | [ ] |

### 3.1 Oversikt (Hjem)

Åpne «Hjem» etter en skanning. Panelet «Oversikt» ligger under kategorikortene.

| # | Gjør | Forventet fokusrekkefølge og opplesning | N | NVDA |
|---|---|---|---|---|
| 1.1 | Tab fra overskriften. | Legg til mappe, Rask opprydding, Papirkurv (tall), kategorilenkene (Eksakte duplikater, Like bilder, Uskarpe, Skjermbilder, **Serier**, **Regelforslag**) med størrelse og antall bilder i navnet, «Gå gjennom forslagene», så tellerlenkene («Beskyttede», «Serier», «Personer», «Regler»). | [ ] | [ ] |
| 1.2 | Les overskriftene (skannemodus, H eller 2). | «Oversikt», «Fremdrift», «Neste steg», «Kan ryddes per type», «Mer i Fjell», «Største mapper». Nivåene er logiske (2 under 1). | [ ] | [ ] |
| 1.3 | Les «Fremdrift». | «<n> av <m> forslag gjennomgått» som tekst, ikke bare en ring. | [ ] | [ ] |
| 1.4 | Les «Tømt» og «Kan ryddes». | «Tømt» sier at filene går til Windows Papirkurv og at plassen frigjøres først når du tømmer den. Ordet «frigjort» brukes ikke om noe som bare er flyttet. | [ ] | [ ] |
| 1.5 | Les grafen/tabellen «Tømt per måned» (hvis den finnes). | Tall i en tabell med kolonnene Måned, Bilder, Størrelse; ikke bare en tegning. | [ ] | [ ] |
| 1.6 | Enter på «Serier», «Personer», «Regler» i tellerlenkene. | Går til riktig skjerm, fokus på overskriften. | [ ] | [ ] |

Funn: ________________________________________________

### 3.2 Serie (Gjennomgang · Serier og Sammenlign bildene i serien)

Åpne Gjennomgang, «Serier», og «Sammenlign serien» på en serie.

| # | Gjør | Forventet fokusrekkefølge og opplesning | N | NVDA |
|---|---|---|---|---|
| 2.1 | Les serierader i Gjennomgang. | Hver rad: «Gruppe med <n> bilder, <størrelse>, Beste …». Lenken «Sammenlign serien» finnes per serie. | [ ] | [ ] |
| 2.2 | Åpne «Sammenlign serien». | Tittel «Gjennomgang · Fjell», overskrift «Sammenlign bildene i serien», fokus på overskriften. Ingen horisontal rulling. | [ ] | [ ] |
| 2.3 | Tab: rekkefølge. | Tilbake til Hjem, Zoom ut, Zoom inn, Tilbakestill zoom, så per bilde: «Beskytt bilde <n>» (På/Av), avmerking «Flytt til papirkurv» (ikke på beste), «Gjør dette til beste». Til slutt «Behold alle» og «Flytt <n> til papirkurv». | [ ] | [ ] |
| 2.4 | Les ett bilde. | «Bilde <n> av <totalt>», «Foreslått å beholde» (på beste, i tillegg til hake og ramme), rangering, poeng og begrunnelse («Best i serien», «Åpne øyne» eller «Lukkede øyne») som tekst. | [ ] | [ ] |
| 2.5 | Serien med lukkede øyne. | Begrunnelsen sier «Lukkede øyne» med antall. Hvis bygget ikke har øyemodell: «Øyne er ikke vurdert: bildene er rangert på skarphet.» (V1-PLAN åpent spørsmål 12). | [ ] | [ ] |
| 2.6 | Fokuser «Beskytt bilde 2» og trykk Space. | Annonserer «Bilde 2 er beskyttet.» Knappen leses som «Beskytt bilde 2, av/på-knapp, trykket/ikke trykket» (navnet endres ikke, bare tilstanden). Beskyttet bilde tas ut av utvalget; å fjerne beskyttelsen velger det ikke på nytt. | [ ] | [ ] |
| 2.7 | «Gjør dette til beste» på et annet bilde (Enter). | «Bilde <n> er nå foreslått beholdt.» Fokus flyttes til bildet, ikke til toppen. | [ ] | [ ] |
| 2.8 | Endre utvalg, les summen. | «<n> bilder valgt for papirkurv» annonseres. Knappen viser samme tall. | [ ] | [ ] |
| 2.9 | «Flytt <n> til papirkurv». | «Serien er ferdig. <n> bilder i papirkurven.» Fokus på resultatet, «Angre» er første kontroll. Angre gir bildene tilbake. | [ ] | [ ] |
| 2.10 | Beskyttet bilde i utvalget, flytt. | Dialogen «Beskyttede bilder» («Behold dem», «Flytt likevel») leses før knappene. Fokus i dialogen, Esc = behold. | [ ] | [ ] |
| 2.11 | Zoom: Tab til bildeområdet, trykk + − 0. | Zoomnivå annonseres («Zoom <pct>»). Alt har en knapp. | [ ] | [ ] |

Funn: ________________________________________________

### 3.3 Kortforklaring (Kortmodus for serie og regel)

Åpne «Rask opprydding» (`Rask opprydding` fra Hjem). Kortene av typen serie og regel har forklaringen «Hvorfor foreslås dette?».

| # | Gjør | Forventet fokusrekkefølge og opplesning | N | NVDA |
|---|---|---|---|---|
| 3.1 | Tab til kortet. | Kortet er én gruppe: «Forslag <n> av <totalt>. <størrelse>. <begrunnelse>» (for serie: «Bilde <i> av <n> i serien», for regel: «Regel: <navn>»). | [ ] | [ ] |
| 3.2 | Les forklaringen. | Overskrift «Hvorfor foreslås dette?», en setning om hvorfor, og fakta (Plass i serien, Regel, Antall, Sikkerhet) som tekst. Teksten sier «Appen foreslår. Du bestemmer.» Ingen dom («uskarpt» har «anslag, ikke en dom»). | [ ] | [ ] |
| 3.3 | Pil høyre (hopp over), pil venstre (behold), Ctrl+Z. | Hver gir én melding, for eksempel «<n> igjen». Knappene Behold, Papirkurv, Hopp over, Angre, Zoom gir samme resultat. | [ ] | [ ] |
| 3.4 | Beskyttet bilde. | Beskyttede bilder er aldri i bunken (SKJERMER.md åpent spørsmål 3). Skriv om du ser et. | [ ] | [ ] |
| 3.5 | F1. | Egen dialog «Hurtigtaster i kortmodus». Esc lukker, fokus tilbake på kortet. | [ ] | [ ] |
| 3.6 | Papirkurv-knappen. | Melding som «Flyttet til papirkurv. <n> igjen.» og Angre virker. | [ ] | [ ] |

Funn: ________________________________________________

### 3.4 Personer

Ansiktsindeksen er **av** som standard. Test begge tilstander.

| # | Gjør | Forventet fokusrekkefølge og opplesning | N | NVDA |
|---|---|---|---|---|
| 4.1 | Åpne Personer (indeks av). | Overskrift «Personer», delen «Ansiktsindeksen er av» forklarer at analysen bare skjer på denne PC-en og at originalene aldri endres. Én knapp: «Slå på ansiktsindeks». Ingen personliste. | [ ] | [ ] |
| 4.2 | Enter på «Slå på ansiktsindeks». | Dialogen «Slå på ansiktsindeks?» leses (Hva som lagres, Det forlater aldri enheten, Originalene endres aldri, Du kan slå av og slette). Fokus står på «Avbryt». | [ ] | [ ] |
| 4.3 | Esc. | Dialogen lukkes, ingenting slått på, fokus tilbake på «Slå på ansiktsindeks». | [ ] | [ ] |
| 4.4 | Åpne igjen, Tab til «Slå på», Enter. | «Ansiktsindeksen er slått på.» annonseres. Fokus på overskriften. Personliste (eller «Ingen personer funnet ennå» hvis ingen modell). | [ ] | [ ] |
| 4.5 | Les listen. | Liste med navn «Personer». Hver person: navn (eller «Uten navn»), antall bilder. Knapper: «Gi navn til <person>», «Slå sammen <person> med en annen person», «Beskytt <person>». | [ ] | [ ] |
| 4.6 | Gi navn: Enter på «Gi navn til …», skriv, Enter. | Fokus i tekstfeltet «Navn». Esc avbryter. Lagret navn annonseres, «Angre» finnes. For langt navn (over 100 tegn): feilen «Navnet er for langt.» leses, feltet er `aria-invalid`. | [ ] | [ ] |
| 4.7 | Slå sammen. | Dialogen «Slå sammen personer», radioknapper for hvem som blir stående, «Slå sammen» er av til du har valgt. Melding «<a> er slått sammen med <b>.» og Angre skiller dem igjen. | [ ] | [ ] |
| 4.8 | Beskytt en person (Space). | «<person> er beskyttet. Bilder av personen flyttes aldri uten din bekreftelse.» | [ ] | [ ] |

Funn: ________________________________________________

### 3.5 Regler

| # | Gjør | Forventet fokusrekkefølge og opplesning | N | NVDA |
|---|---|---|---|---|
| 5.1 | Åpne Regler. | Overskrift «Regler», «Regler foreslår bare. Ingenting slettes uten at du velger det.» Hver regel er en overskrift (nivå 2) med setningen «Foreslår bilder der alle disse stemmer: …» og antall treff. | [ ] | [ ] |
| 5.2 | Tab: rekkefølge. | «Ny regel», så per regel: bryter «Aktiv: <navn>» (leses som av/på-bryter), «Rediger regelen <navn>», «Slett regel <navn>». | [ ] | [ ] |
| 5.3 | Enter på «Ny regel». | Dialogen «Ny regel» åpnes, fokus i «Navn». Tab og Shift+Tab forlater ikke dialogen. Esc lukker og fokus tilbake på «Ny regel». | [ ] | [ ] |
| 5.4 | Fyll navn, velg vilkår (Type for vilkår 1), «Forhåndsvis treff». | «Treffer <n> bilder (<størrelse>)» annonseres. Ugyldig vilkår: feilen «Vilkåret er ugyldig.» leses (rolle varsel). «Lagre regel» krever forhåndsvisning først. | [ ] | [ ] |
| 5.5 | «Lagre regel». | Dialogen lukkes, «Regelen «<navn>» er lagret. Treffene kommer som forslag i Gjennomgang.» annonseres, fokus er ikke tapt. | [ ] | [ ] |
| 5.6 | Bryteren med Space. | «Regelen «<navn>» er slått av.» / «slått på.» | [ ] | [ ] |
| 5.7 | Slett regel, deretter Angre. | «Regelen «<navn>» er slettet. Bildene er urørt.» med Angre som første kontroll; Angre gir regelen tilbake. | [ ] | [ ] |
| 5.8 | Åpne Gjennomgang, Regelforslag. | Hvert kort sier «Regel: <navn>». | [ ] | [ ] |

Funn: ________________________________________________

### 3.6 Beskyttelser (Innstillinger, delen «Beskyttelser»)

| # | Gjør | Forventet fokusrekkefølge og opplesning | N | NVDA |
|---|---|---|---|---|
| 6.1 | Åpne Innstillinger og gå til gruppen «Beskyttelser». | Gruppen har navn. Teksten sier at beskyttede bilder, mapper, album og personer aldri flyttes uten bekreftelse, og at bildene aldri endres. Tom liste: «Du har ikke beskyttet noe ennå.» | [ ] | [ ] |
| 6.2 | Enter på «Beskytt en mappe». | Dialogen «Beskytt en mappe». Knappen «Beskytt» er av til du har skrevet en sti. Esc lukker og fokus tilbake på «Beskytt en mappe». | [ ] | [ ] |
| 6.3 | Skriv en ekte mappesti, «Beskytt». | «Mappen <sti> er beskyttet.» Mappen vises i listen med dato («Beskyttet <dato>»). Mappen og bildene er uendret i Utforsker. | [ ] | [ ] |
| 6.4 | «Fjern beskyttelse for <sti>». | Dialogen «Fjerne beskyttelsen?» sier at bildene ikke røres. Etter bekreftelse: «Beskyttelsen for <sti> er fjernet.» med Angre («Beskyttelsen … er på igjen»). | [ ] | [ ] |
| 6.5 | Flytt et bilde i den beskyttede mappen i Gjennomgang. | Dialog om beskyttede bilder, «Flytt likevel» krever eget valg. Standardfokus er på det trygge valget. | [ ] | [ ] |

Funn: ________________________________________________

### 3.7 Ansikter (Innstillinger, delen «Ansikter og personer»)

| # | Gjør | Forventet fokusrekkefølge og opplesning | N | NVDA |
|---|---|---|---|---|
| 7.1 | Gå til gruppen «Ansikter og personer». | Radiogruppe med «Av» og «På» (Av valgt). Tab går til gruppen som ett stopp, piler bytter. | [ ] | [ ] |
| 7.2 | Pil høyre (til «På»). | Samtykkedialogen «Slå på ansiktsindeks?» åpnes. Esc avbryter: «Av» er fortsatt valgt og fokus er i gruppen. | [ ] | [ ] |
| 7.3 | Slå på via samtykket. | «På» er valgt. «Slett ansiktsindeksen» finnes. | [ ] | [ ] |
| 7.4 | «Slett ansiktsindeksen». | Dialogen «Slette ansiktsindeksen?» sier «Dette kan ikke angres», at beskyttelse av personer faller bort (blir bildebeskyttelse) og at bildene beholdes. Fokus på «Avbryt». | [ ] | [ ] |
| 7.5 | Bekreft sletting. | «<n> ansikter er slettet fra indeksen. Bildene er beholdt.» annonseres. Bildene i Utforsker er uendret. | [ ] | [ ] |
| 7.6 | Slå av indeksen igjen. | Personer-skjermen er tilbake til «Ansiktsindeksen er av». | [ ] | [ ] |

Funn: ________________________________________________

### 3.8 Sensitivitet (Innstillinger, delen «Hvor mange forslag»)

| # | Gjør | Forventet fokusrekkefølge og opplesning | N | NVDA |
|---|---|---|---|---|
| 8.1 | Gå til gruppen «Hvor mange forslag». | Teksten sier «Appen foreslår bare. Du bestemmer alltid selv.» Tre radiogrupper med navn: «Like bilder», «Uskarpe», «Serier». Eksakte kopier, skjermbilder og regelforslag har ingen følsomhet (egen merknad). | [ ] | [ ] |
| 8.2 | Tab til «Like bilder». | Ett tabulatorstopp per gruppe, valgt radio er i fokus: «Normal». Alternativene: «Færre og sikrere forslag», «Normal», «Flere forslag, mindre sikre». | [ ] | [ ] |
| 8.3 | Pil høyre og venstre. | Valget endres og leses. Terskeltallene under («Største forskjell mellom like bilder: <n>») oppdateres og leses som vanlig tekst. | [ ] | [ ] |
| 8.4 | Les delen «Terskler akkurat nå (kan ikke endres her)». | Det er bare tekst (ingen tekstfelt). | [ ] | [ ] |
| 8.5 | Bytt til «Flere forslag» og gå til Gjennomgang. | Antall forslag er endret. Allerede avgjorte forslag er uendret. | [ ] | [ ] |

Funn: ________________________________________________

## 4. Høykontrast

Slå på temaene med Alt+venstre Shift+PrtScn, og test **alle fire**: Aquatic, Desert, Dusk, Night sky (Innstillinger, Tilgjengelighet, Kontrasttemaer). Gjør første runde med appen åpen og bytt tema uten å starte på nytt, så en runde med omstart.

| # | Gjør | Forventet | Aq | De | Du | Ni |
|---|---|---|---|---|---|---|
| H1 | Bytt tema mens appen kjører (Hjem). | Appen følger straks, ingen gjenværende lyse eller mørke flater. All tekst er leselig. | [ ] | [ ] | [ ] | [ ] |
| H2 | Hver V1-skjerm (Oversikt, Serie, Kortforklaring, Personer, Regler, Innstillinger med de tre delene). | Alle knapper har kant. Tekst og ikon har tema-farger. Ingen tekst forsvinner (for eksempel mot bildeflater). | [ ] | [ ] | [ ] | [ ] |
| H3 | Tab gjennom hver skjerm. | Fokusrammen er synlig overalt, også på bilder, bryterne, radioene og dialogknapper. | [ ] | [ ] | [ ] | [ ] |
| H4 | Serie: beste bilde og beskyttet bilde. | «Foreslått å beholde» skilles uten farge (hake, tekst, kant). «Beskytt»-knappen viser På/Av som tekst, ikke bare farge. | [ ] | [ ] | [ ] | [ ] |
| H5 | Regler: bryteren «Aktiv». | På og av skilles uten farge. | [ ] | [ ] | [ ] | [ ] |
| H6 | Oversikt: ring, fremdrift og stolper. | Tallene står som tekst. Stolper og ring er synlige (kant eller systemfarge), ikke borte. | [ ] | [ ] | [ ] | [ ] |
| H7 | Dialoger (Ny regel, Slå på ansiktsindeks, Slett ansiktsindeksen, Beskytt en mappe). | Kant rundt dialogen, tydelig skille mot bakgrunnen, farlige valg skilles med tekst og ikke bare rød farge. | [ ] | [ ] | [ ] | [ ] |
| H8 | Bruk Narrator og NVDA mens temaet er på. | Samme opplesning som i del 3. | [ ] | [ ] | [ ] | [ ] |

Funn: ________________________________________________

## 5. Skalering

Innstillinger, System, Skjerm, Skala. Start appen på nytt for hver verdi. Test 100 %, 125 %, 150 %, 175 % og 200 %. Test også **minstevindu** (dra vinduet til minste størrelse; Tauri-minimum er 480x520) ved 200 %, og språk **norsk** (nb er 15 til 30 % lengre enn en).

| # | Gjør | Forventet | 100 | 125 | 150 | 175 | 200 |
|---|---|---|---|---|---|---|---|
| S1 | Oversikt (Hjem). | Ingen avkuttet tekst, ingen sidelengs rulling, alle knapper kan nås. Kategorikortene brytes pent (ikke tre kolonner som klemmer). | [ ] | [ ] | [ ] | [ ] | [ ] |
| S2 | Navigasjonen (seks valg). | Brytes over flere linjer uten å skjule noe; ingen horisontal sideskroll (SKJERMER.md åpent spørsmål 10). | [ ] | [ ] | [ ] | [ ] | [ ] |
| S3 | Serie med fire bilder. | Bildene ligger i et rutenett som tåler bredden. Handlingsfeltet nederst dekker ikke innhold; siste bilde kan rulles fri. | [ ] | [ ] | [ ] | [ ] | [ ] |
| S4 | Kortforklaring. | Kort, forklaring og knapper er innenfor vinduet. Lang forklaring (norsk) kan leses uten å kutte. | [ ] | [ ] | [ ] | [ ] | [ ] |
| S5 | Personer (av, samtykke, på). | Samtykkedialogen har alle fire avsnitt og begge knapper tilgjengelig (ruller inne i dialogen om nødvendig). | [ ] | [ ] | [ ] | [ ] | [ ] |
| S6 | Regler og «Ny regel». | Redigeringsdialogen får plass, knappene «Forhåndsvis treff», «Avbryt», «Lagre regel» kan nås. | [ ] | [ ] | [ ] | [ ] | [ ] |
| S7 | Innstillinger: Sensitivitet, Ansikter, Beskyttelser. | Radiogruppene og listen brytes uten avkutting. | [ ] | [ ] | [ ] | [ ] | [ ] |
| S8 | Flytt vinduet mellom to skjermer med ulik skalering (hvis du har det). | Layouten tilpasser seg uten uskarpe eller avkuttede områder. | [ ] | [ ] | [ ] | [ ] | [ ] |
| S9 | Tekststørrelse: Innstillinger, Tilgjengelighet, Tekststørrelse 150 % med skalering 100 %. | Samme som for skalering. | [ ] | [ ] | [ ] | [ ] | [ ] |

Funn: ________________________________________________

## 6. Tastatur-bare helhetsflyt, nettverk og originaler

Gjør hele flyten uten mus (bare Tab, Shift+Tab, piler, Enter, Space, Esc, F1) fra Hjem til du har avgjort en serie, laget en regel, slått på ansiktsindeks, gitt en person navn og beskyttet en mappe.

| # | Gjør | Forventet | |
|---|---|---|---|
| K1 | Hele flyten over uten mus. | Alt kan gjøres. Fokus mistes aldri til toppen av siden uten grunn. Hver handling har en knapp (ingen tast eller gest er eneste vei). | [ ] |
| K2 | Åpne og lukk hver dialog med Esc. | Fokus går tilbake til knappen som åpnet den. | [ ] |
| K3 | Alt+Tab bort og tilbake midt i en dialog. | Fokus er fortsatt i dialogen. | [ ] |
| N1 | Ressursovervåking (Nettverk) mens du gjør del 3. Fantes trafikk fra `Fjell.exe`? | Ingen. (`msedgewebview2.exe` er Microsoft, ikke Fjell.) | [ ] |
| O1 | Etter del 3, sjekk i Utforsker: er alle originalbilder og mapper uendret (antall, størrelse, endringstid)? | Ja. Beskyttelser, regler, personer og indeks er bare konfigurasjon; bilder flyttes kun av «Papirkurv»-handlinger og er angrebare. | [ ] |
| O2 | Slett ansiktsindeksen og fjern en beskyttelse. | Ingen bilder berøres. | [ ] |

## 7. Funnskjema

Skriv ett funn per rad. Alvorlighet: **A** blokkerer bruk (for eksempel ikke nåbart med tastatur, tapt data), **B** stor ulempe (feil opplesning, tekst borte i høykontrast), **C** kosmetisk.

| Nr. | Skjerm | Tabellrad (f.eks. 3.2/2.6) | Skjermleser / tema / skala | Hva du så (og hørte) | Forventet | A/B/C |
|---|---|---|---|---|---|---|
| 1 | | | | | | |
| 2 | | | | | | |
| 3 | | | | | | |
| 4 | | | | | | |
| 5 | | | | | | |
| 6 | | | | | | |
| 7 | | | | | | |
| 8 | | | | | | |

Send tilbake: utfylt protokoll, skjermbilder av alt som ble `[F]` (uten private bilder synlige), talehistorikk fra NVDA ved opplesningsfunn, og Windows-versjon.

## 8. Funn fra de automatiske testene

Disse er funnet av V1-spesene og kontraktstesten og er **ikke rettet her** (filene er delte og låst for denne fasen). De står som egne tester som er grønne så lenge feilen finnes (`test.fail()` i Playwright, `#[should_panic]` i Rust) og som slår ut den dagen feilen er rettet, så merkingen fjernes da.

1. **Feilkoder fra skallet når ikke frontend (kontraktsbrudd, middels).** Skallet sender kjernefeil som `code = "core_error"` med den stabile koden i `detail` (`empty_rule`, `empty_name`, `name_too_long`, `rule_not_found`, `unknown_person`, `not_merged`, `unknown_series`). Frontend slår opp meldingen på `BridgeError.code` (for eksempel `ERROR_KEYS[e.code]` i `rulesState.ts`) og `MockBridge` sender `code = "empty_rule"` osv. I ekte app vil derfor regelfeil bli den generiske «Noe gikk galt. Ingenting er endret.» i stedet for «Regelen trenger et navn.», og tilsvarende for Personer. Test: `v1_error_codes_reach_frontend_as_the_code_it_has_messages_for` i `desktop/src-tauri/shell/tests/v1_kommandolag.rs`. Rettelse: enten `ErrorDto::from(ApiError::Internal(kode))` i `shell/src/dto.rs` mapper kjente V1-koder til `code`, eller `TauriBridge` løfter `detail` til `code` når `code === "core_error"` og `detail` er en kjent kode. Ingenting går tapt eller utføres feil; bare meldingen blir mindre presis.
2. **Kortmodus beholder bunken ved hash-skifte mellom korttyper (liten).** `Cards.svelte` oppretter `CardsSession` én gang fra `kind`-propen. Å bytte adresse direkte fra `#/cards/series` til `#/cards/rule` (uten å gå via en annen skjerm) viser fortsatt serie-bunken. Via Hjem eller Gjennomgang virker det riktig. Test: «Kortforklaring: hash-skifte mellom korttyper bytter bunke» i `v1-win-tastatur.spec.ts`. Rettelse: nøkle `<Cards>` på `kind` i `App.svelte`, eller gjenskap sesjonen når `kind` endres.
3. **Observasjoner som ikke er funn.** Beskyttelse av et bilde tar det ut av utvalget i Serie, og å fjerne beskyttelsen velger det ikke på nytt (trygg retning). Enter rett etter at en dialog er lukket og fokus er tilbake kan treffe før lukketilstanden er ferdig; testene venter derfor 150 ms (ikke raskere enn et menneske). Axe i Chromium leser designfargene i stedet for tvungne farger under `forced-colors` og gir falske kontrastbrudd (for eksempel `#06262b` mot svart på en knapp hvis beregnede farge er hvit); V1-spesene slår av `color-contrast` der og kontrollerer kontrast på beregnede stiler i stedet.

## 9. Kjøre de automatiske testene

Fra `desktop/` (Chromium er forhåndsinstallert her; på en ny maskin: `npx playwright install chromium`):

```
npm ci
npx playwright test e2e/v1-win-axe.spec.ts e2e/v1-win-hoykontrast.spec.ts e2e/v1-win-skalering.spec.ts e2e/v1-win-tastatur.spec.ts --trace off
```

Skallet (Rust, ingen WebView2 nødvendig):

```
cargo test --manifest-path desktop/src-tauri/Cargo.toml -p fjell-shell --locked --test v1_kommandolag
```

CI: `.github/workflows/e2e-windows.yml` har to jobber for V1 i tillegg til den eksisterende `real`: `v1-mock` (Playwright-spesene på `windows-latest`) og `v1-shell` (kontraktstesten på `windows-latest`). Triggere er som før (manuelt og nattlig). **Ikke verifisert:** at jobbene er grønne på `windows-latest`; de er bare kjørt på Linux her.

Filer: `desktop/e2e/v1-win-felles.ts` (hjelpere), `v1-win-axe.spec.ts`, `v1-win-hoykontrast.spec.ts`, `v1-win-skalering.spec.ts`, `v1-win-tastatur.spec.ts`. Hver spec har selvtester som viser at kontrollene finner feil (testene er ikke tomme).
