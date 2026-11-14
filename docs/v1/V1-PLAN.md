# V1-plan: serier, beskyttelse, personer, regler og dashboard

Status: kontrakten er lagt (commit «V1-kontrakt: ...»). Omfang: kjernen i `core/`. Desktop-UI og i18n-strenger utover begrunnelsesmerkene følger i egne faser.
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
`model/mod.rs`, `model/v1.rs`, `model/analyze.rs`, `model/plan.rs`, `store/002_v1.sql`, `store/migrate.rs`, `store/migrate_tests.rs`, `store/ops_tests.rs`, `store/mod.rs`, `api/mod.rs`, `api/types.rs`, `api/mock.rs`, `lib.rs`, `tools/check-layers/layers.toml`, `i18n/nb.json`, `i18n/en.json`, `docs/mvp/ARKITEKTUR.md`, `docs/v1/V1-PLAN.md`, `fjell-ffi`-speilene (`types.rs`).

| Gren | Eier (hele mappen/filen) | Bruker fra kontrakten |
|---|---|---|
| `protect` | `src/protect/`, `store/protection.rs`, `src/actions/plan.rs` (bare de delene som legger til `ConsequenceCode::ProtectedByScope`/`ProtectCounts.scope`; ingen annen gren rører `actions/`) | `ProtectScope`, `ProtectionEntry`, tabell `protection` |
| `series` | `src/series/`, `store/series.rs` | `SeriesId`, `EyeAnalyzer`/`EyeReport`, `ReasonCode::{SeriesMember, KeeperBestOverall, KeeperEyesOpen, ClosedEyes}`, `ReasonDetail::Position`, `SeriesCompare`, tabellene `grp` (kind `series`), `eye_result` |
| `people` | `src/people/`, `store/people.rs` | `PersonId`, `FaceAnalyzer`/`FaceObservation`/`Embedding`, `PersonView`, tabellene `person`, `face_embedding`, `setting.face_index_enabled` |
| `rules` | `src/rules/`, `store/rules.rs` | `RuleId`, `RuleSpec`/`RuleCondition`, `RuleDraft`/`RuleView`/`RulePreview`, `ReasonCode::Rule`, tabellene `rule`, `rule_hit` |
| `dashboard` | `src/dashboard/`, `store/dashboard.rs` | `DashboardData` og delvisninger |
| Integrer | `api/fjell.rs`, `api/fjell_ops.rs` (+ tester), `pipeline/` (kjør analysatorer og gruppering), `fjell-cli`, `fjell-ffi`, `desktop/`, `testdata/`, `Settings`-lagring | alt over |

Hver av de fem modulene har en smoketest i `src/<modul>/tests.rs` og et skjelett i `store/<modul>.rs` (en `Store`-metode med test). Grenen utvider disse; den lager ikke nye delte filer. Nye filer i egen mappe er fritt fram.

Lagdeling (`tools/check-layers`): de fem modulene er L2, kan bruke `model` og `store`, men ikke hverandre eller `actions`/`suggest`/`cluster`. Trenger en gren logikk fra en annen L2-modul, løftes den til `model` av Integrer eller dupliseres bevisst.

## 4. Kontraktens innhold

- **model**: `SeriesId` (= `grp.id` med `kind = 'series'`), `PersonId`, `RuleId`, `ProtectScope` (kode og mål-koding for `protection`), `ReasonDetail`, `RuleSpec`/`RuleCondition` (tom regel treffer ingenting), `DashboardData`; `EyeAnalyzer`, `FaceAnalyzer` med `NullEyeAnalyzer`/`NullFaceAnalyzer` (svarer `Unavailable`, aldri «ingen funn»); `Embedding` (LE-bytes). `SuggestionKind::{Series, Rule}` (`ALL` = 6), 6 nye `ReasonCode` (`ALL` = 18) med i18n i nb og en, `ConsequenceCode::ProtectedByScope`, `KindCounts.{series, rule}`, `ProtectCounts.scope`. `Card.detail: ReasonDetail`.
- **store**: migrering `002_v1.sql` (001 uendret): `grp.kind` og `suggestion.kind` utvidet via trygg tabellomskriving (`foreign_keys` av utenfor transaksjonen, `foreign_key_check` før commit, ingen kaskade; testet med data), `protection`, `eye_result`, `person` (`merged_into`), `face_embedding` (BLOB, kun lokal), `rule` (`deleted_at`, trigger mot hard sletting), `rule_hit`, `setting.face_index_enabled = '0'`. «Fra tom» og «fra forrige» gir lik dump (K6-1). Triggere og I1-I16 urørt. I16 er utvidet bevisst, se åpne spørsmål.
- **api**: `FjellApi` har alle V1-metodene med standardimplementasjon (tom verdi eller `ApiError::Internal("ikke implementert")`): `protect_set`, `unprotect`, `protections`, `rules_list`, `rule_save`, `rule_preview`, `rule_toggle`, `rule_delete`, `rule_restore`, `people_list`, `person_name`, `person_merge`, `person_unmerge`, `person_protect`, `face_index_enabled`, `face_index_set`, `face_index_wipe`, `dashboard`, `series_compare`, `thresholds`. `rule_delete` og `person_merge` er angrebare (`rule_restore`, `person_unmerge`). `MemApi` og `Fjell` uendret oppførsel.
- **Settings**: `trash_days`, `sensitivity: Sensitivities` (per `SuggestionKind`, standard `Normal`), `face_index: bool` (standard av). Nøkler i `setting`: `sensitivity.<kind>` og `face_index_enabled`; manglende nøkkel gir standardverdi (eldre databaser leses uendret). Lagring i `Fjell` er Integrers jobb.

## 5. Åpne spørsmål

1. **I16 utvidet**: `person.name`, `rule.name`, `rule.spec` er brukerens egne etiketter (første fritekst i indeksen), og `face_embedding.embedding` er første BLOB utenom `content_hash`. Begge er listet eksplisitt i `i16_schema_has_no_image_data_or_free_text`. Bekreft at dette er akseptert, eller krev kryptering/ekskludering fra eksport.
2. **`face_index_wipe`** sletter embeddings og personer (avledet data, kan gjenskapes) og er ikke angrebar. Det er et eksplisitt personvernvalg med varsel i UI. Beskyttelser av typen `person` blir hengende (hvilende) etter wipe; bør UI varsle om det? Anbefaling: bevar radene, vis varsel.
3. **Terskler**: `Thresholds`-standardene (Hamming 6, blur 0,006, serieglipp 10 s, øyesikkerhet 0,8) er startverdier. Sensitivitet `Low/Normal/High` skal kalibreres mot golden-korpus og dokumenteres her før Integrer; modellvalg for øyne og ansikter (lisens, ≤ 30 MB, ONNX, ARKITEKTUR beslutning 3) er ikke tatt. Til da brukes Null-analysatorene.
4. **Regelspråk**: `RuleCondition` er en startliste (skjermbilde, uskarp, eksakt duplikat, alder, størrelse, album, mappe, MIME). Kodingen til `rule.spec` velges av `rules`-grenen (stabil, versjonert tekst). Mer uttrykkskraft (ELLER, negasjon) venter.
5. **Mappebeskyttelse og flytting**: `Folder(prefiks)` matcher `source_id`-prefiks. Flytting utenfor Fjell ses som «borte + ny» (beslutning 21), så beskyttelsen følger ikke med. Akseptabelt i V1?
6. **Serier vs. visuelle duplikater**: samme bilde kan være medlem i både en `visual`- og en `series`-gruppe, og dermed ha to forslag (`UNIQUE(asset_id, kind)` tillater det). Integrer må avgjøre kortrekkefølge og «samme asset vises aldri i flere kort samtidig»-regelen.
7. **Endring i delt fil midt i en gren** (f.eks. flere `ReasonCode`): sendes til Integrer, som samler dem i én PR mot kontrakten.
8. **Ekte ansiktsmodell (people)**: `people` er ferdig mot `FaceAnalyzer`-trekket med `MemFaceAnalyzer` (tester) og `NullFaceAnalyzer` (`Unavailable`, standard). Modell for deteksjon og embedding er ikke valgt (lisens, ≤ 30 MB, ONNX, ARKITEKTUR beslutning 3), og ingen modell er lastet ned. Klyngeterskelen (`PeopleParams.threshold`, cosinus 0,6) er en plassholder som må kalibreres mot golden-korpus når modellen er valgt; embeddingdimensjonen følger modellen. Integrer må i tillegg: (a) kjøre `index_asset` og `cluster_new_faces` i pipeline bare når `face_index` er på, (b) koble `person_protect` til `protect` (`ProtectScope::Person`; `people::person_assets` gir bildene), og (c) la `face_index_wipe` varsle om hengende personbeskyttelser (jf. spørsmål 2).
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
