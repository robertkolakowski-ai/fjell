# V1-skjermer: kontrakt

Status: kontrakten og skjermene er lagt og integrert (del 7). Bridgen (`CoreBridge`, `TauriBridge`, `MockBridge`), ruter, navigasjon, forslagstypene serier og regler, monteringspunkter og `ProtectToggle` finnes, og skjermene er bygget på toppen av dette (grenene `ui/dashboard`, `ui/series`, `ui/people`, `ui/rules`). Kjernen og kommandoflaten er beskrevet i `docs/v1/V1-PLAN.md`; komponent-API og tokens i `docs/design/LOFT.md`.

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

Rekkefølge i navigasjonen: Hjem, Gjennomgang, Personer, Regler, Papirkurv, Innstillinger (`NAV_ROUTES` i `router.ts`). `navFor(series)` er `review`. Ugyldig `series/:id` gir Hjem, som alle andre ruter. F1-hjelp har rader for `series` (1–9, + / −, 0); `people` og `rules` har ingen egne taster (bare «Overalt»).

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
- **Beskyttede bilder** får `protectFlags` bit 16 (`SCOPE_FLAG`), så skjermene aldri forhåndsvelger dem. Kjernen setter samme bit (del 6, punkt 4). Papirkurv uten bekreftelse gir `ack_required` med `detail = "protected_by_scope"`; behold er alltid trygt.
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

## 6. Avklaringer av de åpne UX-spørsmålene (UX-fasen)

Merkene: **besluttet** = følger av `CLAUDE.md` (trygghet først) eller er gjort og testet i koden. **Anbefaling, ikke besluttet** = egentlig et produktvalg for Robert; anbefalingen er det som er bygget til han sier noe annet.

| # | Spørsmål | Avklaring | Status |
|---|---|---|---|
| 1 | Personer i navigasjonen når indeksen er av | Valget er synlig så funksjonen kan oppdages, men ansiktsindeksen slås aldri på av seg selv (egen «Slå på» med samtykke). | Aldri automatisk: **besluttet** (`CLAUDE.md`). Synlig valg: **anbefaling, ikke besluttet** |
| 2 | Beskyttelsesbryter: hvor | `ProtectToggle` i Gruppe, Serie, Personer og Innstillinger («Beskyttelser»), ikke på hver flis i Gjennomgang. Beskyttede fliser er likevel synlige fordi kjernen merker dem (se 4). | **Anbefaling, ikke besluttet** |
| 3 | Beskyttede kort i Kortmodus og «Flytt likevel» | Beskyttet er låst i skjermene: aldri i Kortmodus-bunken, aldri forhåndsvalgt, ikke flyttbare i Gjennomgang, Gruppe og Serie. For å flytte et beskyttet bilde fjerner brukeren beskyttelsen først (kan angres). Det finnes ingen «Flytt likevel» i V1. Porten i kjernen (`ack_required` med `protected_by_scope`) står uendret som siste sperre for det som likevel kommer gjennom (beskyttelse satt etter at listen ble lastet, CLI, andre klienter), og dialogene som svarer på den finnes i Gjennomgang, Gruppe, Serie og Kortmodus. | Låst beskyttelse: **besluttet** (trygghet først). Å tilby «Flytt likevel» senere: **anbefaling, ikke besluttet** (nei i V1) |
| 4 | Beskyttelse og `protectFlags` | **Løst, kjernen er sannheten.** Kjernen setter bit 16 (`SCOPE_FLAG`) i `Card.protect_flags` og `SeriesMemberView.protect_flags` for bilder som er beskyttet via `protection`-tabellen (bilde, mappe, album, person), pluss plattformflaggene 1, 2, 4, 8 som før. MockBridge brukte allerede bit 16; nå gjør kjernen det også (`api/fjell_v1.rs`, `scoped_assets`/`with_scope_flag`; ett kall mot `protection` per side). Flagget er bare visning; sletting stoppes av `ack_required`. `conformance_v1.json` holder mock og kjerne like (del 3). | **Besluttet** og testet |
| 5 | Serie vs. gruppe i Gjennomgang | Serier vises som grupperader. «Velg gruppe» velger alle unntatt behold-kandidaten, som for duplikater. Ingenting flyttes uten bekreftelse, og alt kan angres. Vil brukeren beholde to eller tre, avmerker hun enkeltbilder eller åpner «Sammenlign serien». | **Anbefaling, ikke besluttet** |
| 6 | `series.eyes.unavailable` | Merket vises én gang per serieside (`SeriesCompare`), ikke på hver rad i Gjennomgang. Et felles merke øverst i Gjennomgang er ikke bygget (`Review.svelte` er delt). | **Anbefaling, ikke besluttet** |
| 7 | «Tømt» vs «frigjort» | Dashboardet sier «tømt», aldri «frigjort», til purge-modus lagres (V1-PLAN, oppfølging). | **Besluttet** (kjernen kan ikke vite om «Tøm» gikk til Windows Papirkurv) |
| 8 | Regel-forhåndsvisning før lagring | «Lagre» er aktiv ved 0 treff, med forklaringen `rules.preview.zero`. Skjermen avviser derimot tom regel og tomme eller ugyldige verdier lokalt (se del 8). | **Anbefaling, ikke besluttet** (0 treff). Lokal validering: **besluttet** |
| 9 | Navn er brukerens egne | Navn på personer og regler brukes aldri i `alt`, `title`, `aria-label` på miniatyrer eller i feilmeldinger, og skrives ikke til konsollen (gjennomgang av `People`, `Rules`, `RuleEditor`, `CardExplain`, `SettingsFaces`). Gjennomgangen er ikke en automatisk test. | **Besluttet**; ikke automatisk testet |
| 10 | 200 % skalering og pseudo | Dekket av `herding.spec.ts` (omvisningen går gjennom alle V1-skjermer med skalering 100–200 %, pseudo-språk og høykontrast). Ikke kjørt på nytt i UX-fasen, og ikke verifisert med Narrator eller NVDA. | **Ikke verifisert på Windows** |
| 11 | Home og seks kategorikort | Serier og regler blandes inn i kategorikortene (3×2 på bred skjerm), uten egen overskrift. | **Anbefaling, ikke besluttet** |

Anbefalingene som egentlig er Roberts produktvalg: 1 (synlig valg), 2, 3 (nei til «Flytt likevel»), 5, 6, 8 (aktiv ved 0 treff) og 11.

## 7. Konformitet mellom MockBridge og kjernen

Skjermer og e2e kjører mot `MockBridge`; den ekte appen kjører mot kjernen. For at mocken ikke skal love noe kjernen ikke gjør, kjører begge samme scenarier fra én fil, `core/crates/fjell-core/src/api/fjell_v1_tests/conformance_v1.json`:

- Kjernen: `api/fjell_v1_tests/conformance.rs` (`core_matches_conformance_fixture`) mot en ekte `Fjell` med syntetisk korpus.
- Mocken: `desktop/src/bridge/conformance.test.ts` mot `MockBridge`.

Fixturen dekker, for hver forslagstype (eksakt, visuell, serie, regel, uskarp) og for beskyttelse av bilde og mappe: merket (`SCOPE_FLAG`), `ack_required` med `protected_by_scope`, bekreftet flytting, behold uten bekreftelse, fjernet beskyttelse, «alt eller ingenting» når ett kort i runden er beskyttet, medlemsflagg i serie og beskyttet behold-kandidat, regeltreff som forsvinner og kommer tilbake, at samme bilde aldri står i flere kort, mappebeskyttelse (hele mappenavn og eksakt skrivemåte), wipe av ansiktsindeksen (personbeskyttelse blir bildebeskyttelse, ingen mister vernet) og 20 feilkoder. Endres kontrakten, endres fixturen og begge sidene i samme commit.

Forskjeller konformitetstesten avdekket (kjernen vant, mocken er rettet):

| Område | Mocken gjorde | Kjernen gjør (nå også mocken) |
|---|---|---|
| Forhåndsvisning av tom regel | `empty_rule` | 0 treff, ingen feil (en tom regel treffer aldri alt) |
| Forhåndsvisning av tomt mime- eller mappevilkår | `invalid_condition` | 0 treff, ingen feil |
| Tall i vilkår (`older_than_days: 0` osv.) | `invalid_condition` for alt ≤ 0 | Tar imot heltall fra 0; avviser bare tomme tekstvilkår ved lagring |
| Mappebeskyttelse | rent prefiks, uavhengig av store/små bokstaver | hele stikomponenter, tegn for tegn |

## 8. Det skjermen selv må fange

Siden kjernen tar imot `0` og forhåndsvisningen ikke validerer, ville en tom tallrute i regelbyggeren (tolket som 0) blitt en regel som treffer alle bilder. `conditionsError` i `screens/rulesState.ts` stopper det før bridgen kalles: minst ett vilkår, tekstvilkår ikke tomme, tall hele og minst 1. Feilkodene er de samme (`empty_rule`, `invalid_condition`). Anbefaling til kjernen (ikke gjort, filen eies ikke av UX-fasen): avvis `0` som `invalid_condition` i `suggest_rules::validate`.

**Funn, rettet i V1-fullføringen (`52c5963`):** skallet sender V1-feilkodene som `{ code: "core_error", detail: "empty_rule" }` (`ErrorDto::from(ApiError)`, testet i `tests_v1.rs`), mens mocken og tekstoppslaget (`error.<kode>`) bruker `code: "empty_rule"`. `TauriBridge` normaliserer nå (`normalizeError`): når `code === "core_error"` og `detail` er en av `V1_ERROR_CODES`, brukes `detail` som `code`. Konformitetstesten bygger på den normaliseringen. Ikke verifisert i den ekte appen på Windows.

## 9. Albumvilkår uten `listAlbums`

Regelvilkåret `in_album` tar et albumnummer, og brukeren har ingen måte å finne nummeret på i appen. Det er trygt (feil nummer gir færre eller andre forslag, aldri sletting), men ubrukelig. **Besluttet: `listAlbums` legges ikke til i denne fasen.** Den trenger endringer i delte filer og i skallet som ikke lar seg bygge her: `FjellApi` (`api/mod.rs`), `CoreBridge` og `TauriBridge`, Tauri-kommando og DTO i `desktop/src-tauri`, `fjell-ffi`-speil og i18n. Å legge halvparten inn (mock og bro uten skall) ville gitt en metode som feiler i den ekte appen. Minste mulige utforming for Integrer:

- Kjerne: `FjellApi::albums() -> Result<Vec<AlbumView>, ApiError>` med `AlbumView { id: u64, name: String, shared: bool, count: u64 }`, rett fra `album` og `asset_album` (aktive bilder), sortert på navn. Navnene er brukerens egne album fra plattformen og forlater aldri enheten.
- Bro: `listAlbums(): Promise<Album[]>` i `CoreBridge`, `TauriBridge` (`list_albums`) og `MockBridge` (fra `MOCK_ALBUMS`, med navn).
- UI: `RuleEditor` bytter tallfeltet for `in_album` med en liste; `ProtectionsSession` bruker navnet i stedet for «nr. N».
- Til da: anbefaling, ikke besluttet, at `in_album` skjules i regelbyggeren i den ekte appen.

## 10. Status etter integrasjon

Alle skjermene i del 1 er bygget; `placeholder.todo` er fjernet. Navigasjonen viser teller for Gjennomgang (alle seks typer), Personer (når ansiktsindeksen er på), Regler (aktive) og Papirkurv; tellerne for Personer og Regler hentes fra `getDashboard` ved start og rutebytte. Hjem viser oversikten (`DashboardPanel`) med små tellere som lenker til serier, personer og regler.

| Område | Dekning |
|---|---|
| Enhetstester | `dashboard.test.ts`, `series.test.ts`, `people.test.ts`, `rules.test.ts`, `help.test.ts` (rader for Serie) |
| e2e | `dashboard`, `series`, `people`, `rules`, `v1-skjermer`; felles omvisning `tour.ts` har Serie (to), Regler (+ ny regel), Personer (av, samtykke, på), Kortmodus for serie og regel, Gjennomgang for serier og regler |
| axe | `a11y.spec.ts`: alle V1-ruter og dialoger, lys/mørk × bred/smal |
| Herding | `herding.spec.ts` (skalering 100–200 %, minstevindu, pseudo, høykontrast, redusert bevegelse) går gjennom omvisningen |
| Skjermbilder | `docs/screens/v1/` (nb, lys/mørk, bred 1280 og smal 480), laget med `SHOTS=1 npx playwright test e2e/skjermbilder-v1.spec.ts` |

Funn under integrasjon: `MockBridge.getUpdateDownloadUrl` returnerte en `http://`-adresse som havnet i produksjonsbygget og feilet `nonet.test.ts` så snart `dist/` fantes; den er nå en sti uten skjema.

Åpne UX-spørsmål 1–11 er avklart i del 6 (de som er produktvalg står som anbefaling, ikke besluttet) (Personer synlig i navigasjonen, beskyttelsesbryter i detaljvisning, serier som grupperader, «Tømt» og ikke «frigjort»). Verifisering med Narrator og NVDA på Windows med ekte kjerne gjenstår (som M9 for MVP).
