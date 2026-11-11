# Plan: UX, i18n og tilgjengelighet (MVP)

Status: forslag fra UX-spesialist. Bygger på `ARKITEKTUR.md` (bindende) og `PRODUKT.md` (del 3, Hovedskjermer, Onboarding).
Prinsipp: UI er tynt. Kjernen eier logikk, tall og begrunnelseskoder. UI eier tekst, formatering, bilder, bevegelse og tilgjengelighet.

## 1. Omfang

| Skjerm | MVP | Merknad |
|---|---|---|
| Onboarding (60 s) | Ja | Eksakte duplikater først (rask skann), ett trykk |
| Hjem | Ja | Ring «kan frigjøres», kategorikort, rask opprydding |
| Gjennomgang | Ja | Rutenett per kategori, fast bunnlinje |
| Gruppevisning | Ja | Karusell, «Beste», bytt vinner |
| Swipe-modus | Ja | Alltid med knappealternativ |
| Papirkurv + «Tøm» | Ja | Gjenopprett, utløp, tøm med bekreftelse |
| Innstillinger | Minimal | Språk, papirkurvfrist, lyd/haptikk. Følsomhet per kategori utsettes (terskler er konstanter i kjernen) |
| Sammenligning, Samlinger, regler, album | Nei | Utenfor MVP. Swipe «høyre = album» erstattes, se 5 |

Avvik fra PRODUKT.md (begrunnet): «Slett» heter «Flytt til papirkurv» overalt. Lagring frigjøres først ved «Tøm» (ARKITEKTUR #12). Ordet «frigjort» brukes bare om faktisk tømt plass.

## 2. Designregler som gjelder alle skjermer

- Én primærhandling per skjerm, aksentfarge bare der. Bildet er hovedperson, fullskjerm der det er plass.
- Ingen auto-forhåndsvalg bortsett fra: eksakte duplikater og visuelle duplikater (alt unntatt keeper), men aldri beskyttede bilder (TRYGGHET 4.2). Forhåndsmerking er UI-valg (kjernen setter aldri `accepted`); avklart av produkteier 2026-11-11. Uskarpe og skjermbilder er **aldri** forhåndsvalgt, de krever eget valg eller swipe.
- Keeper kan ikke velges for papirkurv i samme gruppe (UI hindrer at alle i en gruppe trashes).
- Alle handlinger er optimistiske: UI oppdateres umiddelbart, kjernekall går på én seriell bakgrunnskø (rekkefølgen bevares), feil gir tilbakerulling og melding.
- Angre er alltid synlig etter en handling (knapp, ikke bare snackbar). Angre = `undo()` = siste batch.
- Ingen poeng, streaks eller trykk-varsler. Oppsummering er rolig og sann.
- Tall formateres i UI (plattformens `ByteCountFormatter`, `Formatter.formatFileSize`, `NumberFormatter`). Kjernen leverer `u64` bytes og heltall.

## 3. Skjermspesifikasjoner

### 3.1 Onboarding (budsjett 60 s)

| Trinn | Innhold | Budsjett | Kjerne/UI |
|---|---|---|---|
| 1 Velkommen | Én setning, én knapp «Kom i gang». Ingen konto. | ≤ 5 s lesing | UI |
| 2 Tilgang | Forklaring («Bildene dine blir på telefonen. Ingenting sendes noe sted.») så systemdialog | ≤ 10 s | UI + adapter |
| 3 Rask skann | Fremdrift med levende tall («Fant 312 eksakte duplikater»). Kun metadata + BLAKE3 ved lik størrelse, ingen dekoding. Hopp over ikke-lokale (iCloud) aktiva. | ≤ 30 s for 10 000 bilder, første tall ≤ 5 s | `start_scan(mode=quick)` |
| 4 Resultat | «312 eksakte duplikater (2,1 GB)». Primærknapp «Flytt 312 til papirkurv». Sekundær «Se gjennom først». Ved ≥ 50 bilder eller ≥ 1 GB kreves `MassAck` med advarsel «Fjell kan ikke se om disse er sikkerhetskopiert» før commit (TRYGGHET 4.3). | ≤ 5 s | `cards(exact)` + summary |
| 5 Seier | «312 bilder i papirkurven. 2,1 GB kan frigjøres når du tømmer.» Knapper «Angre» og «Til Hjem». | - | `decide_many` |
| 6 Hjem | Full analyse (dekoding, hash, uskarphet) fortsetter i bakgrunnen med synlig fremdrift | - | `start_scan(mode=full)` |

Regler og kantsaker:
- Hvis rask skann ikke er ferdig etter 45 s: vis «Foreløpig resultat» med det som er funnet, fortsett i bakgrunnen.
- 0 eksakte duplikater: «Ingen eksakte duplikater. Vi leter videre etter like bilder i bakgrunnen.» → Hjem med fremdrift. Ingen kunstig seier.
- Begrenset tilgang (iOS «Utvalgte bilder»): fungerer som vanlig på utvalget. Banner på Hjem: «Du har gitt tilgang til noen bilder» + knapp for å endre.
- Tilgang avslått: egen tilstand med forklaring og «Åpne innstillinger». Appen krasjer eller tømmes aldri.
- Avbrutt skann (app lukket): gjenopptas via `mod_token`. Onboarding vises ikke på nytt.
- Merk: «Flytt til papirkurv» krever ingen systemdialog (tilstand i indeksen). Systemdialog kommer først ved «Tøm».

### 3.2 Hjem

| Element | Innhold | Datakilde |
|---|---|---|
| Ring | «X GB kan ryddes» = sum bytes for ventende forslag (uten keepere) + papirkurv-bytes vises separat under: «Y GB i papirkurven» | `home_summary` |
| Kategorikort | Eksakte duplikater, Like bilder, Uskarpe, Skjermbilder. Hver: antall + GB + chevron | `home_summary.per_kind` |
| Rask opprydding | Primærknapp: swipe-modus over kort sortert på konfidens (maks 20) | `cards(all, 20)` |
| Skannstatus | Rolig linje: «Analyserer … 4 210 av 10 000» med avbryt/pause. Bare synlig under skann | `scan_progress` |
| Papirkurvlenke | «Papirkurv (312)» | `trash_summary` |
| Banner utløpt | «5 bilder i papirkurven er utløpt. De er ikke slettet.» → Papirkurv | `trash_summary.expired` |

Tomtilstand (alt ryddet): «Ingenting å rydde nå. Vi sier fra når vi finner mer.» uten feiring.
Ringen har alltid tekstekvivalent (tall i klartekst), aldri bare farge/vinkel.

### 3.3 Gjennomgang

- Kategori valgt fra Hjem. Duplikatkategorier viser **grupper som rader** (miniatyrer + «Beste» + antall). Uskarpe og skjermbilder viser **rutenett** med begrunnelsesmerke.
- Fast bunnlinje: «Flytt 87 til papirkurv (1,2 GB)». Deaktivert ved 0 valgt. Antall og bytes summeres av UI fra kortdata (`bytes` per kort), ikke et kjernekall per trykk.
- Valg: trykk på miniatyr veksler. Meny: «Velg alle», «Fjern alle valg». Lang trykk åpner stor visning (zoom).
- Commit = ett `decide_many`-kall = én batch. `AckRequired`/`MassAck` (beskyttede, ≥ 50 bilder) håndteres med bekreftelsesark før commit; beskyttede bilder ligger i egen seksjon og er aldri i «Velg alle». Etter commit: bunnlinje blir «87 flyttet. Angre».
- Per kort: begrunnelsesmerker (se 6.3). Konfidens vises bare for uskarp/skjermbilde («Uskarpt 82 %»).
- Sortering: konfidens synkende (kjernens `next_cards`). Pagineres med markør (infinite list, 60 kort per side).

### 3.4 Gruppevisning

- Horisontal sidevending (karusell) med miniatyrstripe under. «Beste» markert med ikon + tekst. Valgt kort har tydelig ramme (ikke bare farge).
- Under bildet: kort tabell for aktivt bilde: oppløsning, filstørrelse, dato, relativ skarphet («Skarpere enn de andre»). Gir «zoom for forskjeller» uten Sammenligning-skjerm: dobbelttrykk/knip zoomer, zoomnivå beholdes ved sidevending.
- Handlinger: «Behold dette, flytt N til papirkurv» (primær), «Behold alle» (sekundær, `keep` på hele gruppen), «Gjør dette til beste» (bytter keeper, ett trykk, `set_keeper`).
- Eksakte grupper bruker samme visning. Forklaring: «Identiske filer».
- Skjermleser: grupper vises som liste (ikke karusell). Se 8.

### 3.5 Swipe-modus

| Handling | Gest | Knapp (alltid synlig) | Kjernekall | Merknad |
|---|---|---|---|---|
| Behold | Sveip venstre | «Behold» | `Decision::Keep` | Forslaget markeres avvist, vises ikke igjen |
| Papirkurv | Sveip opp | «Papirkurv» | `Decision::Trash` | Hele batch-regelen gjelder (angre) |
| Hopp over | Sveip høyre | «Hopp over» | ingen (UI-lokal, kortet havner bakerst i økten) | Erstatter «album» (utenfor MVP) |
| Angre | Knapp + rist (valgfritt, av som standard) | «Angre» | `undo()` | Alltid én tilgjengelig |
| Stor visning | Lang trykk | - | ingen | Zoom, ikke Sammenligning |

- Kortstabel viser 1 kort + 2 bak. Forhåndslast 3 (ARKITEKTUR del 4). Bilder i skjermstørrelse hentes av plattformen, ikke kjernen.
- Overlegg under dra: farge + ikon + tekst (aldri farge alene). Terskel 35 % av bredde/høyde eller fart; under terskel spretter kortet tilbake.
- Haptikk: lett ved terskelkryssing, middels ved commit. Lyd av som standard (innstilling). Respekter «Reduser bevegelse»: kryssfade i stedet for flyvende kort.
- Budsjett: slipp til neste kort på plass **< 100 ms** (p95). Kjernekall < 10 ms kjøres bak animasjonen.
- Slutt på kortbunken: rolig oppsummering «Du flyttet 41 bilder til papirkurven og beholdt 12» + «Angre siste» (kan trykkes gjentatte ganger: kjernen har en stabel av batcher, `undo()` tar siste ikke-angrede). Ingen samlet «angre alle» i MVP.
- Skjermleser og hurtig tilgang: gester slås av, handlingene finnes som egendefinerte handlinger og knapper (8).

### 3.6 Papirkurv

| Element | Innhold |
|---|---|
| Topp | «312 bilder · 2,1 GB kan frigjøres» (alltid med forklaring: «Plassen frigjøres først når du tømmer papirkurven.») |
| Liste | Gruppert: «Utløpt», «Utløper snart (≤ 7 dager)», øvrige. Rad: miniatyr, dato flyttet, «Utløper om 12 dager» |
| Valg | «Gjenopprett» (valgte), «Gjenopprett alt». Gjenoppretting er `restore(batch)`, umiddelbar, uten bekreftelse |
| Tøm | Knapp «Tøm papirkurv…». Skjerm 1 (appens ark): antall, GB, «Dette kan ikke angres», sekundær «Avbryt», destruktiv «Fortsett». Skjerm 2: plattformens egen dialog (PhotoKit/MediaStore) |
| Utløpt | Merkes «Utløpt», aldri stille slettet. Vis banner på Hjem. Lokalt varsel er valgfritt (innstilling, av som standard, spørres ikke i onboarding) |
| Historikk | Én linje nederst: «Du har frigjort 4,8 GB siden mars» basert på faktisk tømt (`purged` med `cause` ≠ external) |

Åpent UX-punkt fra arkitekturen («Tøm» og plattformdialogen): løst slik at appens ark forklarer og plattformdialogen utfører. Antakelse som må verifiseres på referanseenhet: iOS flytter slettede bilder til «Nylig slettet» i 30 dager, så plassen frigjøres der først etter nytt tømmetrinn. Hvis bekreftet, legges en infolinje til i arket: «På iPhone ligger bildene i ‹Nylig slettet› i 30 dager.» Tekst i `i18n` er klar, vises per plattform.

## 4. UI-tilstand per skjerm (kontrakt for agenter)

Hver skjerm = én fil med en `ViewState`-type (sealed/enum) og én ren funksjon `reduce(state, event)`. Testes uten UI.

| Skjerm | Tilstander |
|---|---|
| Onboarding | welcome, permissionRationale, permissionDenied, scanning(found), partial(found), result(count, bytes), nothingFound, done(count, bytes) |
| Hjem | loading, scanning(progress), ready(summary), limitedAccess, allClean, error |
| Gjennomgang | loading, list(items, selection), committing, committed(batch), empty, error |
| Gruppe | loading, ready(group, activeIndex, zoom), committed, error |
| Swipe | loading, card(top, nextN), dragging(dir, progress), deckEmpty(stats), error |
| Papirkurv | empty, list(sections, selection), confirmPurge, purging, purged(bytes), error |

Filstruktur (iOS): `Features/{Onboarding,Home,Review,Group,Swipe,Trash,Settings}`, `Design/` (tokens, komponenter), `CoreBridge/` (mapper UniFFI-typer til ViewState), `Strings/` (generert). Ingen forretningslogikk utenfor `CoreBridge`. Hver fil ≤ ca. 300 linjer.

## 5. i18n (nb, en)

### 5.1 Regler

- Kilde: `i18n/nb.json` (kildespråk), `en.json` (`nn.json` etter MVP). Generert til `.xcstrings` av `tools/gen-i18n`. Ingen manuell redigering av genererte filer.
- Ingen tekst i kjernen. Kjernen leverer `ReasonCode` + parametre. UI mapper kode → nøkkel `reason.<kode>`.
- Ingen strengsammenstilling i kode. Hele setninger med navngitte plassholdere: `{count}`, `{size}`, `{days}`. Datoer, tall og størrelser formateres med plattformens locale og sendes inn ferdig formatert.
- Flertall med ICU-delmengde: `{count, plural, one {# bilde} other {# bilder}}`. gen-i18n støtter bare `plural`, `select` og enkle plassholdere, og feiler på resten. (nb og en har kategoriene `one` og `other`.)
- Nøkkelnavn: `<område>.<element>.<tilstand>`, små bokstaver, punktum. Eks. `home.ring.caption`, `trash.purge.confirm.title`.
- Hver nøkkel har valgfri `"ctx"` (beskrivelse til oversetter) og `"len": "short"` for knapper, faner og merker (kort plass).
- Bruk språkets egne typografiregler: hard mellomrom før `%` og mellom tall og enhet i nb (`82 %`, `1,2 GB`). Formateres av plattformen, ikke i JSON.
- Språkvalg: følger systemspråk, kan overstyres i Innstillinger (`setting.language`). Språk uten oversettelse (nn etter MVP) faller tilbake nøkkelvis til nb.

### 5.2 Format

```json
{
  "_meta": { "locale": "nb", "version": 1 },
  "home.ring.caption": { "t": "{size} kan ryddes", "ctx": "Under ringen på Hjem. {size} = formatert filstørrelse" },
  "review.bar.trash": { "t": "{count, plural, one {Flytt # til papirkurv} other {Flytt # til papirkurv}} ({size})", "len": "short" },
  "reason.blurry": { "t": "Uskarpt ({pct})", "len": "short" }
}
```

### 5.3 Nøkkelkatalog (omfang, ca. 160 nøkler i MVP)

| Område | Nøkler (ca.) | Eksempler (nb → en) |
|---|---|---|
| `onb.*` | 22 | «Kom i gang» → «Get started»; «Fant {count} eksakte duplikater» → «Found {count} exact duplicates» |
| `home.*` | 18 | «{size} kan ryddes» → «{size} can be cleaned up» |
| `review.*` | 24 | «Velg alle» → «Select all» |
| `group.*` | 16 | «Behold dette, flytt {count} til papirkurv» → «Keep this one, move {count} to trash» |
| `swipe.*` | 16 | «Hopp over» → «Skip» |
| `trash.*` | 22 | «Plassen frigjøres først når du tømmer papirkurven.» → «Space is freed only when you empty the trash.» |
| `reason.*` | 14 | se 5.4 |
| `a11y.*` | 18 | «Sveip opp for å flytte til papirkurv» → «Swipe up to move to trash» |
| `err.*`, `perm.*`, `settings.*` | 20 | |
| `consequence.*` | 16 | En nøkkel per `ConsequenceCode` og feilkode fra TRYGGHET (`AckRequired`, `PlanStale`, `PurgeStale`, `NotUndoable`, `AlreadyPurged`, `UnknownAsset`) |

### 5.4 Begrunnelseskoder (kjerne) → tekst (UI)

Kjernen eier kodene (PLAN-KJERNE del «Kjerneplanen leverer», 12 koder). Nøkkel = `snake_case` av enum-navnet. «Samme bilde i lavere oppløsning/mer komprimert/beskåret» skilles ikke i MVP (ingen egne koder).

| Kode | nb | en | Params |
|---|---|---|---|
| `exact_copy` | Identisk kopi | Identical copy | - |
| `visual_duplicate` | Lignende bilde | Similar photo | - |
| `higher_resolution` | Høyere oppløsning | Higher resolution | - |
| `less_compressed` | Mindre komprimert | Less compressed | - |
| `sharper` | Skarpere | Sharper | - |
| `older` | Eldst | Oldest | - |
| `blurry` | Uskarpt ({pct}) | Blurry ({pct}) | `confidence` |
| `screenshot` | Skjermbilde | Screenshot | `signals` (bitmaske) |
| `shot_platform_flag` | I albumet «Skjermbilder» | In the Screenshots album | - |
| `shot_screen_size` | Skjermstørrelse | Screen-sized image | - |
| `shot_no_camera_exif` | Ingen kamerainfo | No camera info | - |
| `shot_png` | PNG-format | PNG format | -

Merker vises som separate chips (maks 2 per kort), aldri sammensatt setning, slik at grammatikk ikke avhenger av språk. Ukjent kode: vis nøytral «Forslag» og logg i test (lint feiler på kode uten nøkkel).

### 5.5 Tekstlengde (+30 %) og layout

- Pseudo-lokale: `gen-i18n` lager `pseudo` fra nb: hver streng forlenges 30 % (utfyll med «·»), omsluttes av `[ ]`, og aksenttegn brukes på noen bokstaver (æ, ø, å, ü) for å fange skrift- og kodingsfeil. Brukes i alle UI-skjermbildetester.
- Layoutregler: ingen faste bredder på tekst, tekst bryter til 2+ linjer, knapper vokser i høyde. Merker og faner (`len: short`) kan avkortes med ellipsis **bare** hvis full tekst er tilgjengelig for skjermleser og i lang trykk-hjelp.
- Bunnlinjen i Gjennomgang og de tre knappene i Swipe stables vertikalt ved lang tekst eller stor skrift.
- Lint (CI `i18n`): nøkkelparitet nb/en, plassholderparitet, flertallsformer, ingen hardkodet tekst i `app/` (regex på strenglitteraler i SwiftUI `Text("…")`), `en` ≤ 1,3 × lengden av nb for `len: short`.

### 5.6 Nynorsk (utsatt til etter MVP)

Avklart 2026-11-11: nn er ikke MVP-krav. MVP = nb + en. Nøkkelstruktur og nøkkelvis fallback til nb beholdes, slik at `nn.json` kan legges til etter MVP uten kodeendring.

## 6. Tilgjengelighet (WCAG 2.2 AA)

### 6.1 Krav

| Område | Krav | Test |
|---|---|---|
| Kontrast | Tekst ≥ 4,5:1 (stor ≥ 3:1), ikoner og kontrollkanter ≥ 3:1, lys og mørk modus. Gjelder også overlegg på bilder (bruk skrimgradient eller bakgrunnsplate) | Enhetstest som beregner kontrast fra tokens + automatisk revisjon |
| Trykkflater | ≥ 44 pt (iOS) | Revisjon |
| Tekststørrelse | iOS Dynamic Type til AX5 uten avkutting eller tap av funksjon | Skjermbildetest × skala |
| Reflow | Brukbart på 320 pt bredde, stående og liggende | Skjermbildetest |
| Ikke bare farge | Handling, valg, «Beste», feil og overlegg har ikon + tekst | Manuell + revisjon |
| Gester | Alle gester har knapp- eller handlingsalternativ (WCAG 2.5.1). Ingen presisjonsdra | UI-test (3.5) |
| Bevegelse | Respekterer «Reduser bevegelse»; ingen blinking > 3/s | Test med flagg |
| Fokus og rekkefølge | Logisk leserekkefølge, synlig fokus, fokus flyttes til neste kort etter handling | Manuell skript |
| Statusmeldinger | Handling, angre, antall valgte og skannframdrift annonseres uten fokusflytting (WCAG 4.1.3) | Manuell + UI-test |
| Navn/rolle/verdi | Alle kontroller har etikett; valgt-tilstand eksponeres | Revisjon |
| Tidsbegrensning | Ingen auto-forsvinnende angre. «Angre» ligger i UI til neste handling | UI-test |

### 6.2 Skjermleser (VoiceOver)

- Bilder har ingen innholdsbeskrivelse (ingen ML i MVP). Etikett er metadata: «Bilde 3 av 12. Tatt 14. juli 2025. 3,2 MB. Beste i gruppen. Uskarpt 82 %.» Nøkler i `a11y.photo.*`.
- Swipe-kort: egendefinerte handlinger «Behold», «Flytt til papirkurv», «Hopp over», «Angre» (iOS `accessibilityCustomActions`). Gest-overlegg skjules fra skjermleser.
- Gruppevisning: liste med ett element per bilde og handlingene «Gjør til beste», «Zoom». Karusellen er visuell.
- Gjennomgang: valgknapp per rad med tilstand «valgt / ikke valgt». Bunnlinjen annonserer ny sum ved endring (med 300 ms demping).
- Ring på Hjem er ett element med full tekst.
- Tøm-flyt: tydelig advarsel i fokusrekkefølgen, destruktiv knapp sist.

### 6.3 Øvrig

- Ekstern tastatur/Switch Control/Full Keyboard Access: piltaster = swipe-handlinger i Swipe, mellomrom = velg, Enter = åpne. Ingen nye hurtigtaster utenfor Swipe og Gjennomgang.
- Haptikk og lyd kan slås av og har ingen informasjon som ikke også finnes visuelt og i tekst.

## 7. Ytelse (UI-andel av budsjettet)

| Mål | Budsjett | Måling |
|---|---|---|
| Slipp av kort → neste kort på plass (p95) | < 100 ms | `os_signpost`, UI-test på referanseenhet |
| Trykk «Angre» → tilstand synlig | < 100 ms | Samme |
| Bunnlinjesum ved valg | < 16 ms (én ramme) | UI-test |
| Kald oppstart til første kort/Hjem | < 1,5 s | Samme (ARKITEKTUR del 4) |
| Rutenett scroll | ingen tapte rammer ved 60 Hz på referanseenhet | Profilering |
| Rask skann i onboarding | Første tall ≤ 5 s, ferdig ≤ 30 s ved 10 000 bilder | Referanseenhet |
| Minne (bildekø) | Maks 3 forhåndslastede bilder i skjermstørrelse + 2 bak | Profilering |

Tiltak: forhåndslast og dekod neste kort på bakgrunnstråd, optimistisk oppdatering, kjernekall via seriell kø, miniatyrer fra plattformens cache, ingen layoutberegning i dra-løkken.

## 8. Testplan

| Nivå | Hva | Verktøy | Kjøres |
|---|---|---|---|
| Reducer-tester | Alle `ViewState`-overganger i 4 (rene funksjoner) | Swift Testing / JUnit | Hver PR (`app/`) |
| i18n | Nøkkelparitet, plassholdere, flertall, hardkodet tekst, lengdegrense `short`, ReasonCode uten nøkkel | `tools/gen-i18n --check` | Hver PR (`i18n/`, `app/`) |
| Funksjon (UI) | Swipe og knapp gir lik batch (`action_log` lik). Angre gjenoppretter. Keeper kan ikke trashes. Onboarding-flyt inkl. avslått tilgang, begrenset tilgang, 0 treff, avbrutt | XCUITest, Compose UI Test + fake `AssetSource` fra `fjell-cli` | Hver PR (`app/`) |
| Skjermbildetest | PR: hver skjerm × {nb, pseudo} × {lys, mørk} × {standard, AX5 / 2,0} × 320 pt uten avkutting. Full matrise (også en, liten skrift, stor telefon) i `nightly` | swift-snapshot-testing, Paparazzi/Roborazzi | Hver PR (`app/`) |
| Tilgjengelighetsrevisjon | 0 brudd i automatisk revisjon: `performAccessibilityAudit` (iOS 17+). Kontrastenhetstest for tokens | Samme | Hver PR (`app/`) |
| Manuell skjermleser | Skript per skjerm i VoiceOver: fullfør onboarding, gå gjennom en gruppe, swipe 10 kort, tøm papirkurv. Dokumentert avkrysning | Manuell, referanseenhet | Før hver beta |
| Ytelse | Swipe/angre p95 < 100 ms, kald oppstart, rask skann-tider | Signpost/Trace + UI-test | `nightly` på referanseenhet |
| Brukbarhet | 5 deltakere, onboarding fullført ≤ 60 s uten hjelp; forstår «Flytt til papirkurv» vs «Tøm» (spørsmål: «Er plassen frigjort nå?») | Observasjon | Før beta, og etter endring i tekst for 3.1/3.6 |
| Nøytral kontroll | Appen skriver aldri til originaler (UI-testen sjekker sjekksum av testbibliotek etter hele flyten) | Fixture-sjekksum | Hver PR (`app/`) |

Akseptkriterier for MVP-UX:
1. Onboarding gir resultat ≤ 60 s på referanseenhet med 10 000 bilder (eller foreløpig resultat etter 45 s).
2. Alle gester har knappalternativ og skjermleserhandling, bevist av UI-test.
3. 0 brudd i a11y-revisjon i lys/mørk, nb/en, standard og AX5.
4. Ingen avkutting i pseudo-lokale på noen skjerm.
5. Handling p95 < 100 ms på referanseenhet.
6. Ingen tekst forteller at plass er frigjort før «Tøm» er utført.

## 9. Leveranseplan (én fase = én gren = én PR)

| Fase | Innhold | Avhenger av | Akseptkriterium |
|---|---|---|---|
| UX-0 | `tools/gen-i18n` (nb/en/pseudo → xcstrings), lint, nøkkelkatalog 5.3 | Repo-skjelett | `just i18n` grønn, paritet, pseudo genereres |
| UX-1 | Design tokens, kontrasttest, komponentbibliotek (kort, merke, bunnlinje, ring), `CoreBridge`-skall mot fake kjerne | UX-0, `api`-trait | Skjermbildetester for komponenter, kontrast OK |
| UX-2 | Onboarding + Hjem | UX-1, rask skann i kjerne | Kriterium 1, flyt-tester |
| UX-3 | Gjennomgang + Gruppevisning | UX-2, `decide_many`, `set_keeper` | Batch-tester, a11y-revisjon |
| UX-4 | Swipe-modus + knapper + skjermleserhandlinger | UX-3 | Kriterium 2 og 5 |
| UX-5 | Papirkurv, Tøm-flyt, utløp, historikk | UX-3, `purge`, plattformadapter | Kriterium 6, tømmeflyt på iPhone |
| UX-6 | Hardening: AX5, ytelse, manuell skjermleser, brukbarhetstest | Alle | Alle akseptkriterier |

Tokenøkonomi for agenter: leser bare del 3 (skjermen), del 4 (tilstand) og `CoreBridge`-filen for fasen. Nøkkelkatalogen leses ikke, men sjekkes av lint. Skjermbildetester er fasit for utseende, ikke prosa.

## 10. Grensesnitt mot andre planer

Antatte planer: Kjerne/API (arkitekt), Plattformadaptere, Bygg/CI, Kalibrering og golden-korpus.

### Krever fra Kjerne/API-planen

| # | Krav | Hvorfor |
|---|---|---|
| K1 | `decide_many(Vec<(CardId, Decision)>) -> BatchId` (én transaksjon, én `batch_id`), internt `plan` + `commit` (< 10 ms også for én swipe). Returnerer `AckRequired(codes)` for beskyttede/masse (TRYGGHET). `Decision` = `Trash`, `Keep` | Bunnlinje «Flytt 87» = én batch = ett angre |
| K2 | `set_keeper(grp, asset)` og at `keep(group)` markerer alle forslag i gruppen avvist | Gruppevisning |
| K3 | Kort (`Card`) inneholder: `asset_id`, `bytes`, `width`, `height`, `created_at`, `reason_codes` (maks 2) med `confidence` og `signals`, `grp_id`, `is_keeper`, relativ skarphet | UI summerer og viser uten ekstra kall |
| K4 | `cards(kind, limit, cursor)` med markør (ikke offset). Sortert konfidens ↓. Hopp-over er UI-lokal | Sveip og paginering |
| K5 | `home_summary`: per kind {antall, bytes}, `reclaimable_bytes`, papirkurv {antall, bytes, expired}, `scan_progress {phase, done, total, found_exact}` | Hjem, onboarding |
| K6 | `start_scan(mode)` med `quick` (kun metadata og BLAKE3 ved lik størrelse, ingen dekoding, hopper over ikke-lokale aktiva) og `full`. Progress-callback med delresultat (`found_exact`) minst hvert sekund. Resultat for `exact` er tilgjengelig via `cards` før `full` er ferdig | Onboarding ≤ 60 s |
| K7 | `trash_summary`: elementer med `trashed_at`, `expires_at`, `expired`, bytes. `freed_total(since)` fra `purge`-rader i `action_log` med `cause` ≠ external | Papirkurv, historikk |
| K8 | Tøm-flyt som i TRYGGHET 2.2: `begin_purge(ids)` → `confirm_purge(intent)` (etter appens ark) → adapter `delete` → `finish_purge(ticket, results)`. Returnerer faktisk antall og bytes slettet (bruker kan avbryte i systemdialogen) | Tøm-flyt |
| K9 | `undo()` returnerer batchens type og antall (for tekst «Angret: 87 bilder») | Meldinger |
| K10 | ReasonCode-settet i 5.4 (avtalt i `model`), alle med parametre som heltall/bitmaske, aldri tekst | i18n |
| K11 | Alle kall < 10 ms og trådsikre for seriell bakgrunnskø. Feil som typed enum (`NoAccess`, `Io`, `Db`, `Cancelled` + TRYGGHETs `AckRequired`, `PlanStale`, `PurgeStale`, `NotUndoable`, `AlreadyPurged`, `UnknownAsset`) med nøkkel `err.<kode>` | Optimistisk UI |

### Krever fra Plattformadapter-planen

| # | Krav |
|---|---|
| A1 | `AssetMeta.is_local` (iCloud / ikke nedlastet) slik at rask skann aldri utløser nedlasting |
| A2 | Tilgangsstatus (`full`, `limited`, `denied`) og hendelse ved endring |
| A3 | Bildelaster for skjermstørrelse (PhotoKit/Coil el.) med prefetch API og avbrudd; leveres til `Design/`-lagene, ikke gjennom kjernen |
| A4 | Slettedialog (`PHPhotoLibrary.performChanges`) som returnerer faktisk slettede ID-er; beskriv oppførsel «Nylig slettet» på iOS |
| A5 | Systemspråk og regionformat tilgjengelig for UI (ikke for kjernen) |

### Krever fra Bygg/CI-planen

| # | Krav |
|---|---|
| C1 | `just i18n` kjører `tools/gen-i18n --check` og generering; jobb `i18n` utløses av `i18n/` og `app/` (finnes) |
| C2 | Skjermbildetester og a11y-revisjon i jobben `ios`; referanseskjermbilder lagres som artefakter, ikke som store filer i git uten LFS-avklaring |
| C3 | `nightly` utvides med UI-ytelsestester (swipe/angre p95, kald oppstart, rask skann) på referanseenhet |
| C4 | Hardkodet-tekst-lint som del av `ios`-jobben, kjørbar lokalt via `just` |

### Krever fra Kalibrering/golden-plan

| # | Krav |
|---|---|
| G1 | `fjell-cli` fixture-bibliotek og fake `AssetSource` som UI-tester kan bruke (inkl. varianter: 0 duplikater, kun uskarpe, 10 000 bilder, avbrutt skann) |
| G2 | Terskelendringer (uskarphet, skjermbilde) endrer ikke kodesettet; endres kodesettet, oppdateres 5.4 først |

### Leverer til andre planer

| # | Leveranse | Til |
|---|---|---|
| L1 | `i18n/nb.json`, `en.json`, og `tools/gen-i18n` med spesifikasjon i 5 | Bygg/CI, alle UI |
| L2 | Ferdig kjørbar mock av `api` i `CoreBridge` (fake kjerne) slik at UI utvikles parallelt med kjernen | Kjerne |
| L3 | Kravlisten K1–K11 og A1–A5 over som bindende ønsker; avvik avklares i `ARKITEKTUR.md` først | Arkitekt |
| L4 | ViewState-kontrakter (del 4) og skjermspesifikasjoner (del 3) som akseptgrunnlag for UI-PR-er | Alle |
| L5 | UI-ytelsesmål og målemetode (del 7) | Bygg/CI nightly |
| L6 | Åpne beslutninger: (1) «Hopp over» i stedet for «album» på høyre sveip, (2) iOS «Nylig slettet»-oppførsel verifiseres, (3) om lokale varsler om utløp skal være med i MVP | Produkteier/arkitekt |
