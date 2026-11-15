# V1-plan: serier, beskyttelse, personer, regler og dashboard

Status: kontrakten er lagt og de fem grenene (`protect`, `series`, `dashboard`, `people`, `rules`) er integrert (del 7). Omfang: kjernen i `core/`, CLI og Tauri-skallets kommandoflate. Skjermkontrakten (bro, ruter, monteringspunkter) er lagt i `docs/v1/SKJERMER.md`; selve skjermene bygges i egne faser.
Bindende regler: `CLAUDE.md` (ingen hard sletting, alt kan angres, appen foreslår, on-device, ingen nettverk, < 100 ms p95, nb + en, plattformnøytral kjerne).

## 1. Mål

1. **Serier**: rekker av nesten like bilder tatt tett i tid grupperes; appen foreslår beste bilde (skarphet, åpne øyne) og forklarer valget.
2. **Beskyttelse**: bruker beskytter enkeltbilder, mapper, album eller personer. Beskyttede bilder krever alltid egen bekreftelse før sletting.
3. **Personer**: valgfri ansiktsindeks (av som standard) gir personer som kan navngis, slås sammen og beskyttes. Embeddings forlater aldri enheten.
4. **Regler**: brukeren lager regler («skjermbilder eldre enn 90 dager»). Regler foreslår bare; ingen automatisk sletting.
5. **Dashboard**: oversikt over bibliotek, forslag, papirkurv og frigjort plass, rett fra indeksen.
6. **Innstillinger**: sensitivitet per forslagstype og ansiktsindeks (standard av).

Alt gjenbruker eksisterende trygghetsløype: forslag blir kort, beslutninger går via `actions` (plan, bekreftelse, én batch, angre), sletting er karantene og Papirkurv.

## 2. Graf

```
Kontrakt ──┬─> protect   ─┐
           ├─> series    ─┤
           ├─> dashboard ─┼─> Integrer ─> Sluttreview
           ├─> people    ─┤
           └─> rules     ─┘
```

- Grener kan **bare** avhenge av kontrakten, aldri av hverandre. Alt delt ligger i kontrakten.
- Hver gren: én gren og én PR, rører bare filene den eier (del 3).
- Integrer eier all kobling til `Fjell`, pipeline og skall (inkl. bytte av `home_summary` fra `SuggestionKind::MVP` til `ALL` når UI viser serier og regler). Sluttreview er `code-review` mot hele V1-diffen og en full `just check`.

## 3. Eierskap per fil

**Kontrakten eier alle delte filer** og er ferdig før grenene starter. Grenene endrer dem ikke; trenger en gren en endring i en delt fil, meldes det til Integrer (åpne spørsmål, del 4).

Delt (kontrakten, låst for grenene):
`model/mod.rs`, `model/v1.rs`, `model/analyze.rs`, `model/plan.rs`, `store/005_v1.sql`, `store/migrate.rs`, `store/migrate_tests.rs`, `store/ops_tests.rs`, `store/mod.rs`, `api/mod.rs`, `api/types.rs`, `api/mock.rs`, `lib.rs`, `tools/check-layers/layers.toml`, `i18n/nb.json`, `i18n/en.json`, `docs/mvp/ARKITEKTUR.md`, `docs/v1/V1-PLAN.md`, `fjell-ffi`-speilene (`types.rs`).

| Gren | Eier (hele mappen/filen) | Bruker fra kontrakten |
|---|---|---|
| `protect` | `src/protect/`, `store/protection.rs`, `src/actions/plan.rs` (bare de delene som legger til `ConsequenceCode::ProtectedByScope`/`ProtectCounts.scope`; ingen annen gren rører `actions/`) | `ProtectScope`, `ProtectionEntry`, tabell `protection` |
| `series` | `src/series/`, `store/series.rs` | `SeriesId`, `EyeAnalyzer`/`EyeReport`, `ReasonCode::{SeriesMember, KeeperBestOverall, KeeperEyesOpen, ClosedEyes}`, `ReasonDetail::Position`, `SeriesCompare`, tabellene `grp` (kind `series`), `eye_result` |
| `people` | `src/people/`, `store/people.rs` | `PersonId`, `FaceAnalyzer`/`FaceObservation`/`Embedding`, `PersonView`, tabellene `person`, `face_embedding`, `setting.face_index_enabled` |
| `rules` (nå `suggest_rules`) | `src/suggest_rules/`, `store/suggest_rules.rs` | `RuleId`, `RuleSpec`/`RuleCondition`, `RuleDraft`/`RuleView`/`RulePreview`, `ReasonCode::Rule`, tabellene `rule`, `rule_hit` |
| `dashboard` | `src/dashboard/`, `store/dashboard.rs` | `DashboardData` og delvisninger |
| Integrer | `api/fjell.rs`, `api/fjell_ops.rs` (+ tester), `pipeline/` (kjør analysatorer og gruppering), `fjell-cli`, `fjell-ffi`, `desktop/`, `testdata/`, `Settings`-lagring | alt over |

Hver av de fem modulene har en smoketest i `src/<modul>/tests.rs` og et skjelett i `store/<modul>.rs` (en `Store`-metode med test). Grenen utvider disse; den lager ikke nye delte filer. Nye filer i egen mappe er fritt fram.

Lagdeling (`tools/check-layers`): de fem modulene er L2, kan bruke `model` og `store`, men ikke hverandre eller `actions`/`suggest`/`cluster`. Trenger en gren logikk fra en annen L2-modul, løftes den til `model` av Integrer eller dupliseres bevisst.

## 4. Kontraktens innhold

- **model**: `SeriesId` (= `grp.id` med `kind = 'series'`), `PersonId`, `RuleId`, `ProtectScope` (kode og mål-koding for `protection`), `ReasonDetail`, `RuleSpec`/`RuleCondition` (tom regel treffer ingenting), `DashboardData`; `EyeAnalyzer`, `FaceAnalyzer` med `NullEyeAnalyzer`/`NullFaceAnalyzer` (svarer `Unavailable`, aldri «ingen funn»); `Embedding` (LE-bytes). `SuggestionKind::{Series, Rule}` (`ALL` = 6), 6 nye `ReasonCode` (`ALL` = 18) med i18n i nb og en, `ConsequenceCode::ProtectedByScope`, `KindCounts.{series, rule}`, `ProtectCounts.scope`. `Card.detail: ReasonDetail`.
- **store**: migrering `005_v1.sql` (opprinnelig `002_v1.sql`; omnummerert ved sammenslåing med main, 001–004 uendret): `grp.kind` og `suggestion.kind` utvidet via trygg tabellomskriving (`foreign_keys` av utenfor transaksjonen, `foreign_key_check` før commit, ingen kaskade; testet med data), `protection`, `eye_result`, `person` (`merged_into`), `face_embedding` (BLOB, kun lokal), `rule` (`deleted_at`, trigger mot hard sletting), `rule_hit`, `setting.face_index_enabled = '0'`. «Fra tom» og «fra forrige» gir lik dump (K6-1). Triggere og I1-I16 urørt. I16 er utvidet bevisst, se åpne spørsmål.
- **api**: `FjellApi` har alle V1-metodene med standardimplementasjon (tom verdi eller `ApiError::Internal("ikke implementert")`): `protect_set`, `unprotect`, `protections`, `rules_list`, `rule_save`, `rule_preview`, `rule_toggle`, `rule_delete`, `rule_restore`, `people_list`, `person_name`, `person_merge`, `person_unmerge`, `person_protect`, `face_index_enabled`, `face_index_set`, `face_index_wipe`, `dashboard`, `series_compare`, `thresholds`. `rule_delete` og `person_merge` er angrebare (`rule_restore`, `person_unmerge`). `MemApi` og `Fjell` uendret oppførsel.
- **Settings**: `trash_days`, `sensitivity: Sensitivities` (per `SuggestionKind`, standard `Normal`), `face_index: bool` (standard av). Nøkler i `setting`: `sensitivity.<kind>` og `face_index_enabled`; manglende nøkkel gir standardverdi (eldre databaser leses uendret). Lagring i `Fjell` er Integrers jobb.

## 5. Åpne spørsmål

1. **I16 utvidet**: `person.name`, `rule.name`, `rule.spec` er brukerens egne etiketter (første fritekst i indeksen), og `face_embedding.embedding` er første BLOB utenom `content_hash`. Begge er listet eksplisitt i `i16_schema_has_no_image_data_or_free_text`. Bekreft at dette er akseptert, eller krev kryptering/ekskludering fra eksport.
2. **`face_index_wipe`** sletter embeddings og personer (avledet data, kan gjenskapes) og er ikke angrebar. Det er et eksplisitt personvernvalg med varsel i UI. Beskyttelser av typen `person` gjøres om til beskyttelse av hvert bilde i samme transaksjon (avgjort: ingen hengende rader, ingen id-gjenbruk, ingen bilder mister beskyttelsen). Slettede bytes overskrives (`secure_delete`) og WAL tømmes. Navn på personer slettes og kan ikke angres (del av valget).
3. **Terskler**: `Thresholds`-standardene (Hamming 6, blur 0,006, serieglipp 10 s, øyesikkerhet 0,8) er startverdier. Sensitivitet `Low/Normal/High` skal kalibreres mot golden-korpus og dokumenteres her før Integrer; modellvalg for øyne og ansikter (lisens, ≤ 30 MB, ONNX, ARKITEKTUR beslutning 3) er ikke tatt. Til da brukes Null-analysatorene.
4. **Regelspråk**: `RuleCondition` er en startliste (skjermbilde, uskarp, eksakt duplikat, alder, størrelse, album, mappe, MIME). Kodingen til `rule.spec` velges av `rules`-grenen (stabil, versjonert tekst). Mer uttrykkskraft (ELLER, negasjon) venter.
5. **Mappebeskyttelse og flytting**: `Folder(prefiks)` matcher `source_id`-prefiks. Flytting utenfor Fjell ses som «borte + ny» (beslutning 21), så beskyttelsen følger ikke med. Akseptabelt i V1?
6. **Serier vs. visuelle duplikater**: samme bilde kan være medlem i både en `visual`- og en `series`-gruppe, og dermed ha to forslag (`UNIQUE(asset_id, kind)` tillater det). Integrer må avgjøre kortrekkefølge og «samme asset vises aldri i flere kort samtidig»-regelen.
7. **Endring i delt fil midt i en gren** (f.eks. flere `ReasonCode`): sendes til Integrer, som samler dem i én PR mot kontrakten.
8. **Ekte ansiktsmodell (people)**: `people` er ferdig mot `FaceAnalyzer`-trekket med `MemFaceAnalyzer` (tester) og `NullFaceAnalyzer` (`Unavailable`, standard). Modell for deteksjon og embedding er ikke valgt (lisens, ≤ 30 MB, ONNX, ARKITEKTUR beslutning 3), og ingen modell er lastet ned. Klyngeterskelen (`PeopleParams.threshold`, cosinus 0,6) er en plassholder som må kalibreres mot golden-korpus når modellen er valgt; embeddingdimensjonen følger modellen. Integrer må i tillegg: (a) kjøre `index_asset` og `cluster_new_faces` i pipeline bare når `face_index` er på, (b) koble `person_protect` til `protect` (`ProtectScope::Person`; `people::person_assets` gir bildene), og (c) ferdig: `face_index_wipe` gjør personbeskyttelser om til bildebeskyttelse (jf. spørsmål 2).
9. **Person-typer i `store`** (`PersonRow`, `FaceRow`) ligger i `store/people.rs`, men `store/mod.rs` (delt, låst) eksporterer dem ikke. `people` speiler det UI trenger (`PersonSummary`, `MergeOutcome`). Integrer kan legge til `pub use people::{FaceRow, PersonRow};` hvis `fjell-ffi` trenger dem direkte.

## 6. Definisjon av ferdig

Kontrakten er ferdig når:
- `cargo fmt --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`, `cargo test --workspace --locked`, `cargo test` i `core/testdata`, `check-layers` (med selvtest) og i18n-sjekken er grønne.
- Migrering 2 gir lik dump fra tom og fra versjon 1, bevarer data og triggere, og slår `foreign_keys` på igjen.
- Alle fem modulene finnes (modulhode, tom API, smoketest) og er registrert i `lib.rs`, `layers.toml` og ARKITEKTUR del 2.
- `FjellApi` har alle V1-metodene med standardimplementasjon; `MemApi` og `Fjell` kompilerer og oppfører seg som før.

Hver gren er ferdig når:
- Alt den eier er implementert test først, `just check` er grønn, og ingen delt fil er endret.
- Den bryter ingen CLAUDE.md-regel: ingen hard sletting, alt som endres kan angres, hvert forslag har begrunnelsesmerke, ingen nettverk, handlinger < 100 ms p95, nb + en.
- Modulhodet («Avhenger av:», «Grensesnitt:») er oppdatert.

V1 er ferdig når Integrer har koblet alle grenene til `Fjell` (skanning kjører serier, ansikter bare ved `face_index`, regler og beskyttelse inn i `actions`-planen), `testdata` har golden-tester for hver forslagstype, og Sluttreview har ingen åpne funn.

## 7. Integrasjon (status)

Grenene er slått sammen i rekkefølgen protect, series, dashboard, people, rules uten konflikter (hver eide sine egne filer). Integrer har koblet alt til `Fjell`:

- **Pipeline** (`pipeline/groups.rs`, `tuning.rs`, `analyzers.rs`): `refresh_groups` regner nå eksakte og visuelle grupper, deretter serier (`Store::apply_series`) og til slutt forslagsregler (`suggest_rules::evaluate`). `run_analyzers` kjører etter skanning: øyne til serier, og ansikter og klynging bare når `face_index` er på. `Unavailable` (Null-analysatorene) avbryter steget uten å lagre noe; ingen modell er valgt eller lastet ned. `Fjell::with_analyzers` injiserer modellene.
- **Sensitivitet**: `setting.sensitivity.<type>` leses av `pipeline::tuning_from_store`. Low/Normal/High flytter visuell `max_dist` (4/6/8), uskarphetsterskel (x0,7/x1/x1,3), serieglipp (5/10/20 s), serie-`max_dist` (8/10/12) og øyesikkerhet (0,9/0,8/0,7). Normal er identisk med MVP. Verdiene er startverdier, ikke kalibrert (åpent spørsmål 3).
- **API** (`api/fjell_v1.rs`): alle V1-metodene på `FjellApi` er overstyrt for `Fjell`. `Card.detail` fylles (serieposisjon, antall lukkede øyne, regel). `home_summary` viser alle seks forslagstyper. Feil er koder i `ApiError::Internal` (`empty_folder`, `unknown_target`, `empty_rule`, `empty_name`, `name_too_long`, `invalid_condition`, `rule_not_found`, `unknown_person`, `not_merged`, `unknown_series`).
- **Kortprioritet** (`store/cards.rs`): eksakt > visuelt > serie > regel > skjermbilde > uskarpt, så samme bilde vises aldri i flere kort. Svar på åpent spørsmål 6: bilder som alt er eksakt eller visuelt duplikat deltar ikke i serier (`series_specs`), så serier dekker nesten-like bilder utenfor duplikatterskelen (Hamming 7..=10) og alle serier om sensitiviteten for visuelle duplikater settes til Low.
- **Skall og CLI**: `fjell-cli` har `protect`, `series`, `dashboard`, `people`, `rules`; Tauri-skallet har DTO-er (`dto_v1.rs`), `Shell`-metoder (`v1.rs`), Tauri-kommandoer og `desktop/src/bridge/{types,types-v1,core-bridge,tauri-bridge,mock-bridge}.ts` med samme kontrakt. Skjermene: se `docs/v1/SKJERMER.md`.
- **i18n**: 57 nye nøkler i `nb.json` og `en.json` (områdene `protect`, `series`, `dashboard`, `people`, `rules`, `settings.sensitivity`), `gen_i18n --check` grønn.
- **Tester**: `api/fjell_v1_tests.rs` (skann syntetisk korpus, serie med beste valg, beskyttet bilde, regel, dashboard, ansiktsindeks av som standard og wipe, originalenes sjekksum uendret), `fjell-cli/tests/cli_v1.rs`, `shell/tests_v1.rs`, `pipeline/tuning.rs`.

### Funn under integrasjonen

- **Sikkerhetshull lukket**: `actions::commit` kjenner bare plattformflaggene (favoritt, delt album, redigert). `ConsequenceCode::ProtectedByScope` stod i planens konsekvenser, men `decide_many` krevde ingen bekreftelse for den, så en beskyttet serie- eller duplikatkandidat kunne flyttes til «Nylig slettet» uten ack. Porten ligger nå i `api/fjell_ops.rs` (`needs_ack`) og gjelder også `confirm_purge` når beskyttelsen kom etter at bildet havnet i papirkurven. Testet i `fjell_v1_tests.rs`. Bør på sikt flyttes inn i `actions::commit` med en egen ack-kategori (delt `model`-endring).
- `protect_set`/`unprotect` evaluerer reglene på nytt, ellers ville regeltreff på beskyttede bilder stått til neste skanning.

### Nye åpne spørsmål

10. **Bilder uten ansikter analyseres på nytt** ved hver kjøring med `face_index` på (ingen markør i skjemaet). Trenger en delt schema-endring (f.eks. `face_scan(asset_id, algo_ver)`) før en ekte modell tas i bruk.
11. **`fjell-ffi`** speiler ikke V1-flaten ennå (UniFFI-typene for protect, regler, personer, dashboard). Trengs før iOS/Android, ikke for Windows-MVP.
12. **Ansiktsmodell og øyemodell** er fortsatt ikke valgt (spørsmål 3 og 8). Til da er seriene rangert på skarphet alene, og `eyes_unavailable`-merket i UI bør forklare det (`series.eyes.unavailable`).
13. **`face_index_wipe` og hengende personbeskyttelser** (spørsmål 2): løst: ingen hengende beskyttelser finnes lenger (se spørsmål 2), så ingen teller eller varsel trengs.
14. **Skall mot `MemApi`**: `Shell::mock()` svarer på V1-kommandoene med tomme lister og feilkoden `core_error`. Skallet kobles til `Fjell` når UI-fundamentet (M6/M7) og lagring av indeksfil i appdata er på plass.
15. **`just` er ikke installert i kjøremiljøet**; oppskriftene i `justfile` ble kjørt som enkeltkommandoer. `fjell-desktop` (Tauri-limet med de nye kommandoene) kan ikke kompileres uten webkit2gtk og er bare gjennomgått, ikke bygget; `desktop-check` kjører det på `windows-latest`.

## Oppfølging etter verifisering

- **«Frigjort» er nå «Tømt» i UI-tekstene** (`dashboard.freed.*`, `trash.freed.note`): `action_log` har ingen purge-modus (ARKITEKTUR 22), så kjernen vet ikke om «Tøm» gikk til Windows Papirkurv. Åpent: legg `recycle`/`permanent` på purge-raden (ny migrering) og tell bare permanent som frigjort.
- **Klynging holder ikke skrive-låsen under regningen**: `cluster_load` (kort lås), `cluster_plan` (ingen lås), `cluster_apply` (én transaksjon). p95-test i `people/tests.rs`.
- **Dashboard**: `FjellApi::dashboard_full` gir fremdrift, neste steg og tømt per måned (`DashboardDto`).
- **Keeper-bytte**: `set_keeper` finnes som Shell-metode, Tauri-kommando og `setKeeper` i bridgen.

## Sammenslåing med main (regelmotoren fra PR #25)

Main har en egen regelmotor (keeper-regler, `docs/mvp/REGLER.md`: `src/rules/`, `store/rules.rs`, migrering 004). Den gjelder og er uendret. V1 er lagt oppå:

- **Navn**: V1-reglene heter nå forslagsregler og bor i `src/suggest_rules/` og `store/suggest_rules.rs` (tabellene `rule`, `rule_hit` og forslagstypen `rule` er uendret). Keeper-reglene velger keeper i grupper; forslagsreglene foreslår enkeltbilder. Ingen av dem sletter eller godtar noe.
- **Beskyttede mapper vinner**: et bilde under en keeper-regel `ProtectFolder` får verken serie- eller regelforslag og telles ikke i forhåndsvisning (`suggest_rules::Guard`, bygget i `pipeline/groups.rs` og `api/fjell_v1.rs`; testet i `api/fjell_v1_tests.rs` og `suggest_rules/tests.rs`).
- **Migrering**: `002_v1.sql` er blitt `005_v1.sql`. Den tar med `grp.keeper_reason`/`grp.keeper_rule` og indeksene fra 002 og 003 gjennom tabellomskrivingen. En database fra main (versjon 4) oppgraderes uten tap (`store/migrate_tests.rs`).
- **Gruppering**: `refresh_groups` bruker mains inkrementelle klynging og lesetilkobling; sensitivitet går inn via `compute_tuned`. Serier og forslagsregler skrives under låsen rett etter gruppene (`apply_v1`). `store/groups.rs` rører ikke lenger serier eller forslag av typen `series`/`rule`.
- **Skall og bro**: `Shell::decide_confirmed`, `set_keeper` og `settings` er mains. V1-innstillingene heter `suggestion_settings`/`set_suggestion_settings` (`get_suggestion_settings`/`set_suggestion_settings` i Tauri, `SuggestionSettingsDto`). V1-funksjonene i frontend er flyttet inn i `CoreBridge`, `TauriBridge` og `MockBridge` (`v1.ts` er fjernet; typene ligger i `types-v1.ts`). Skjermkontrakten og eierskap per skjermfil står i `docs/v1/SKJERMER.md`.

Punkt 14 over er løst av main: skallet åpner en ekte `Fjell` (`Shell::with_data_dir`), så V1-kommandoene går mot ekte kjerne; `Shell::mock()` brukes bare i tester og dev. Punkt 15 gjelder fortsatt: `fjell-desktop` (Tauri-limet) er ikke bygget i utviklingsmiljøet (mangler webkit2gtk); de nye kommandoene der er bare gjennomgått, og bygges av `desktop`-jobben på `windows-latest`.

Åpent etter sammenslåingen:

16. **(Løst)** Gjennomgang, Kortmodus og Hjem viser alle seks forslagstyper, og skjermene er bygget på bridgen: serieside med synkronisert zoom og «Gjør dette til beste» (`SeriesCompare`), regelforklaring på kort (`CardExplain`), Regler med redigering og forhåndsvisning, Personer med samtykke til ansiktsindeksen, beskyttelser og følsomhet i Innstillinger og oversikten på Hjem. Bilder med serie- eller regelforslag kan dermed gjennomgås i UI. Se `docs/v1/SKJERMER.md` del 10. Gjenstår før V1 slippes: kalibrering av terskler og valg av øye- og ansiktsmodell (egne punkter over).
17. **To mappebeskyttelser** finnes side om side: keeper-regelen `ProtectFolder` (hele stikomponenter, uavhengig av store/små bokstaver; får aldri forslag) og `protection`-tabellens `folder` (rent prefiks; krever egen bekreftelse). Bør samles i én.
18. **Mappevilkåret i forslagsregler** (`InFolder`) er rent prefiks på `source_id`, ikke hele stikomponenter som i keeper-reglene.
19. **Sensitivitet og inkrementell klynging**: endret sensitivitet for visuelle duplikater bygger klyngeindeksen på nytt ved neste utregning.

## Avklaringer av åpne spørsmål (UX-fasen)

«Besluttet» følger av `CLAUDE.md` eller er gjort og testet. «Anbefaling, ikke besluttet» er et produktvalg for Robert; anbefalingen står som valgt til han sier noe annet. «Åpen» betyr at det ikke lot seg lukke her (krever modell, Windows eller en delt fil). UX-spørsmålene står i `docs/v1/SKJERMER.md` del 6.

| # | Avklaring | Status |
|---|---|---|
| 1 | I16-utvidelsen (`person.name`, `rule.name`, `rule.spec`, `face_embedding`) godtas: indeksfilen ligger lokalt i appdata, det finnes ingen eksport, og ingenting forlater enheten. Kryptering av indeksen er ikke krevd i V1. Skulle en eksport komme, skal navn og embeddings ekskluderes som standard. | **Anbefaling, ikke besluttet** (personvern) |
| 2 | `face_index_wipe` er ikke angrebar (avledet data, kan gjenskapes), krever bekreftelse med varselet `people.index.wipe.confirm`, og personbeskyttelser blir beskyttelse av hvert bilde. Konformitetstesten viser at mocken og kjernen gjør det samme. | **Besluttet** |
| 3 | Terskler og modellvalg. Kalibreringsverktøy og golden-tester finnes (`KALIBRERING-V1.md`) og `fjell-models` har rammen (`MODELLER.md`), men tallene kommer fra syntetisk korpus og ingen modell er valgt. Integrasjonen lot standardene stå: anbefalingen High `blur_scale` 1,1 (fra 1,3) bygger på syntetiske data uten ekte harde negativer og tas inn først med Roberts testsett. Null-analysatorene gjelder. | **Åpen** (krever Robert: modell og ekte data) |
| 4 | Regelspråket er startlisten (OG, ingen ELLER eller negasjon); kodingen er den versjonerte teksten i `suggest_rules::spec`. Kjernen tar imot tall fra 0, og forhåndsvisningen validerer ingenting. Skjermen krever mer (minst ett vilkår, tekst ikke tom, tall minst 1; `conditionsError`), så en tom tallrute aldri blir en regel som treffer alt. Anbefaling til kjernen: avvis 0 som `invalid_condition` (`suggest_rules::validate`, ikke UX-fasens fil). | Skjermvalidering **besluttet**. Kjerneendring: **anbefaling, ikke besluttet** |
| 5 | Mappebeskyttelse og flytting: akseptabelt i V1 (beslutning 21, «borte + ny»). Beskyttelsen forsvinner aldri stille (den står i Innstillinger, `protections()` er uendret), og et flyttet bilde får bare vanlige forslag; sletting skjer fortsatt bare via bekreftet papirkurv og kan angres. Svakheten er at det flyttede bildet er ubeskyttet. Testet i `api/fjell_v1_tests/scope.rs` (`folder_protection_is_a_prefix_and_does_not_follow_a_move`). Anbefaling (krever delte filer i `store` og `pipeline`): når et bilde som var beskyttet blir borte og samme innholdshash dukker opp på ny sti, overfør beskyttelsen eller varsle; og vis antall treff per beskyttelse i Innstillinger (0 treff betyr at mappen ikke finnes lenger). | V1-aksept: **besluttet**. Overføring og treffantall: **anbefaling, ikke besluttet** |
| 6 | Serie vs. visuell duplikat: samme bilde har aldri to kort. Kortprioritet er eksakt, visuell, serie, regel, skjermbilde, uskarpt (`store/cards.rs`), og bilder som er duplikater deltar ikke i serier. Låst av `api/fjell_v1_tests.rs` (alle typer, ingen asset i to kort) og av konformitetsscenariet «samme bilde vises aldri i flere kort» mot både kjerne og mock. | **Besluttet** |
| 7 | Prosess, ikke produkt. UX-fasen har ikke endret noen delt fil; behov er meldt til Integrer. | Ingen handling |
| 8 | Ansiktsmodell er ikke valgt (lisens, størrelse, ONNX) og ingen modell er lastet ned. | **Åpen** |
| 9 | `PersonRow` og `FaceRow` blir i `store/people.rs`; Integrer kan eksportere dem hvis `fjell-ffi` trenger dem. Ingen UX-konsekvens. | Ingen handling |
| 10 | Bilder uten ansikter analyseres på nytt ved hver kjøring med indeksen på. Trenger en delt schema-endring (`face_scan`) før en ekte modell tas i bruk. | **Åpen** |
| 11 | `fjell-ffi` speiler nå V1-flaten (`FFI.md`, paritetstest mot `FjellApi`). Ingen mobilkode. | **Løst** |
| 12 | Øyemodell er ikke valgt. Til da rangeres serier på skarphet, og `series.eyes.unavailable` vises på serieside (SKJERMER del 6, punkt 6). | **Åpen** |
| 13, 14, 16 | Allerede løst (se over). | Løst |
| 15 | `just` er ikke installert her og `fjell-desktop` (GTK) kan ikke bygges her; `desktop`-jobben på `windows-latest` bygger skallet. | Miljø |

De tre punktene etter sammenslåingen:

| # | Avklaring | Status |
|---|---|---|
| 17 | To mappebeskyttelser består: keeper-regelen `ProtectFolder` er en regel som holder bilder helt utenfor forslag (hele stikomponenter, uavhengig av store/små bokstaver), og `protection.folder` er brukerens beskyttelse som krever bekreftelse. Begge bygger nå på hele stikomponenter i bekreftelsesporten. Ett forbehold: `protection.folder` sammenlignes tegn for tegn, så på Windows beskytter «C:\bilder» ikke «C:\Bilder\...» (under-beskyttelse, målt i konformitetstesten «annen skrivemåte»). Anbefaling: sammenlign uavhengig av store/små bokstaver på Windows i `store/protection.rs` (protect-grenens delte fil, ikke gjort her), og samle begge i ett Beskyttelser-bilde i UI. | Å beholde begge i V1: **besluttet**. Case-ufølsom sammenligning og samling: **anbefaling, ikke besluttet** |
| 18 | `InFolder` i forslagsregler er rent prefiks (`suggest_rules/eval.rs`), og regelskanningen hopper over beskyttede mapper med rent prefiks (`store/suggest_rules.rs`). Det er over-beskyttende (flere bilder utelates enn porten ville stoppet), altså trygg retning. Beholdes. | **Besluttet** (trygg retning) |
| 19 | Endret sensitivitet for visuelle duplikater bygger klyngeindeksen på nytt ved neste utregning. Engangskostnad ved en innstilling brukeren sjelden endrer; ingen kodeendring. | **Anbefaling, ikke besluttet** |

### Funn og endringer i UX-fasen

- **Kjernen merker beskyttede bilder.** `Card.protect_flags` og `SeriesMemberView.protect_flags` får bit 16 (`SCOPE_FLAG` i `api/fjell_v1.rs`) for bilder beskyttet via `protection`. Før satte bare mocken den; i den ekte appen ville beskyttede kort ellers blitt vist som vanlige og forhåndsvalgt. Dokumentkommentaren på `protect_flags` i `api/types.rs` (delt) bør nevne bit 16.
- **Konformitet**: `conformance_v1.json` kjøres mot både kjerne og `MockBridge` (SKJERMER del 7). Den avdekket fire avvik i mocken (forhåndsvisning av tom regel, tomme vilkår, tall fra 0, mappematching), som er rettet.
- **Feilkoder i skallet**: V1-feilkoder kom til UI som `core_error` med koden i `detail`. Rettet i integrasjonen: `TauriBridge` (`normalizeError`) løfter kjent kode til `code`; trådformen i skallet er uendret og låst i `v1_kommandolag.rs`.
- **`listAlbums`**: ikke lagt til; utforming for Integrer i SKJERMER del 9.

## V1-fullføring (integrasjon, 2026-11-15)

Seks grener er slått sammen (`models`, `calibrate`, `ux`, `ffi`, `winkit`, `v2plan`) uten konflikter. Rettet i integrasjonen: V1-feilkoder løftes i `TauriBridge`; `<Cards>` nøkles på `kind` i `App.svelte` (hash-skifte mellom korttyper bytter bunke). De to forventet-feilende testene (`#[should_panic]`, `test.fail()`) er fjernet.

Åpne funn (ikke rettet, krever produktvalg eller delte filer):
- `Store::apply_series` beholder lagret keeper også når brukeren ikke valgte den, så øyeanalysen flytter ikke keeper i en allerede lagret serie. Forslag: egen markør for manuelt valgt keeper.
- `set_settings` (sensitivitet) utløser ingen ny gruppering; gjelder fra neste skanning.
- `ProtectScope::Folder` følger mappegrenser, ikke tegnprefiks; Windows-skrivemåte (`C:\bilder` mot `C:\Bilder`) er ikke case-ufølsom.
- `suggest_rules::validate` godtar 0 dager (konformitetsfixturen låser det; snu begge samtidig).
- `listAlbums` mangler (SKJERMER del 9).
- Dokumentkommentaren på `protect_flags` (`api/types.rs`, `types.ts`) bør nevne bit 16.
- Windows-jobbene `v1-mock` og `v1-shell` er ikke kjørt.
