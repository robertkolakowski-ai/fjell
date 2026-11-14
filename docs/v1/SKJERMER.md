# V1-skjermer: kontrakt

Status: kontraktfasen er lagt. Bridgen (`CoreBridge`, `TauriBridge`, `MockBridge`), ruter, navigasjon, forslagstypene serier og regler, monteringspunkter og `ProtectToggle` finnes. Selve skjermene bygges i egne faser (en gren og en PR hver) på toppen av dette. Kjernen og kommandoflaten er beskrevet i `docs/v1/V1-PLAN.md`; komponent-API og tokens i `docs/design/LOFT.md`.

Bindende regler (`CLAUDE.md`): ingen hard sletting, alt som slettes kan angres, appen foreslår og dømmer ikke, ingen automatisk sletting, on-device og ingen nettverkskall, handlinger under 100 ms (p95), WCAG AA (Narrator og NVDA, skalering 100 til 200 %, høykontrast), alle hurtigtaster og gester har knappealternativ, nb og en (test med pseudo), kjernen sender koder og aldri tekst.

## 1. Skjermkart og ruter

| Rute | Skjerm | Fil | Nav | Merknad |
|---|---|---|---|---|
| `#/` | Hjem + oversikt | `screens/Home.svelte` + `screens/DashboardPanel.svelte` | Hjem | Panelet er montert under kategorikortene. |
| `#/review[/<type>]` | Gjennomgang | `screens/Review.svelte` | Gjennomgang | Seks typer: de fire fra MVP + `series` + `rule`. |
| `#/group/:id` | Gruppe (eksakt/visuell) | `screens/Group.svelte` | Gjennomgang | Uendret. |
| `#/series/:id` | Sammenlign serie | `screens/SeriesCompare.svelte` | Gjennomgang | Ny underside. Serieradene i Gjennomgang lenker hit. |
| `#/cards[/<type>]` | Kortmodus | `screens/Cards.svelte` + `screens/CardExplain.svelte` | Gjennomgang | `CardExplain` er montert på kort av typen serie og regel. |
| `#/people` | Personer | `screens/People.svelte` | Personer (ny) | Toppnivå. |
| `#/rules` | Regler | `screens/Rules.svelte` | Regler (ny) | Toppnivå. |
| `#/trash` | Papirkurv | `screens/Trash.svelte` | Papirkurv | Uendret. |
| `#/settings` | Innstillinger | `screens/Settings.svelte` + `SettingsSensitivity`, `SettingsFaces`, `SettingsProtections` | Innstillinger | Tre nye seksjoner montert. |
| `#/onboarding` | Kom i gang | `screens/Onboarding.svelte` | (ingen) | Uendret. |

Rekkefølge i navigasjonen: Hjem, Gjennomgang, Personer, Regler, Papirkurv, Innstillinger (`NAV_ROUTES` i `router.ts`). `navFor(series)` er `review`. Ugyldig `series/:id` gir Hjem, som alle andre ruter. F1-hjelp har rader for `people`, `rules` og `series` (tomme til skjermene har egne taster).

## 2. Forslagstyper

`KindCode` og `KIND_CODES` i `bridge/types.ts` har nå seks verdier (`series`, `rule` kom til). `MVP_KIND_CODES` er de fire gamle. `GROUPED_KINDS` (`exact_dup`, `visual_dup`, `series`) er typene der kortene har `group` og `keeper`. `V1KindCode` er fjernet; `types-v1.ts` bruker `KindCode`.

- `Card.detail: Detail` følger kjernen (`none`, `percent`, `count`, `position {index, of}`, `rule {id}`, `person {id}`).
- `ReasonCode` har de seks nye kodene (`SeriesMember`, `KeeperBestOverall`, `KeeperEyesOpen`, `ClosedEyes`, `Rule`, `ProtectedPerson`). Teksten bygges i `screens/reason.ts` (`cardReason`): serieplass fra `detail.position`, regelnavn fra regellisten (`loadRuleNames`; ukjent navn gir «uten navn»), prosent fra `confidence`.
- Home: kategorikortene viser alle seks typer (`home.cat.series`, `home.cat.rule`). Review: serier er grupperader med lenke «Sammenlign serien»; regler er rutenett med «Regel: navn». Hjelpetekst per type: `review.kind.series.help`, `review.kind.rule.help`.
- Kortmodus bruker alle seks typer når `kind` er null. Beskyttede kort (alle `protectFlags` ulik 0) er aldri i bunken.

## 3. Bridge

Alt V1 ligger i `CoreBridge` (ingen egen `v1.ts`, ingen direkte `invoke` i skjermer). `decide(ids, decision, confirmed?)` tar konsekvenskoder brukeren har bekreftet etter `ack_required` (`protected_by_scope`).

| Område | Metoder |
|---|---|
| Beskyttelse | `protect(scope, on)`, `listProtections()` |
| Regler | `listRules`, `saveRule`, `previewRule`, `toggleRule`, `deleteRule`, `restoreRule` |
| Personer | `listPeople`, `namePerson`, `mergePeople`, `unmergePerson`, `protectPerson`, `getFaceIndex`, `setFaceIndex`, `wipeFaceIndex` |
| Oversikt | `getDashboard`, `compareSeries`, `getSuggestionSettings`, `setSuggestionSettings`, `getThresholds` |

Angre-par: `protect(s, true)` og `protect(s, false)`; `deleteRule` og `restoreRule`; `mergePeople` og `unmergePerson`. Eneste som ikke kan angres er `wipeFaceIndex` (avledede ansiktsdata og personer, aldri bilder): bekreft først, og vis varselet `people.index.wipe.confirm`.

Feilkoder (`BridgeError.code`, tekst via `error.<kode>`): `empty_folder`, `unknown_target`, `empty_rule`, `empty_name`, `name_too_long`, `invalid_condition`, `rule_not_found`, `unknown_person`, `not_merged`, `unknown_series`, i tillegg til MVP-kodene.

### MockBridge (skjermer og e2e kjører mot den)

Troverdig innebygd data og samme semantikk som kjernen (`bridge/mock-v1.ts`, test i `bridge/v1.test.ts`):

- **Data**: seks kort per type (serie 41 og 42 med tre kort hver, serie 42 har et kort med lukkede øyne; regel 1 og 2 med tre kort hver), fire personer (to med navn, bare synlige når ansiktsindeksen er på), to album, to regler, seks måneder dashboard-tall.
- **Ansiktsindeks av som standard**: `listPeople` gir tom liste. `setFaceIndex(true)` viser personene. `wipeFaceIndex` nuller personer, gjør personbeskyttelser om til bildebeskyttelse (ingen mister vernet) og returnerer antall ansikter; ny skanning med indeksen på bygger personene på nytt.
- **Regler foreslår bare**: `saveRule` lager kort (aldri papirkurv), `previewRule` teller uten å lagre, beskyttede bilder gir aldri treff (og treffet kommer tilbake når beskyttelsen fjernes), av/på og sletting skjuler regelens kort, `restoreRule` henter dem tilbake.
- **Beskyttede bilder** får `protectFlags` bit 16 (`SCOPE_FLAG`), så skjermene aldri forhåndsvelger dem. Papirkurv uten bekreftelse gir `ack_required` med `detail = "protected_by_scope"`; behold er alltid trygt.
- **Dashboard**: `freedBytes` er 0 til noe er tømt; `freedByMonth` fylles bare ved «Tøm»; `nextStep` er `review`, `empty_trash` eller `nothing`.
- **Terskler** følger sensitiviteten (V1-PLAN del 7). `setSuggestionSettings` validerer (`bad_request`).

## 4. Delt komponent

`lib/components/ProtectToggle.svelte` (eksportert fra `lib`): props `scope`, `protected`, `onchange(next)`, `label`, `disabled?`. Knapp med `aria-pressed`, ikon (skjold/hake), synlig tekst og tilstandsord «På/Av». Navnet (`label`) endres ikke med tilstanden; tilstandsordet er bare visuelt. Kalleren eier tilstanden: kall `protect(scope, next)`, oppdater `protected` når kjernen har svart, og tilby angre. Testet i `lib/components/protect-toggle.test.ts` og `e2e/v1-skjermer.spec.ts`; vist i dev-galleriet (`?gallery`).

## 5. Hvem eier hvilke filer

Kontrakten (denne fasen) eier alle delte filer. Grenene fyller bare sin egen fil, og endrer ikke delte filer; trenger en gren noe delt, meldes det til Integrer (samme regel som V1-PLAN del 3).

Delt og låst for skjermgrenene: `bridge/*` (inkl. `types.ts`, `types-v1.ts`, `mock-bridge.ts`, `mock-v1.ts`), `router.ts`, `App.svelte`, `Home.svelte`, `Review.svelte`, `Cards.svelte`, `Settings.svelte`, `screens/helpKeys.ts`, `screens/reason.ts`, `lib/**` (komponenter, tokens, i18n-runtime), `i18n/nb.json`, `i18n/en.json` og de genererte `lib/i18n/strings/*`.

| Gren | Eier (fyll disse) | Bruker |
|---|---|---|
| `dashboard` | `screens/DashboardPanel.svelte` (+ egne filer `dashboard*.ts`, test) | `getDashboard`, `dashboard.*` |
| `series` | `screens/SeriesCompare.svelte`, `screens/CardExplain.svelte` (serie-delen) | `compareSeries`, `setKeeper`, `series.*`, `reason.*` |
| `rules` | `screens/Rules.svelte`, `screens/CardExplain.svelte` (regel-delen, avtales med `series`) | `listRules`, `saveRule`, `previewRule`, `toggleRule`, `deleteRule`, `restoreRule`, `rules.*` |
| `people` | `screens/People.svelte`, `screens/SettingsFaces.svelte` | `listPeople`, `namePerson`, `mergePeople`, `unmergePerson`, `protectPerson`, ansiktsindeks, `people.*` |
| `protect` | `screens/SettingsProtections.svelte`, `ProtectToggle`-bruk i Gruppe, Serie og Personer (via de andre grenene) | `protect`, `listProtections`, `protect.*` |
| `sensitivity` | `screens/SettingsSensitivity.svelte` | `getSuggestionSettings`, `setSuggestionSettings`, `getThresholds`, `settings.sensitivity.*` |

Nye filer i egen skjermmappe er fritt fram (state, flyt, test etter mønsteret `.svelte` + `*State.ts` + `*Flow.ts` + vitest). Nye i18n-nøkler samles av Integrer (de ligger i delte filer); en gren som trenger nøkler leverer dem som liste i PR-beskrivelsen, eller legger dem inn i en egen commit som Integrer slår sammen. Plassholderteksten `placeholder.todo` fjernes når skjermen er på plass.

`CardExplain` har to eiere (serie og regel). Foreslått avtale: `series` lager skjelettet (`switch` på `reason`), `rules` legger til regelgrenen etter at `series` er sammenslått.

## 6. Åpne UX-spørsmål

1. **Personer i navigasjonen når indeksen er av.** Navigasjonen viser alltid «Personer». Med ansiktsindeksen av (standard) er skjermen bare en forklaring og en «Slå på»-knapp. Alternativ: skjul valget til indeksen er på. Anbefaling: behold synlig, så funksjonen kan oppdages, men aldri slå på av seg selv.
2. **Beskyttelsesbryter: hvor.** `ProtectToggle` passer i Gruppe (per bilde), Serie (per bilde), Personer (per person) og i Innstillinger («Beskyttelser»). Skal Gjennomgang ha den på hver flis? Kan gjøre flisene travle; foreslått: bare i detaljvisning.
3. **Hva skjer med beskyttede kort i Kortmodus.** I dag hoppes de over (aldri i bunken). Alternativ: vis dem med merket «Beskyttet» og bare knappene Behold og Hopp over. Må avklares sammen med `ack_required`-flyten (`protected_by_scope`): skal brukeren kunne velge «Flytt likevel» med eksplisitt bekreftelse, eller er beskyttet alltid låst i Gjennomgang?
4. **Beskyttelse og `protectFlags`.** Mocken setter bit 16 på bilder brukeren selv har beskyttet. Kjernen sender plattformflaggene (1, 2, 4, 8) i `Card.protect_flags`; beskyttelse fra `protection`-tabellen (bilde, mappe, album, person) går i dag via bekreftelsesporten (`ack_required`) og ikke via flagget. Avklar med Integrer om kjernen også skal sette et flagg for dette, ellers må Review/Cards vise slike kort som vanlige og møte `ack_required`.
5. **Serie vs. gruppe i Gjennomgang.** Serier vises som grupperader (som duplikater) med «Sammenlign serien». «Velg gruppe» velger alle kort unntatt behold-kandidaten, som for duplikater. Er det riktig for serier der brukeren ofte vil beholde to eller tre?
6. **`series.eyes.unavailable`.** Til øyemodell er valgt (V1-PLAN åpent spørsmål 12) rangeres serier på skarphet alene. Skal merket vises på hver serie eller bare én gang øverst i Gjennomgang?
7. **«Tømt» vs «frigjort».** `freedBytes` betyr bare tømt fra Fjell (standard «Tøm» går til Windows Papirkurv). Dashboardet må ikke påstå frigjort plass; tekstene `dashboard.freed.*` er allerede formulert slik.
8. **Regel-forhåndsvisning før lagring.** `previewRule` gir antall, størrelse og opptil tre eksempelbilder. Skal «Lagre» være deaktivert ved 0 treff (regelen er likevel gyldig)? Foreslått: aktiv, med forklaring.
9. **Navn er brukerens egne** (personer og regler, maks 100 og 80 tegn). Vis aldri navnene i miniatyrenes `alt`/tooltips i feilmeldinger som kan logges; navn forlater aldri enheten.
10. **200 % skalering og pseudo.** `nav` har nå seks valg; under 900 px bryter toppmenyen over flere linjer. Verifiser ved 200 % og med pseudo (nb +30 %) når skjermene er på plass; ingen horisontal sideskroll.
11. **Home og seks kategorikort.** `auto-fit` gir 3×2 på bred skjerm. Skal serier og regler vises under en egen overskrift («Nytt i V1») eller blandes inn? Foreslått: blandes inn.
