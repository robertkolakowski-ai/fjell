# Plan: UX, i18n og tilgjengelighet (MVP, Windows-PC)

Status: forslag fra UX-spesialist, revidert 2026-11-11 for Windows-PC (Tauri 2 + Svelte/TypeScript; ARKITEKTUR 11). Bygger på `ARKITEKTUR.md` (bindende) og `PRODUKT.md` (del 3, Hovedskjermer, Onboarding).
Prinsipp: UI er tynt. Kjernen eier logikk, tall og begrunnelseskoder. UI eier tekst, formatering, bilder, bevegelse og tilgjengelighet.

## 1. Omfang

| Skjerm | MVP | Merknad |
|---|---|---|
| Onboarding (60 s) | Ja | Eksakte duplikater først (rask skann), ett trykk |
| Hjem | Ja | Ring «kan frigjøres», kategorikort, rask opprydding |
| Gjennomgang | Ja | Rutenett per kategori, fast bunnlinje |
| Gruppevisning | Ja | Karusell, «Beste», bytt vinner |
| Kortmodus (swipe) | Ja | Piltaster og hurtigtaster, mus-dra valgfritt, knapper alltid |
| Papirkurv + «Tøm» | Ja | Gjenopprett, utløp, tøm med bekreftelse |
| Innstillinger | Minimal | Språk, mapper, papirkurvfrist, lyd (haptikk utgår). Følsomhet per kategori utsettes (terskler er konstanter i kjernen) |
| Sammenligning, Samlinger, regler, album | Nei | Utenfor MVP. Kortmodus «høyre = album» erstattes av «Hopp over», se 3.5 |

Avvik fra PRODUKT.md (begrunnet): «Slett» heter «Flytt til papirkurv» overalt. Lagring frigjøres først ved «Tøm» (ARKITEKTUR #12, #22). Ordet «frigjort» brukes bare om permanent slettet plass; Windows Papirkurv frigjør ikke plass før den tømmes.

## 2. Designregler som gjelder alle skjermer

- Én primærhandling per skjerm, aksentfarge bare der. Bildet er hovedperson, fullskjerm der det er plass.
- Ingen auto-forhåndsvalg bortsett fra: eksakte duplikater og visuelle duplikater (alt unntatt keeper), men aldri beskyttede bilder (TRYGGHET 4.2). Forhåndsmerking er UI-valg (kjernen setter aldri `accepted`); avklart av produkteier 2026-11-11. Uskarpe og skjermbilder er **aldri** forhåndsvalgt, de krever eget valg eller swipe.
- Keeper kan ikke velges for papirkurv i samme gruppe (UI hindrer at alle i en gruppe trashes).
- Alle handlinger er optimistiske: UI oppdateres umiddelbart, kjernekall går på én seriell bakgrunnskø (rekkefølgen bevares), feil gir tilbakerulling og melding.
- Angre er alltid synlig etter en handling (knapp, ikke bare snackbar). Angre = `undo()` = siste batch.
- Ingen poeng, streaks eller trykk-varsler. Oppsummering er rolig og sann.
- Tall formateres i UI (`Intl.NumberFormat`, `Intl.PluralRules`, én liten `formatBytes`-funksjon). Kjernen leverer `u64` bytes og heltall.

## 3. Skjermspesifikasjoner

### 3.1 Onboarding (budsjett 60 s)

| Trinn | Innhold | Budsjett | Kjerne/UI |
|---|---|---|---|
| 1 Velkommen | Én setning, én knapp «Kom i gang». Ingen konto. | ≤ 5 s lesing | UI |
| 2 Mapper | Forklaring («Bildene dine blir på maskinen. Ingenting sendes noe sted.»), så velg mapper (standard: Bilder; eksterne disker og nettverksstasjoner kan legges til). Ingen systemtillatelse. | ≤ 10 s | UI + adapter |
| 3 Rask skann | Fremdrift med levende tall («Fant 312 eksakte duplikater»). Kun metadata + BLAKE3 ved lik størrelse, ingen dekoding. Hopp over skyplassholdere (OneDrive) uten å laste dem ned. | ≤ 30 s for 10 000 bilder, første tall ≤ 5 s | `start_scan(mode=quick)` |
| 4 Resultat | «312 eksakte duplikater (2,1 GB)». Primærknapp «Flytt 312 til papirkurv». Sekundær «Se gjennom først». Ved ≥ 50 bilder eller ≥ 1 GB kreves `MassAck` med advarsel «Fjell kan ikke se om disse er sikkerhetskopiert» før commit (TRYGGHET 4.3). | ≤ 5 s | `cards(exact)` + summary |
| 5 Seier | «312 bilder i papirkurven. 2,1 GB kan frigjøres når du tømmer.» Knapper «Angre» og «Til Hjem». | - | `decide_many` |
| 6 Hjem | Full analyse (dekoding, hash, uskarphet) fortsetter i bakgrunnen med synlig fremdrift | - | `start_scan(mode=full)` |

Regler og kantsaker:
- Hvis rask skann ikke er ferdig etter 45 s: vis «Foreløpig resultat» med det som er funnet, fortsett i bakgrunnen.
- 0 eksakte duplikater: «Ingen eksakte duplikater. Vi leter videre etter like bilder i bakgrunnen.» → Hjem med fremdrift. Ingen kunstig seier.
- Mappe utilgjengelig (ingen lesetilgang, frakoblet disk eller nettverksstasjon): fungerer som vanlig på resten. Banner på Hjem: «Noen mapper kunne ikke leses» + knapp for å se hvilke. Frakoblet disk gir aldri «slettede» bilder.
- Ingen mapper valgt eller ingen lesetilgang: egen tilstand med forklaring og «Velg mapper». Appen krasjer eller tømmes aldri.
- Avbrutt skann (app lukket): gjenopptas via `mod_token`. Onboarding vises ikke på nytt.
- Merk: «Flytt til papirkurv» krever ingen dialog (indekstilstand + flytting til karantene, ARKITEKTUR 20). Bekreftelse kommer først ved «Tøm».

### 3.2 Hjem

| Element | Innhold | Datakilde |
|---|---|---|
| Ring | «X GB kan ryddes» = sum bytes for ventende forslag (uten keepere) + papirkurv-bytes vises separat under: «Y GB i papirkurven» | `home_summary` |
| Kategorikort | Eksakte duplikater, Like bilder, Uskarpe, Skjermbilder. Hver: antall + GB + chevron | `home_summary.per_kind` |
| Rask opprydding | Primærknapp: kortmodus over kort sortert på konfidens (maks 20) | `cards(all, 20)` |
| Skannstatus | Rolig linje: «Analyserer … 4 210 av 10 000» med avbryt/pause. Bare synlig under skann | `scan_progress` |
| Papirkurvlenke | «Papirkurv (312)» | `trash_summary` |
| Banner utløpt | «5 bilder i papirkurven er utløpt. De er ikke slettet.» → Papirkurv | `trash_summary.expired` |

Tomtilstand (alt ryddet): «Ingenting å rydde nå. Vi sier fra når vi finner mer.» uten feiring.
Ringen har alltid tekstekvivalent (tall i klartekst), aldri bare farge/vinkel.

### 3.3 Gjennomgang

- Kategori valgt fra Hjem. Duplikatkategorier viser **grupper som rader** (miniatyrer + «Beste» + antall). Uskarpe og skjermbilder viser **rutenett** med begrunnelsesmerke.
- Fast bunnlinje: «Flytt 87 til papirkurv (1,2 GB)». Deaktivert ved 0 valgt. Antall og bytes summeres av UI fra kortdata (`bytes` per kort), ikke et kjernekall per trykk.
- Valg: klikk på miniatyr eller mellomrom veksler; Shift+klikk velger område. Meny: «Velg alle» (Ctrl+A), «Fjern alle valg». Enter eller dobbeltklikk åpner stor visning (zoom).
- Commit = ett `decide_many`-kall = én batch. `AckRequired`/`MassAck` (beskyttede, ≥ 50 bilder) håndteres med bekreftelsesark før commit; beskyttede bilder ligger i egen seksjon og er aldri i «Velg alle». Etter commit: bunnlinje blir «87 flyttet. Angre».
- Per kort: begrunnelsesmerker (se 6.3). Konfidens vises bare for uskarp/skjermbilde («Uskarpt 82 %»).
- Sortering: konfidens synkende (kjernens `next_cards`). Pagineres med markør (infinite list, 60 kort per side).

### 3.4 Gruppevisning

- Horisontal visning (karusell) med miniatyrstripe under; ←/→ blar. «Beste» markert med ikon + tekst. Valgt kort har tydelig ramme (ikke bare farge).
- Under bildet: kort tabell for aktivt bilde: oppløsning, filstørrelse, dato, relativ skarphet («Skarpere enn de andre»). Gir «zoom for forskjeller» uten Sammenligning-skjerm: dobbeltklikk, +/− og Ctrl+hjul zoomer, zoomnivå beholdes ved sidevending.
- Handlinger: «Behold dette, flytt N til papirkurv» (primær), «Behold alle» (sekundær, `keep` på hele gruppen), «Gjør dette til beste» (bytter keeper, ett trykk, `set_keeper`).
- Eksakte grupper bruker samme visning. Forklaring: «Identiske filer».
- Skjermleser: grupper vises som liste (ikke karusell). Se 8.

### 3.5 Kortmodus (swipe)

| Handling | Hurtigtast | Mus (valgfritt) | Knapp (alltid synlig) | Kjernekall | Merknad |
|---|---|---|---|---|---|
| Behold | ← | dra venstre | «Behold» | `Decision::Keep` | Forslaget markeres avvist, vises ikke igjen |
| Papirkurv | ↑ eller Delete | dra opp | «Papirkurv» | `Decision::Trash` | Hele batch-regelen gjelder (angre) |
| Hopp over | → | dra høyre | «Hopp over» | ingen (UI-lokal, kortet havner bakerst i økten) | Erstatter «album» (utenfor MVP); → blir «Til album» når Samlinger kommer |
| Angre | Ctrl+Z | - | «Angre» | `undo()` | Alltid én tilgjengelig |
| Stor visning | Mellomrom | dobbeltklikk | «Zoom» | ingen | Zoom, ikke Sammenligning |
| Hurtigtasthjelp | F1 | - | «Hurtigtaster» | ingen | Generert fra tabellen i 6.4 |

- Kortstabel viser 1 kort + 2 bak. Forhåndslast 3 (ARKITEKTUR del 4). Visningsbilder hentes av adapteren (diskcache), ikke kjernen.
- Overlegg under mus-dra: farge + ikon + tekst (aldri farge alene). Terskel 35 % av bredde/høyde eller fart; under terskel spretter kortet tilbake. Tastetrykk viser samme overlegg kort. Haptikk utgår.
- Lyd av som standard (innstilling). Respekter Windows «Animasjonseffekter» av (`prefers-reduced-motion`): kryssfade i stedet for flyvende kort.
- Budsjett: tastetrykk eller slipp til neste kort på plass **< 100 ms** (p95). Kjernekall < 10 ms kjøres bak animasjonen; filflytting til karantene skjer etterpå på bakgrunnskø.
- Slutt på kortbunken: rolig oppsummering «Du flyttet 41 bilder til papirkurven og beholdt 12» + «Angre siste» (kan trykkes gjentatte ganger: kjernen har en stabel av batcher, `undo()` tar siste ikke-angrede). Ingen samlet «angre alle» i MVP.
- Skjermleser: handlingene er vanlige knapper i Tab-rekkefølge med `aria-keyshortcuts`; mus-dra-overlegg skjules (`aria-hidden`); resultat annonseres i live-region (8, 6.2).
- Touch (Windows-berøringsskjerm) er valgfritt: dra virker som mus-dra, knappene er store nok; ingen egen touch-design i MVP.

### 3.6 Papirkurv

| Element | Innhold |
|---|---|
| Topp | «312 bilder · 2,1 GB kan frigjøres» (alltid med forklaring: «Plassen frigjøres først når du tømmer papirkurven.») |
| Liste | Gruppert: «Utløpt», «Utløper snart (≤ 7 dager)», øvrige. Rad: miniatyr, dato flyttet, «Utløper om 12 dager» |
| Valg | «Gjenopprett» (valgte), «Gjenopprett alt». Gjenoppretting er `restore(batch)`, umiddelbar, uten bekreftelse |
| Tøm | Knapp «Tøm papirkurv…». Appens ark: antall, GB og valg «Flytt til Windows Papirkurv» (standard) eller «Slett permanent». Sekundær «Avbryt», primær «Fortsett». Permanent gir ekstra trinn: «Dette kan ikke angres», avkrysning og knapp «Slett N permanent» (`PermanentAck`). Volum uten Papirkurv (nettverksstasjon): ark sier det og tilbyr bare permanent, aldri automatisk |
| Utløpt | Merkes «Utløpt», aldri stille slettet. Vis banner på Hjem. Windows-varsel er valgfritt (innstilling, av som standard, spørres ikke i onboarding) |
| Historikk | Én linje nederst: «Du har frigjort 4,8 GB siden mars» basert på permanent slettet (`purged`, modus permanent, `cause` ≠ external) |

Åpent UX-punkt fra arkitekturen («Tøm»): løst slik at appens ark forklarer og utfører (ARKITEKTUR 22), ingen systemdialog. Etter Papirkurv-modus: «Flyttet til Windows Papirkurv. Plassen frigjøres når du tømmer den.» Elementer som ikke kunne flyttes til karantene vises «Merket, ikke flyttet». Antakelse som må verifiseres på referansemaskinen: Papirkurv-oppførsel per volumtype (lokal, ekstern, nettverk); tekst låses etterpå.

## 4. UI-tilstand per skjerm (kontrakt for agenter)

Hver skjerm = én fil med en `ViewState`-type (sealed/enum) og én ren funksjon `reduce(state, event)`. Testes uten UI.

| Skjerm | Tilstander |
|---|---|
| Onboarding | welcome, chooseFolders, noAccess, scanning(found), partial(found), result(count, bytes), nothingFound, done(count, bytes) |
| Hjem | loading, scanning(progress), ready(summary), partialAccess, allClean, error |
| Gjennomgang | loading, list(items, selection), committing, committed(batch), empty, error |
| Gruppe | loading, ready(group, activeIndex, zoom), committed, error |
| Kortmodus | loading, card(top, nextN), dragging(dir, progress), deckEmpty(stats), error |
| Papirkurv | empty, list(sections, selection), confirmPurge(mode), confirmPermanent, purging, purged(mode, bytes), error |

Filstruktur (`desktop/src/`): `features/{onboarding,home,review,group,cards,trash,settings}/` (hver med `State.ts`, `reduce.ts`, `*.svelte`), `design/` (`tokens.css`, komponenter), `bridge/` (`CoreBridge` over Tauri `invoke` + `MockBridge`, mapper kommandotyper til state), `strings/` (generert), `shortcuts.ts` (hurtigtasttabell, 6.4). Ingen forretningslogikk utenfor `bridge/`. Hver fil ≤ ca. 300 linjer.

## 5. i18n (nb, en)

### 5.1 Regler

- Kilde: `i18n/nb.json` (kildespråk), `en.json` (`nn.json` etter MVP). Generert til typede TS-moduler (`strings/nb.ts`, `en.ts`, `pseudo.ts`) av `tools/gen-i18n`. Ingen manuell redigering av genererte filer.
- Ingen tekst i kjernen. Kjernen leverer `ReasonCode` + parametre. UI mapper kode → nøkkel `reason.<kode>`.
- Ingen strengsammenstilling i kode. Hele setninger med navngitte plassholdere: `{count}`, `{size}`, `{days}`. Datoer, tall og størrelser formateres med `Intl` i valgt språk og sendes inn ferdig formatert.
- Flertall med ICU-delmengde: `{count, plural, one {# bilde} other {# bilder}}`. gen-i18n støtter bare `plural`, `select` og enkle plassholdere, og feiler på resten. (nb og en har kategoriene `one` og `other`.)
- Nøkkelnavn: `<område>.<element>.<tilstand>`, små bokstaver, punktum. Eks. `home.ring.caption`, `trash.purge.confirm.title`.
- Hver nøkkel har valgfri `"ctx"` (beskrivelse til oversetter) og `"len": "short"` for knapper, faner og merker (kort plass).
- Bruk språkets egne typografiregler: hard mellomrom før `%` og mellom tall og enhet i nb (`82 %`, `1,2 GB`). Formateres av `Intl`, ikke i JSON.
- Språkvalg: følger Windows-visningsspråk, kan overstyres i Innstillinger (`setting.language`). Språk uten oversettelse (nn etter MVP) faller tilbake nøkkelvis til nb.

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
| `cards.*` | 16 | «Hopp over» → «Skip» |
| `trash.*` | 22 | «Plassen frigjøres først når du tømmer papirkurven.» → «Space is freed only when you empty the trash.» |
| `reason.*` | 14 | se 5.4 |
| `a11y.*` | 18 | «Pil opp flytter til papirkurv» → «Up arrow moves to trash» |
| `err.*`, `folders.*`, `settings.*` | 20 | |
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
- Bunnlinjen i Gjennomgang og de tre knappene i Kortmodus stables vertikalt ved lang tekst eller høy skalering.
- Lint (CI `i18n`): nøkkelparitet nb/en, plassholderparitet, flertallsformer, ingen hardkodet tekst i `desktop/src` (lint på tekstnoder og `aria-label`/`title` med strenglitteral i `.svelte`), `en` ≤ 1,3 × lengden av nb for `len: short`.

### 5.6 Nynorsk (utsatt til etter MVP)

Avklart 2026-11-11: nn er ikke MVP-krav. MVP = nb + en. Nøkkelstruktur og nøkkelvis fallback til nb beholdes, slik at `nn.json` kan legges til etter MVP uten kodeendring.

## 6. Tilgjengelighet (WCAG 2.2 AA)

### 6.1 Krav

| Område | Krav | Test |
|---|---|---|
| Kontrast | Tekst ≥ 4,5:1 (stor ≥ 3:1), ikoner og kontrollkanter ≥ 3:1, lys og mørk modus. Gjelder også overlegg på bilder (bruk skrimgradient eller bakgrunnsplate) | Enhetstest som beregner kontrast fra tokens + automatisk revisjon |
| Klikkflater | ≥ 24×24 CSS px (WCAG 2.5.8); knapper i Kortmodus ≥ 44 px (touch valgfritt) | Revisjon |
| Tekststørrelse og skalering | Windows-skalering og tekststørrelse 100–200 % og nettleserzoom til 200 % uten avkutting eller tap av funksjon; størrelser i `rem` | Skjermbildetest × skala |
| Høykontrast | Windows kontrastmodus (`forced-colors: active`): alle kontroller, fokus, valgt tilstand og kortoverlegg synlige uten egne farger (systemfarger) | Skjermbildetest med emulering + manuell |
| Tastatur | Alt kan gjøres uten mus (WCAG 2.1.1), ingen tastaturfelle, ingen kollisjon med Windows-standard (6.4) | UI-test + manuell |
| Reflow | Brukbart fra 1280×720 ved 200 % skalering og reflow ned til 320 CSS px bredde (WCAG 1.4.10) | Skjermbildetest |
| Ikke bare farge | Handling, valg, «Beste», feil og overlegg har ikon + tekst | Manuell + revisjon |
| Gester og hurtigtaster | Alle hurtigtaster og mus-gester har knappalternativ (WCAG 2.5.1). Mus-dra er valgfritt, ingen presisjonsdra | UI-test (3.5) |
| Bevegelse | Respekterer Windows «Animasjonseffekter» av (`prefers-reduced-motion`); ingen blinking > 3/s | Test med flagg |
| Fokus og rekkefølge | Logisk leserekkefølge, synlig fokus (WCAG 2.4.11), fokus flyttes til neste kort etter handling | Manuell skript |
| Statusmeldinger | Handling, angre, antall valgte og skannframdrift annonseres uten fokusflytting (WCAG 4.1.3) | Manuell + UI-test |
| Navn/rolle/verdi | Alle kontroller har etikett; valgt-tilstand eksponeres | Revisjon |
| Tidsbegrensning | Ingen auto-forsvinnende angre. «Angre» ligger i UI til neste handling | UI-test |

### 6.2 Skjermleser (Windows Narrator og NVDA)

- Bilder har ingen innholdsbeskrivelse (ingen ML i MVP). Etikett er metadata: «Bilde 3 av 12. Tatt 14. juli 2025. 3,2 MB. Beste i gruppen. Uskarpt 82 %.» Nøkler i `a11y.photo.*`.
- Kort: knappene «Behold», «Flytt til papirkurv», «Hopp over», «Angre» i naturlig Tab-rekkefølge med `aria-keyshortcuts`. Mus-dra-overlegg skjules fra skjermleser. Hver handling annonseres i én felles `aria-live="polite"`-region («Flyttet til papirkurv. 12 igjen.»). Piltaster krever fokusmodus (NVDA: NVDA+Mellomrom; Narrator: Caps Lock+Mellomrom); knappene fungerer alltid.
- Gruppevisning: liste med ett element per bilde og handlingene «Gjør til beste», «Zoom». Karusellen er visuell.
- Gjennomgang: valgknapp per rad med tilstand «valgt / ikke valgt». Bunnlinjen annonserer ny sum ved endring (med 300 ms demping).
- Ring på Hjem er ett element med full tekst.
- Tøm-flyt: tydelig advarsel i fokusrekkefølgen, destruktiv knapp sist.

### 6.3 Øvrig

- Synlig knappetekst er også tilgjengelig navn (WCAG 2.5.3), slik at Windows Talestyring fungerer.
- Lyd kan slås av og har ingen informasjon som ikke også finnes visuelt og i tekst. Haptikk utgår.

### 6.4 Hurtigtaster (én tabell i `shortcuts.ts`, vises i F1-dialogen)

| Kontekst | Tast | Handling |
|---|---|---|
| Overalt | Ctrl+Z | Angre siste batch |
| Overalt | Esc | Lukk ark, dialog eller stor visning |
| Overalt | F1 | Vis hurtigtaster |
| Kortmodus | ← / ↑ eller Delete / → | Behold / Papirkurv / Hopp over |
| Kortmodus og stor visning | Mellomrom | Zoom |
| Gjennomgang | Mellomrom / Enter | Velg / åpne |
| Gjennomgang | Ctrl+A / Shift+klikk | Velg alle / område |
| Gjennomgang | Delete | Flytt valgte til papirkurv (bekreftelse ved massesletting, som knappen) |
| Gruppevisning | ← / → | Forrige / neste bilde |
| Gruppevisning | 1–9 | Gjør bilde nr. N til beste (bare med fokus i gruppeområdet; knappen «Gjør dette til beste» finnes alltid) |

Regler: ingen Alt-, Win- eller Ctrl+W/Tab/F4-kombinasjoner (reservert av Windows og WebView2; Alt+←/→ er «tilbake/frem»); ingen enkelttegn-taster (WCAG 2.1.4); Backspace brukes ikke; piler, Delete og Mellomrom er bare aktive når kortområdet eller listen har fokus, aldri i tekstfelt. Endring av tabellen krever oppdatering her først. Test: `shortcuts.test.ts` sjekker at tabellen, F1-dialogen og `aria-keyshortcuts` er like.

## 7. Ytelse (UI-andel av budsjettet)

| Mål | Budsjett | Måling |
|---|---|---|
| Tastetrykk eller slipp av kort → neste kort på plass (p95) | < 100 ms | `performance.measure`, Playwright mot Tauri-appen på referansemaskinen |
| Trykk «Angre» → tilstand synlig | < 100 ms | Samme |
| Bunnlinjesum ved valg | < 16 ms (én ramme) | UI-test |
| Kald oppstart til første kort/Hjem | < 1,5 s | Samme (ARKITEKTUR del 4) |
| Rutenett scroll | ingen tapte rammer ved 60 Hz på referansemaskinen (virtualisert liste) | Profilering |
| Rask skann i onboarding | Første tall ≤ 5 s, ferdig ≤ 30 s ved 10 000 bilder på lokal SSD | Referansemaskin |
| Minne (bildekø) | Maks 3 forhåndslastede bilder i skjermstørrelse + 2 bak | Profilering |

Tiltak: forhåndslast og dekod neste kort (`img.decode()`), optimistisk oppdatering, kjernekall og filflytting via seriell kø, visningsbilder fra diskcache (ARKITEKTUR 21), ingen layoutberegning i dra-løkken.

## 8. Testplan

| Nivå | Hva | Verktøy | Kjøres |
|---|---|---|---|
| Reducer-tester | Alle `ViewState`-overganger i 4 (rene funksjoner) | Vitest | Hver PR (`desktop/`) |
| i18n | Nøkkelparitet, plassholdere, flertall, hardkodet tekst, lengdegrense `short`, ReasonCode uten nøkkel | `tools/gen-i18n --check` | Hver PR (`i18n/`, `desktop/`) |
| Funksjon (UI) | Tastatur, mus og knapp gir lik batch (`action_log` lik). Angre gjenoppretter. Keeper kan ikke trashes. Onboarding-flyt inkl. ingen mapper, utilgjengelig mappe, 0 treff, avbrutt | Playwright mot `MockBridge` (varianter G1) + utvalgte ende-til-ende mot Tauri-appen (`tauri-driver`, WebView2) | Hver PR (`desktop/`) |
| Skjermbildetest | PR: hver skjerm × {nb, pseudo} × {lys, mørk} × {skalering 100 %, 200 %} × {1280×720, 1920×1080} uten avkutting; høykontrast (`forced-colors`) for nøkkelskjermer. Full matrise (også en, 150 %, 1366×768) i `nightly` | Playwright `toHaveScreenshot` | Hver PR (`desktop/`) |
| Tilgjengelighetsrevisjon | 0 brudd i axe (WCAG 2.2 AA) i lys/mørk × bred/smal. Kontrastenhetstest for tokens. `svelte-check` a11y-advarsler er feil | `@axe-core/playwright`, `svelte-check` | Hver PR (`desktop/`) |
| Manuell skjermleser | Skript per skjerm i Narrator og NVDA (Windows, WebView2): fullfør onboarding, gå gjennom en gruppe, ta 10 kort, tøm papirkurv; også ved 200 % og høykontrast. Dokumentert avkrysning | Manuell, referansemaskin | Før hver beta |
| Ytelse | Kortbeslutning/angre p95 < 100 ms, kald oppstart, rask skann-tider | `performance.measure` + Playwright/`tauri-driver` | `nightly` på referansemaskinen |
| Brukbarhet | 5 deltakere, onboarding fullført ≤ 60 s uten hjelp; forstår «Flytt til papirkurv» vs «Tøm» (spørsmål: «Er plassen frigjort nå?») | Observasjon | Før beta, og etter endring i tekst for 3.1/3.6 |
| Nøytral kontroll | Innholdet i originalene er uendret (sjekksum av testbibliotek etter hele flyten, inkl. karantene og angre; filene er tilbake på opprinnelig sti) | Fixture-sjekksum | Hver PR (`desktop/`) |

Akseptkriterier for MVP-UX:
1. Onboarding gir resultat ≤ 60 s på referansemaskinen med 10 000 bilder (eller foreløpig resultat etter 45 s).
2. Alle hurtigtaster og mus-gester har knappalternativ og skjermleserstøtte, og alt kan gjøres med tastatur, bevist av UI-test.
3. 0 brudd i a11y-revisjon i lys/mørk, nb/en, skalering 100 og 200 % og høykontrast.
4. Ingen avkutting i pseudo-lokale på noen skjerm.
5. Handling p95 < 100 ms på referansemaskinen.
6. Ingen tekst forteller at plass er frigjort før «Tøm» er utført.

## 9. Leveranseplan (én fase = én gren = én PR)

| Fase | Innhold | Avhenger av | Akseptkriterium |
|---|---|---|---|
| UX-0 | `tools/gen-i18n` (nb/en/pseudo → TS-moduler), lint, nøkkelkatalog 5.3 | Repo-skjelett | `just i18n` grønn, paritet, pseudo genereres |
| UX-1 | Design tokens (`tokens.css`), kontrasttest, komponentbibliotek (kort, merke, bunnlinje, ring, live-region), `CoreBridge` (Tauri `invoke`) + `MockBridge` | UX-0, `api`-trait | Skjermbildetester for komponenter, kontrast OK |
| UX-2 | Onboarding + Hjem | UX-1, rask skann i kjerne | Kriterium 1, flyt-tester |
| UX-3 | Gjennomgang + Gruppevisning | UX-2, `decide_many`, `set_keeper` | Batch-tester, a11y-revisjon |
| UX-4 | Kortmodus + hurtigtaster (6.4) + knapper + skjermleserstøtte | UX-3 | Kriterium 2 og 5 |
| UX-5 | Papirkurv, Tøm-flyt (Papirkurv/permanent), utløp, historikk | UX-3, `purge`, desktop-adapter (karantene, `delete`) | Kriterium 6, tømmeflyt på Windows |
| UX-6 | Hardening: skalering 200 %, høykontrast, ytelse, manuell Narrator/NVDA, brukbarhetstest | Alle | Alle akseptkriterier |

Tokenøkonomi for agenter: leser bare del 3 (skjermen), del 4 (tilstand) og `CoreBridge`-filen for fasen. Nøkkelkatalogen leses ikke, men sjekkes av lint. Skjermbildetester er fasit for utseende, ikke prosa.

## 10. Grensesnitt mot andre planer

Antatte planer: Kjerne/API (arkitekt), Plattformadaptere, Bygg/CI, Kalibrering og golden-korpus.

### Krever fra Kjerne/API-planen

| # | Krav | Hvorfor |
|---|---|---|
| K1 | `decide_many(Vec<(CardId, Decision)>) -> BatchId` (én transaksjon, én `batch_id`), internt `plan` + `commit` (< 10 ms også for én kortbeslutning). Returnerer `AckRequired(codes)` for beskyttede/masse (TRYGGHET). `Decision` = `Trash`, `Keep` | Bunnlinje «Flytt 87» = én batch = ett angre |
| K2 | `set_keeper(grp, asset)` og at `keep(group)` markerer alle forslag i gruppen avvist | Gruppevisning |
| K3 | Kort (`Card`) inneholder: `asset_id`, `bytes`, `width`, `height`, `created_at`, `reason_codes` (maks 2) med `confidence` og `signals`, `grp_id`, `is_keeper`, relativ skarphet | UI summerer og viser uten ekstra kall |
| K4 | `cards(kind, limit, cursor)` med markør (ikke offset). Sortert konfidens ↓. Hopp-over er UI-lokal | Kortmodus og paginering |
| K5 | `home_summary`: per kind {antall, bytes}, `reclaimable_bytes`, papirkurv {antall, bytes, expired}, `scan_progress {phase, done, total, found_exact}` | Hjem, onboarding |
| K6 | `start_scan(mode)` med `quick` (kun metadata og BLAKE3 ved lik størrelse, ingen dekoding, hopper over ikke-lokale aktiva) og `full`. Progress-callback med delresultat (`found_exact`) minst hvert sekund. Resultat for `exact` er tilgjengelig via `cards` før `full` er ferdig | Onboarding ≤ 60 s |
| K7 | `trash_summary`: elementer med `trashed_at`, `expires_at`, `expired`, bytes. `freed_total(since)` fra `purge`-rader i `action_log` med `cause` ≠ external | Papirkurv, historikk |
| K8 | Tøm-flyt som i TRYGGHET 2.2: `begin_purge(ids)` → `confirm_purge(intent, mode)` (etter appens ark; permanent krever `PermanentAck`) → Rust-kommando `delete(ids, mode)` → `finish_purge(ticket, results)`. Returnerer faktisk antall og bytes slettet, og `Unsupported` per id | Tøm-flyt |
| K9 | `undo()` returnerer batchens type og antall (for tekst «Angret: 87 bilder») | Meldinger |
| K10 | ReasonCode-settet i 5.4 (avtalt i `model`), alle med parametre som heltall/bitmaske, aldri tekst | i18n |
| K11 | Alle kall < 10 ms og trådsikre for seriell bakgrunnskø. Feil som typed enum (`NoAccess`, `Io`, `Db`, `Cancelled` + TRYGGHETs `AckRequired`, `PlanStale`, `PurgeStale`, `NotUndoable`, `AlreadyPurged`, `UnknownAsset`) med nøkkel `err.<kode>` | Optimistisk UI |

### Krever fra Plattformadapter-planen

| # | Krav |
|---|---|
| A1 | `AssetMeta.is_local` (skyplassholder/OneDrive, ikke nedlastet) slik at rask skann aldri utløser nedlasting |
| A2 | Rotstatus per valgt mappe (`ok`, `denied`, `offline`) og hendelse ved endring (disk koblet fra/til) |
| A3 | Visningsbilder (skjermstørrelse) fra adapteren med prefetch og avbrudd, diskcache (ARKITEKTUR 21); leveres gjennom `bridge/`, ikke kjernen |
| A4 | Karantene (`stage`/`unstage`) og `delete(ids, mode)` med per-id-utfall (ARKITEKTUR 20, 22) |
| A5 | Windows-visningsspråk og regionformat tilgjengelig for UI (`Intl`, ikke kjernen) |

### Krever fra Bygg/CI-planen

| # | Krav |
|---|---|
| C1 | `just i18n` kjører `tools/gen-i18n --check` og generering; jobb `i18n` utløses av `i18n/` og `desktop/` |
| C2 | Skjermbildetester og axe-revisjon i jobben `desktop` (`windows-latest`); referanseskjermbilder lagres som artefakter, ikke som store filer i git uten LFS-avklaring |
| C3 | `nightly` utvides med UI-ytelsestester (kortbeslutning/angre p95, kald oppstart, rask skann) på referansemaskinen |
| C4 | Hardkodet-tekst-lint som del av `desktop`-jobben, kjørbar lokalt via `just` |

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
| L6 | Åpne beslutninger: (1) «Hopp over» i stedet for «album» på høyre piltast, (2) Windows Papirkurv-oppførsel per volumtype verifiseres, (3) om Windows-varsler om utløp skal være med i MVP | Produkteier/arkitekt |
