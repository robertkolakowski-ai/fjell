# V2-plan: culling, RAW, sammenligning, Lightroom-eksport og mobil

Status: plan (ingen kode). Omfang: kjernen i `core/`, CLI, Tauri-skall og skjermer for fotografer, deretter mobilfasen. Skrevet på samme måte som `docs/v1/V1-PLAN.md`: mål, graf, eierskap per fil, åpne spørsmål, akseptkriterier. Veikartet (`docs/VEIKART.md`) styrer rekkefølgen; denne planen endrer det ikke. Detaljsider: `RAW-OG-PAR.md`, `EKSPORT.md`, `MOBIL.md`.
Bindende regler: `CLAUDE.md` (ingen hard sletting, alt kan angres, originaler endres aldri, appen foreslår, on-device, ingen nettverk, < 100 ms p95, kald oppstart < 1,5 s, WCAG AA, nb + en, én fase = én gren og én PR).

Utgangspunkt (lest fra repoet ved d41f848): V1 har serier, beskyttelse, forslagsregler, personer (ansiktsindeks av som standard) og dashboard i kjernen; skjermene finnes mot MockBridge og `Fjell`. Øyemodell og ansiktsmodell er ikke valgt, terskler er ikke kalibrert mot ekte bilder, og ingenting er verifisert på Windows (Narrator/NVDA, ekte WIC). `fjell-ffi` (UniFFI) speiler MVP-flaten, men **ikke** V1-flaten (V1-PLAN åpent spørsmål 11; `docs/v1/FFI.md` er en plassholder). HEIC og RAW dekodes bare via WIC på Windows og er aldri prøvd mot ekte filer (`mvp/DEKODING.md`). Disse hullene er forutsetninger for V2, ikke noe V2 løser ved siden av.

## 1. Mål

1. **Culling**: fotografen går gjennom en jobb (mappe, serie eller dato) og setter stjerner (0–5), flagg (behold, forkast, ingen) og fargeetikett med hurtigtaster **og** knapper. Dette er brukerens egne vurderinger. Appen foreslår bare (f.eks. «foreslått behold» fra serier) og setter aldri en vurdering uten at brukeren godtar.
2. **Sammenligning**: 2–4 bilder side om side med synkronisert zoom og panorering, også på tvers av serier og mapper (V1 sammenligner bare innenfor en serie, `SeriesCompare`). Velg vinner, stjerner og flagg direkte i visningen.
3. **RAW+JPEG-par**: filer med samme grunnnavn i samme mappe vises og behandles som ett motiv, med synlig valg for «bare JPEG» og «bare RAW».
4. **Ikke-destruktive metadata**: vurderinger lagres i Fjells egen indeks og skrives tilbake bare når brukeren velger det, aldri inn i originalfilen.
5. **Lightroom-eksport**: XMP-sidefiler og et katalogvennlig format (CSV/JSON) uten å endre originaler. Capture One er stretchmål (`EKSPORT.md`).
6. **Mobil** (etter at MVP og V1 er stabile): SwiftUI/PhotoKit og Kotlin/MediaStore på UniFFI-speilet. Ingen mobilkode før da (beslutning 2026-11-11).

Forslag blir fortsatt kort og beslutninger går fortsatt via `actions` (plan, bekreftelse, én batch, angre). Culling endrer ikke dette: en «forkast»-markering er en etikett, ikke en sletting.

## 2. Brukerhistorier (fotograf)

| # | Historie | Kriterier (del 8) |
|---|---|---|
| H1 | Som bryllupsfotograf går jeg gjennom 1 800 bilder og setter stjerner med tastene 0–5, `P` (behold), `X` (forkast) og `U` (fjern flagg), uten å vente på noe. | A1, A2, A14 |
| H2 | Alt jeg kan gjøre med tastene kan jeg gjøre med knapper (tilgjengelighet), og tastene står i F1-hjelpen. | A8 |
| H3 | Jeg angrer en hel økts stjerner og flagg med ett «Angre økt», og enkeltvis med Ctrl+Z. | A3 |
| H4 | Jeg ser «Forkastet: 312 bilder» og kan velge «Flytt forkastede til Nylig slettet». Appen viser konsekvensene (størrelse, beskyttede bilder, RAW+JPEG-par) og krever bekreftelse. Selve flagget sletter ingenting. | A4, A5 |
| H4b | Bilder med «behold» eller 4–5 stjerner kan aldri bli med i en slik plan uten egen bekreftelse (som beskyttelse i V1). | A5 |
| H5 | Jeg velger 3 bilder fra ulike serier og sammenligner dem med synkronisert zoom til 100 %, og «zoom til øyne» når en øyemodell finnes. | A6, A7 |
| H6 | Jeg har RAW+JPEG fra kameraet. Paret er ett motiv; stjerner gjelder begge filene; jeg kan flytte bare JPEG-en til Nylig slettet og beholde RAW. | A9, A10 |
| H7 | Jeg ser hvorfor appen foreslår et bilde som beste i serien (begrunnelsesmerke) og kan overstyre. Appen dømmer ikke. | A11 |
| H8 | Vurderingene mine ligger i Fjells indeks. Originalfilene har uendret sjekksum etter en hel økt. | A12 |
| H9 | Jeg eksporterer stjerner og etiketter som XMP-sidefiler til Lightroom. Jeg ser en forhåndsvisning («412 sidefiler skrives, 3 finnes fra før og flettes») og kan angre eksporten. | A13, A15 |
| H10 | Jeg kan eksportere til en egen mappe i stedet for ved siden av originalene, og få en CSV/JSON med alt. | A13 |
| H11 | (Mobil, senere) Jeg culler på iPhone og Android med samme vurderinger, lagret lokalt på enheten (ingen sky). | A17 |

## 3. Teknisk retning

Forslag til beslutninger som tas i kontraktfasen (del 5). Det som krever produkteiers ja står i del 7.

### 3.1 Vurderinger og metadata (`culling`)

- Ny tabell `rating(asset_id PK, stars 0..5, flag 0 ingen/1 behold/2 forkast, color 0..8, updated_at, source)`. `source` skiller bruker (`user`) fra importert (`xmp_import`) og godtatt forslag (`suggestion`). Ingen rad = ikke vurdert (ulikt «0 stjerner»; UI viser forskjellen).
- Alle endringer logges som batch i `action_log`-mønsteret (`op = rate`), slik at angre er samme mekanisme som i MVP. Hver enkeltendring er en batch, og en økt har i tillegg en overordnet økt-id slik at både Ctrl+Z og «Angre økt» virker. `rating` er brukerens egen mening, ikke avledet data: `face_index_wipe`, reindeksering og `algo_ver`-bytte rører den ikke.
- «Forkast» er **ikke** `state = trashed`. Sletting skjer bare via `actions::plan` bygget fra et filter («alle forkastede i denne jobben»). Planen viser konsekvenser (ny `ConsequenceCode::Rejected`) og krever bekreftelse for behold-flaggede og 4–5-stjerners bilder (ny ack-kategori). V1 la `ProtectedByScope`-porten i `api/fjell_ops.rs` (`needs_ack`) som teknisk gjeld (V1-PLAN «Funn under integrasjonen»); V2-K flytter den inn i `actions::commit` før nye planbyggere kommer til (risiko R7).
- Hurtigtaster hører til `desktop/`; kjernen kjenner bare `rate(ids, Rating) -> batch`. Hver tast har en knapp (CLAUDE.md).

### 3.2 RAW og par (`pairs`, utvidet dekoding)

Detaljer i `RAW-OG-PAR.md`. Kort:
- Par er en egen tabell `asset_pair(pair_id, primary_id, secondary_id, kind)` bygget av skanningen på (mappe, grunnnavn, ulik endelse), ikke en ny `grp.kind` (unngår enda en tabellomskriving av `grp`, jf. 005).
- **Culling og analyse går på det innebygde JPEG-forhåndsbildet i RAW-filen** (TIFF/EXIF-struktur, største embedded preview), ikke full RAW-dekoding. Det gir visning og handlinger innenfor budsjettene og deterministiske hasher uavhengig av WIC-kodek. Full dekoding (100 %-zoom på sensordata, fokusmaske) går via WIC som i dag og kan gi `Unavailable`.
- Hvilke endelser som er RAW er data (én tabell i kjernen), ikke spredt kode.

### 3.3 Sammenligning (`compare`)

- `CompareSet` (2–4 `AssetId`) er en ren lesemodell i kjernen; synkronisert zoom og panorering er UI-tilstand. V1s `SeriesCompare`/`series_compare` beholdes som spesialtilfelle.
- Visningsbilder kommer fra diskcachen (beslutning 21) i skjermstørrelse; 100 %-zoom laster fullbilde på forespørsel og slipper det igjen (minnegrense, R3).
- Tastatur: `1–4` velger bilde, `+`/`-` zoom, piltaster panorerer, `Space` vekslerer; alt har knapp. Skjermleseren sier «Bilde 2 av 3, 4 stjerner, behold».
- Fokusmaske er ren beregning på forhåndsbildet (Laplace i `quality`-stil, ingen modell). «Zoom til øyne» krever øyemodellen (åpent V1-punkt) og leveres bare når den finnes (`Unavailable` ellers, aldri «ingen funn»).

### 3.4 Eksport (`export_xmp`)

Detaljer i `EKSPORT.md`. Eksporten **skriver nye filer** (`.xmp`, CSV/JSON), aldri i originalfilene. To moduser: «til egen mappe» (standard) og «ved siden av originalene» (eget valg, forhåndsvisning, angrebar). Eksisterende `.xmp` leses og flettes (ukjente noder bevares); den gamle kopieres til appens historikkmappe først, slik at angre gjenoppretter den byte-lik. Ingen nettverk, ingen sky.

### 3.5 Mobil

Detaljer i `MOBIL.md`. UniFFI-speilet utvides med V1- og V2-flaten som en **egen fase før** noen mobilkode; adaptere (PhotoKit, MediaStore) implementerer `AssetSource` utenfor `fjell-core`; UI er tynt og native. Ingen iOS-/Android-kode før MVP og V1 er stabile.

## 4. Avhengighetsgraf

```
V2-0 (V1 stabil) ─> V2-K Kontrakt ─┬─> culling ─────┐
                                   ├─> pairs ───────┤
                                   ├─> compare ─────┼─> Integrer V2 ─> ui-culling ─┐
                                   ├─> raw ─────────┤                ui-compare ──┼─> Sluttreview ─> Windows
                                   └─> export_xmp ──┘                ui-export  ──┘
Integrer V2 ─> FFI-speil (V1+V2) ─┬─> mobile/ios ────┐
                                  └─> mobile/android ┴─> Mobil-integrer     (først etter stabil MVP+V1)
```

Regler (som V1): grener avhenger **bare** av kontrakten, aldri av hverandre; én gren og én PR per fase; trenger en gren en endring i en delt fil, meldes det til Integrer (del 7). `compare` og `export_xmp` leser par og vurderinger bare via kontraktens `model`-typer og `store`-metoder, så alle fem kjernegrener kjører parallelt. Integrasjonsrekkefølge: `pairs`, `culling`, `raw`, `compare`, `export_xmp`.

| Fase | Innhold | Avhenger av | Gren |
|---|---|---|---|
| V2-0 | Forutsetninger: V1 sluttreviewet, terskler kalibrert eller bevisst utsatt, modellvalg avgjort eller utsatt, Windows-verifisering av V1-skjermene, golden-tester per forslagstype | V1 | utenfor denne planen |
| V2-K | Kontrakt (del 5) | V2-0 | `v2/contract` |
| V2-A | `culling` | V2-K | `v2/culling` |
| V2-B | `pairs` | V2-K | `v2/pairs` |
| V2-C | `compare` | V2-K | `v2/compare` |
| V2-D | `raw` (forhåndsbilde, utvidet dekoding) | V2-K | `v2/raw` |
| V2-E | `export_xmp` | V2-K | `v2/export` |
| V2-I | Integrer: `Fjell`, pipeline, skall, CLI, bridge | A–E | `v2/integrate` |
| V2-U | Skjermer: Culling, Sammenligning, Eksport (kan starte mot MockBridge etter V2-K) | V2-K (merge etter V2-I) | `v2/ui-culling`, `v2/ui-compare`, `v2/ui-export` |
| V2-R | Sluttreview: `code-review` mot hele V2-diffen og full `just check` | V2-I, V2-U | `v2/review` |
| V2-W | Windows-verifisering (M9-stil): ekte RAW, Narrator/NVDA, ytelse | V2-R | `v2/windows` |
| V2-M0 | FFI-speil for V1- og V2-flaten | V2-I | `v2/ffi` |
| V2-M1/M2 | iOS og Android, parallelt | V2-M0 + inngangskriterier i `MOBIL.md` | `mobile/ios`, `mobile/android` |

## 5. Kontrakten (V2-K) og eierskap per fil

**Kontrakten eier alle delte filer** og er ferdig før grenene starter. Grenene endrer dem ikke.

Delt (låst for grenene): `model/mod.rs`, `model/v2.rs` (ny), `model/plan.rs`, `store/006_v2.sql` (ny), `store/migrate.rs`, `store/migrate_tests.rs`, `store/ops_tests.rs`, `store/mod.rs`, `actions/` (ack-porten, se 3.1), `api/mod.rs`, `api/types.rs`, `api/mock.rs`, `lib.rs`, `tools/check-layers/layers.toml`, `i18n/nb.json`, `i18n/en.json`, `docs/mvp/ARKITEKTUR.md`, `docs/VEIKART.md`, `docs/v2/V2-PLAN.md`, `fjell-ffi/src/types.rs`, `desktop/src/bridge/types-v2.ts`, workspace-`Cargo.toml` og `Cargo.lock`.

Kontraktens innhold:
1. **model**: `Rating { stars: u8, flag: Flag, color: Color }`, `Flag`, `Color`, `RatingSource`, `PairId`, `PairKind`, `PairView`, `CompareSet`, `ExportSpec`/`ExportPlan`/`ExportReport`, `ExportTarget { OwnFolder(path), BesideOriginals }`, `RawKind`, trekket `PreviewSource` (`embedded_preview(asset) -> Result<Preview, Unavailable>`; `Unavailable` er aldri «ingen funn»). Nye `ReasonCode`/`ConsequenceCode` (f.eks. `Rejected`, `RatedKeep`, `PairHalf`) med i18n i nb og en.
2. **store**: migrering `006_v2.sql`: `rating`, `asset_pair`, `export_job`, `export_file(job_id, path, prev_copy_path, created)` (angre-grunnlag), `fingerprint.preview_source`, `setting`-nøkler. `action_log.op` utvides med `rate` og `export` via trygg tabellomskriving (som 005: `foreign_keys` av utenfor transaksjonen, `foreign_key_check` før commit). «Fra tom» og «fra forrige» gir lik dump. Triggere og I1–I16 urørt; `rating` har ingen bildedata eller fritekst (`color` er en kode).
3. **api**: `FjellApi`-metoder med standardimplementasjon (tom verdi eller `ApiError::Internal("ikke implementert")`): `rate`, `ratings`, `rating_undo_session`, `rejected_plan`, `pairs`, `pair_choose`, `compare_set`, `export_preview`, `export_run`, `export_undo`. `MemApi` og `Fjell` uendret oppførsel til Integrer.
4. **layers.toml**: nye L2-moduler `culling`, `pairs`, `compare`, `export_xmp` (bruker bare `model` og `store`, ikke hverandre). RAW-adapter: egen krate `fjell-raw` (`[crates] fjell-raw = ["fjell-core"]`, `[crate_core_modules] fjell-raw = ["model"]`) eller utvidelse av `fjell-fs` (spørsmål 3). ARKITEKTUR del 2 oppdateres først.
5. **Avhengigheter**: kontrakten låser hvilke nye crater som kommer (TIFF/EXIF for forhåndsbilde, XML for XMP). `core/Cargo.lock` har ingen av dem i dag. Hver må passere `cargo deny check` (tillatt i `deny.toml`: MIT, Apache-2.0, BSD, ISC, Zlib, CC0, Unicode; LGPL/GPL er ikke tillatt) og `--locked`. Lisens og størrelse skrives i kontraktens PR; ingen versjon er sjekket her (nettverk begrenset, ikke verifisert).
6. **Bridge**: `types-v2.ts` og samme metoder i `core-bridge`, `tauri-bridge`, `mock-bridge`; DTO-er i skallet (`dto_v2.rs`).
7. **i18n**: navnerom `culling`, `compare`, `export`, `pairs`; `gen_i18n --check` grønn; nb testes for 15–30 % lengre tekst.

| Gren | Eier (hele mappen/filen) | Bruker fra kontrakten |
|---|---|---|
| `culling` | `src/culling/`, `store/culling.rs` | `Rating`, tabell `rating`, `op = rate` |
| `pairs` | `src/pairs/`, `store/pairs.rs` | `PairId`, `PairKind`, `asset_pair`, RAW-endelsestabell |
| `compare` | `src/compare/`, `store/compare.rs` | `CompareSet`, `PairView`, `Rating` (lesing) |
| `raw` | `core/crates/fjell-raw/` (ny) eller `fjell-fs/src/raw*.rs`; `core/testdata/raw/` (syntetiske TIFF-fixtures, ingen ekte RAW sjekkes inn før lisens er avklart) | `PreviewSource`, `Preview`, `RawKind` |
| `export_xmp` | `src/export_xmp/`, `store/export_xmp.rs`, `core/testdata/xmp/` | `ExportSpec`, `ExportPlan`, `export_job`, `export_file` |
| Integrer | `api/fjell.rs`, `api/fjell_ops.rs`, `api/fjell_v2.rs` (+ tester), `pipeline/`, `fjell-cli`, `fjell-ffi`, `desktop/`, `testdata/`, `Settings`-lagring | alt over |
| UI-faser | `desktop/src/screens/Culling*.svelte`, `Compare*.svelte`, `Export*.svelte`, tester og `e2e/` per skjerm, jf. eierskap i `docs/v1/SKJERMER.md` | bridge og i18n |

Hver modul får modulhode («Avhenger av:», «Grensesnitt:»), en smoketest i `src/<modul>/tests.rs` og et skjelett i `store/<modul>.rs` (som V1). Nye filer i egen mappe er fritt fram.

## 6. Slik startes neste orkestrering

1. `v2/contract` (én agent, serielt): del 5. Ferdig når kontrakt-DoD (del 8) er grønn.
2. Fem parallelle spesialister `v2/culling`, `v2/pairs`, `v2/compare`, `v2/raw`, `v2/export`, hver med eierskap fra tabellen, test først. Spørsmål merket **F** i del 7 må være avgjort.
3. `v2/integrate` (én agent): merge i rekkefølgen pairs, culling, raw, compare, export_xmp; kobling til `Fjell`, pipeline, skall, CLI; `api/fjell_v2_tests.rs` (del 9).
4. Tre parallelle UI-spesialister, deretter `v2/review` og `v2/windows`.
5. Mobil er **utenfor** denne orkestreringen og starter først når inngangskriteriene i `MOBIL.md` er oppfylt.

## 7. Åpne spørsmål

**F** = må avgjøres før grenen starter; **U** = kan avgjøres underveis.

1. **F, produkt: skal «behold»-flagg og stjerner gi beskyttelse?** Forslag: «behold» og 4–5 stjerner gir egen ack-kategori i planen (som `ProtectedByScope`), uten å være en `protection`-rad. Alternativ: ekte `protection`-rader. Påvirker `needs_ack` og dashboardtall.
2. **F, produkt: skal «Flytt forkastede» finnes i V2?** Fotografer vil ha den, men den er den farligste knappen i appen. Forslag: ja, men bare som vanlig plan med bekreftelse, aldri som ett trykk.
3. **F, arkitektur: egen `fjell-raw`-krate eller utvidelse av `fjell-fs`?** `fjell-fs` eier dekoding i dag (`thumb`, `wic.rs`). En egen krate gir ren lisens- og avhengighetsgrense, men krever ny rad i `layers.toml` og ARKITEKTUR del 2.
4. **F, lisens: hvilken crate trekker forhåndsbilde fra RAW og leser/skriver XMP?** Kandidater må være MIT/Apache/BSD (`deny.toml`). LibRaw og rawloader/rawler har LGPL-vilkår og er ikke brukbare uten vedtak. Reserve: egen minimal TIFF-leser (noen hundre linjer, godt testbar). Ikke verifisert (ingen nett).
5. **F, produkt: RAW-formater i første utgave.** Forslag: DNG, CR2, NEF, ARW, ORF, RW2 (TIFF-baserte) først; CR3 (ISO-BMFF) og RAF har annen konteiner og krever egen leser. `WIC_EXTENSIONS` dekker alle i dag.
6. **U: hvilken fil eier sidefilen i et par?** Lightroom forventer `bilde.xmp` ved siden av RAW. Forslag: RAW eier, JPEG får ingen. Må verifiseres mot Lightroom (ikke verifisert).
7. **U, XMP: flagg.** `xmp:Rating` (−1 = avvist, 0–5) og `xmp:Label` er standardisert; behold-flagget bor i Lightrooms katalog og har ingen sikker XMP-form. Planen lover ikke at flagg overføres før det er testet i Lightroom Classic/Bridge.
8. **U: Capture One.** Egen katalog; leser trolig `xmp:Rating` ved import (ikke verifisert). Stretchmål.
9. **F, sikkerhet: eksport ved siden av originalene skriver nye filer i brukerens mapper.** Forslag: tillatt, men aldri over eksisterende fil uten kopi til historikk (angre), og alltid med forhåndsvisning. Bekreft at dette er forenlig med «originalfilers innhold endres aldri» (bildefilene røres ikke; sidefiler er egne filer).
10. **U: par-signal.** Samme mappe og grunnnavn er nok; tidsstempel bare som tilleggssignal ved kollisjon (to kameraer, samme filnummer). Flertydige tilfeller (CR2 + DNG, to JPEG) danner ikke par.
11. **U**: video og Live Photo-par er utenfor V2.
12. **F, mobil: synk av vurderinger mellom PC og telefon?** Nettverk/sky bryter «on-device». Forslag: nei i V2; fil-basert eksport/import (JSON) hvis ønsket.
13. **U: serieterskler for rask serieopptak.** `serieglipp 10 s` (V1) kan være feil for sport og burst. Krever etikettert ekte korpus (som K8).
14. **U: ytelse ved 250 000+ RAW.** Målt bare på Linux (`mvp/YTELSE-STORT.md`); se R3.
15. **U: Windows RAW-kodek.** WIC-RAW krever Microsoft «Raw Image Extension»; appen har ingen nettverkstilgang og kan ikke installere den. Forhåndsbilde-stien reduserer avhengigheten, men 100 %-zoom trenger kodeken.
16. **F: reanalyse ved ny forhåndsbildekilde.** Hash av forhåndsbilde er ikke lik hash av WIC-dekodet RAW. Forslag: `fingerprint.preview_source` lagres, og endret kilde utløser reanalyse (beslutning 14).
17. **F: ack-porten.** Skal flyttes fra `api/fjell_ops.rs` til `actions::commit` i V2-K (R7). Bekreft at `actions/` da er kontraktens (delte) fil.

## 8. Akseptkriterier

Kontrakten (V2-K) er ferdig når:
- `cargo fmt --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`, `cargo test --workspace --locked`, `cargo test` i `core/testdata`, `check-layers` (med selvtest), `cargo deny check` og `python3 tools/gen-i18n/gen_i18n.py --check` er grønne.
- Migrering 6 gir lik dump fra tom og fra versjon 5, bevarer data og triggere, og slår `foreign_keys` på igjen.
- Alle modulene finnes (modulhode, tom API, smoketest) og er registrert i `lib.rs`, `layers.toml` og ARKITEKTUR del 2.
- `FjellApi` har alle V2-metodene med standardimplementasjon; `MemApi` og `Fjell` oppfører seg som før.
- Hver ny avhengighet er listet med lisens og består `cargo deny check` og `--locked`.

Hver gren er ferdig når alt den eier er implementert test først, kontrollene er grønne (enkeltkommandoer der `just` mangler), ingen delt fil er endret, ingen CLAUDE.md-regel er brutt og modulhodet er oppdatert.

V2 er ferdig når følgende er oppfylt og dokumentert:

| # | Kriterium | Test |
|---|---|---|
| A1 | Å sette stjerne, flagg eller etikett på ett bilde tar < 100 ms p95 (kjernen, inkl. skriving) | Ytelsestest på 100 000 rader, `fjell-cli perf`-mønster |
| A2 | Neste bilde i culling (forhåndsbilde fra cache) < 100 ms p95; kald oppstart < 1,5 s med V2-tabellene | Kjernetest og e2e; Windows ikke verifisert før V2-W |
| A3 | Hver vurdering og hele økter kan angres; angre gir lik tilstand som før, rad for rad | Egenskapstest: tilfeldige sekvenser av `rate` og `undo` |
| A4 | Flagget «forkast» endrer aldri `asset.state` og flytter ingen fil | Test: etter 1 000 forkast-flagg er alle `state = active` og filsystemet uendret |
| A5 | Planen for forkastede krever ack for «behold» og 4–5 stjerner og viser konsekvenser; ingen automatisk sletting finnes | Test i `fjell_v2_tests.rs` (som V1-sikkerhetshullet) |
| A6 | Sammenligning av 2–4 bilder på tvers av serier og mapper; vinner kan velges; kan angres | Kjernetest og vitest |
| A7 | Synkronisert zoom og panorering er jevn (60 fps-mål) og har knappealternativ | e2e (Chromium her; WebView2 ikke verifisert) |
| A8 | Alle hurtigtaster har knapp; WCAG AA; skalering 100–200 %; høykontrast; Narrator/NVDA | `npm run check/test/contrast`, axe, e2e; Narrator/NVDA **ikke verifisert** før V2-W |
| A9 | RAW+JPEG-par dannes riktig (samme mappe og grunnnavn; ikke på tvers av mapper; flertydige danner ikke par) og er idempotente ved ny skanning | Golden-test på syntetisk mappe |
| A10 | Beslutning på et par gjelder begge filer; «bare JPEG» og «bare RAW» virker og kan angres | Kjernetest |
| A11 | Hvert forslag og hver foreslått vurdering har begrunnelsesmerke og settes aldri uten brukerens godkjenning | Test: ingen `rating` med `source = suggestion` uten brukerhandling |
| A12 | Sjekksum, størrelse og endringstid på originalene er uendret etter økt, eksport og angre | Sjekksumtest som i `fjell_v1_tests.rs` |
| A13 | Eksport lager gyldige XMP-sidefiler (parsebare, riktig `xmp:Rating` og `xmp:Label`) og CSV/JSON med alle vurderinger, uten å endre originalbilder | Golden-XMP, rundtur (skriv, les, lik) |
| A14 | 1 800 bilder kan culles uten at UI blokkeres; indeksering skjer i bakgrunnen med lav prioritet | Ytelses- og e2e-test |
| A15 | Eksport kan forhåndsvises og angres; eksisterende sidefiler gjenopprettes byte-lik | Skriv, flett, angre, sammenlign sjekksum |
| A16 | Ingen nettverkskall, ingen eksterne fonter eller CDN; ingen bilder forlater enheten | `nonet`-testene fra V1 utvidet |
| A17 | (Mobil) samme vurderinger og begrunnelseskoder via UniFFI; tynt UI | Rundtur-test mot speilet; ikke verifisert før mobilfasen |

## 9. Testplan

Test først; aldri svekke eller hoppe over tester for å få grønt.

- **Kjerne (Linux)**: enhetstester per modul, egenskapstester for angre (A3), golden-tester for par og forslag (`testdata`), migreringstester (fra tom = fra v5), sjekksum av originaler (A12), `check-layers` med selvtest, `cargo deny check`.
- **Integrasjon**: `api/fjell_v2_tests.rs`: skann et syntetisk korpus med RAW+JPEG-par (TIFF-fixtures med innebygd forhåndsbilde), sett vurderinger, bygg «forkastet»-plan, krev ack, eksporter XMP, angre alt, sammenlign databasedump og filsystem med utgangspunktet. `fjell-cli`: `rate`, `pairs`, `export` med tester; ingen kommando sletter bilder.
- **Ytelse**: `fjell-cli perf` utvidet med 250 000 rader og `rating`-skriving; p95 mot A1/A2; merkes «ikke målt på Windows».
- **Frontend**: `npm run check`, `npm test`, `npm run contrast`, `npm run build`, axe, e2e (Playwright/Chromium) per skjerm, skalering 100–200 %, høykontrast, pseudo-lokalisering og nb-tekstlengde, nonet.
- **XMP**: golden-filer: ingen eksisterende sidefil, eksisterende fra Lightroom med ukjente noder (bevares), korrupt XML, skrivebeskyttet, lang sti, Unicode. Kompatibilitet med ekte Lightroom/Bridge er **ikke verifisert** her og er manuell Windows-test (V2-W).
- **Windows (ikke verifisert her)**: WIC-RAW, Narrator/NVDA, WebView2-ytelse, kald oppstart, minnebruk for 100 %-zoom på 45+ MP, bygg av `fjell-desktop` (kan ikke kompileres på Linux; `windows-latest` gjør det). Oppskrift: `mvp/WINDOWS-TEST.md` og `mvp/E2E-WINDOWS.md`.

## 10. Risiko

| # | Risiko | Konsekvens | Tiltak |
|---|---|---|---|
| R1 | **RAW-dekoding**: WIC-kodek varierer per maskin og kamera; CR3/RAF har egne konteinere; ingen ekte RAW er prøvd | Visning mangler eller hash drifter | Innebygd forhåndsbilde som standard med `Unavailable` og forklaring, aldri feil; fixtures per format; kamerastøtte som data |
| R2 | **Lisens**: RAW-biblioteker (LibRaw, rawloader) har LGPL/GPL eller egne vilkår; `deny.toml` tillater dem ikke | Kan ikke bygges eller distribueres | Lisens listes i kontrakten før avhengighet legges til; egen minimal TIFF-leser som reserve; lisenstekster kan ikke sjekkes uten nett her (ikke verifisert) |
| R3 | **Ytelse og minne for store bibliotek**: 250 000+ RAW, 45+ MP fullbilder, 100 %-zoom i flere ruter | Brudd på 100 ms / 1,5 s; RSS over budsjett (dagens 250 000-måling var ca. 280 MB, kun Linux) | Forhåndsbilde og diskcache for alt unntatt eksplisitt 100 %-zoom; fullbilde lazy og kastes; grense på samtidige fullbilder; batchet skriving; måling på referansemaskin før lansering |
| R4 | **XMP-kompatibilitet**: Lightroom leser ikke endrede sidefiler automatisk; flagg er katalogdata | Brukeren tror stjerner er overført | Ærlig eksport-UI («Slik leser du dem inn»); bare verifiserte felt lovet; CSV/JSON som reserve |
| R5 | **Ødelagt eller overskrevet sidefil** fra annet program | Tap av brukerens Lightroom-metadata | Flett, aldri erstatt uten kopi til historikk; forhåndsvisning; atomisk skriving; angre |
| R6 | **«Forkast» oppfattes som sletting**, eller «Flytt forkastede» blir ett trykk | Tap av uerstattelige bilder; tillitsbrudd | Plan, konsekvensvisning og ack for behold/høy stjerne; flagget flytter aldri filer (A4); UI sier «markert», ikke «slettet» |
| R7 | **Ack-porten ligger utenfor `actions::commit`** (V1-gjeld) | Nye planbyggere kan omgå den | V2-K flytter porten inn i `actions::commit` med egen ack-kategori og test før nye planbyggere |
| R8 | **Par feilkoblet** (to kameraer, samme filnummer; omdøpte filer) | Beslutning treffer feil fil | Samme mappe + grunnnavn + tidssignal; paret viser alltid begge filnavn; «Del opp paret» er angrebar |
| R9 | **V1-hull**: ingen valgt øye-/ansiktsmodell, ukalibrerte terskler, V1 ikke Windows-verifisert, FFI mangler V1-flaten | V2 bygger på sand | V2-0 er eksplisitt forutsetning; øyefunksjoner leveres bare når modell finnes |
| R10 | **Omfang**: culling, RAW, sammenligning, eksport og mobil | Én-fase-én-PR brytes; forsinkelse | Grafen i del 4; mobil er egen etappe; kutt først fokusmaske og «zoom til øyne»; Capture One er stretchmål |
| R11 | **Mobil**: batteri/varme, PhotoKit/MediaStore-begrensninger, plattformens egen slettedialog | Dårlig opplevelse; «alt kan angres» kan ikke holdes likt | Se `MOBIL.md` del 4; ikke i V2-kjernen |

## 11. CLAUDE.md-regler som blir mest utsatt

| Regel | Hvor presset oppstår | Vern i planen |
|---|---|---|
| **Aldri hard sletting; alt kan angres** | «Flytt forkastede» og masse-handlinger på 1 000+ bilder; «bare JPEG» i et par; mobilplattformenes egen slettedialog | Alltid via `actions::plan` til Nylig slettet, bekreftelse, angre per batch (A3–A5); mobil: `MOBIL.md` |
| **Originalfilers innhold endres aldri** | Tilbakeskriving av metadata; XMP ved siden av originalene | Ingen embedded skriving i JPEG/DNG; sidefiler er nye filer; gammel sidefil kopieres til historikk; sjekksumtest (A12, A15) |
| **Appen foreslår, dømmer ikke** | Auto-flagging av «beste» og «forkast»; auto-stjerner | Foreslåtte vurderinger har begrunnelsesmerke og settes aldri uten godkjenning (A11) |
| **On-device, ingen nettverk** | Synk mellom PC og telefon, sky-RAW, Lightroom-sky | Ingen synk; eksport er lokal fil; nonet-tester (A16, spørsmål 12) |
| **< 100 ms p95, kald oppstart < 1,5 s** | RAW-forhåndsbilder, 250 000 rader, flere 100 %-bilder | Embedded preview, diskcache, lazy fullbilde, batchet skriving (A1, A2, R3) |
| **WCAG AA, Narrator/NVDA, 100–200 %, høykontrast; hurtigtaster har knapp** | Culling er tastaturtungt; sammenligning er peker- og zoomtungt | Alle taster har knapp; skjermleserbeskrivelse per bilde; zoom og panorering har knapper; e2e per skjerm (A7, A8) |
| **Én fase = én gren og én PR; ikke rør filer utenfor omfanget** | Fem grener over delte filer (`FjellApi`, `layers.toml`, i18n, migrering, `actions`) | Kontrakten eier delte filer; Integrer samler (del 5) |
| **Ingen iOS-/Android-kode i MVP** (beslutning 2026-11-11) | Fristelse til å starte mobil parallelt | Inngangskriterier i `MOBIL.md`; FFI-speilet først |
| **nb + en (nb 15–30 % lengre)** | Mange korte etiketter (taster, flagg) | Egne navnerom, pseudo- og nb-lengdetest |

## 12. Klar til å starte V2-kontrakten når

- V1 er sluttreviewet uten åpne funn, og tersklene er kalibrert eller bevisst utsatt med dokumentert begrunnelse.
- Spørsmål merket **F** i del 7 er avgjort og ført inn her.
- Valg av crater (lisens og størrelse) er godkjent mot `deny.toml`.
- Produkteier har bekreftet at V2-omfanget er riktig (Pro: «AI-culling, serier, regler, RAW, Lightroom-eksport», jf. forretningsmodell i `VEIKART.md`).
