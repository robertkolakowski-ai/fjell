# MVP-arkitektur

Status: besluttet (gren `plan/mvp`). Omfang: eksakte og visuelle duplikater, uskarpe bilder og skjermbilder, swipe-modus, papirkurv, angre, norsk og engelsk.
Prinsipp: færrest mulige bevegelige deler. Ingen ML-modell, ingen vektorindeks og ingen async-runtime i MVP.

## 1. Beslutninger

| # | Valg | Beslutning | Begrunnelse | Forkastet | Konsekvens (kvalitet / tokenbruk) |
|---|---|---|---|---|---|
| 1 | Delt kjerne | **Rust** + UniFFI (genererer Swift og Kotlin) | Én implementasjon på iOS, Android og desktop. Rask hashing uten GC. Samme kode i tester, CLI og app. | Kotlin Multiplatform (svak iOS-historie, tung runtime); separate implementasjoner (tre ganger feil og tre ganger kontekst) | Én logikkbase gir identisk oppførsel. Agenter leser ett språk i `core/`. |
| 2 | Desktop | **Tauri**, men **utenfor MVP**. Kjernen kompileres for desktop fra dag én og brukes via `fjell-cli`. | Tauri bruker Rust-kjernen direkte og bundler ikke Chromium. Mobil er primærmålgruppen. | Electron (Chromium og Node ved siden av Rust); desktop-UI i MVP (tredje UI uten MVP-verdi) | CLI fungerer som testsele og ytelsesmåler uten emulator. |
| 3 | Embeddings og estetikkmodeller | **Ingen i MVP.** Perseptuelle hasher (dHash + pHash, 64 bit hver). Modellvalg utsettes til serier og «beste bilde» (fase etter MVP). Krav da: permissiv lisens, ≤ 30 MB, ONNX. | Visuelle duplikater (samme bilde, ny størrelse eller komprimering) løses av hasher. Modeller gir lisens-, størrelse- og kalibreringsrisiko uten MVP-gevinst. | CLIP-lignende embeddings nå; sky-analyse | Ingen modellfiler, ingen runtime og ingen NNAPI/Core ML-forskjeller å teste. |
| 4 | Vektorindeks | **Ingen.** Multi-index hashing i minnet over 64-bit hasher (8 blokker × 8 bit, Hamming ≤ 6). | 10 000 hasher er 80 kB. Pigeonhole-garanti gir full recall for terskelen. | HNSW i SQLite (passer for embeddings, ikke for biter) | Klynging for 10 000 bilder < 5 s. Ingen indeks å vedlikeholde i databasen. |
| 5 | Indeks | **SQLite** via `rusqlite` (bundled), WAL, én skrivetråd og én lesetilkobling. Migrering via `PRAGMA user_version`. | Allerede bestemt i TEKNIKK.md. Bundled gir lik versjon på alle plattformer. | Egen filbasert indeks; Realm; sqlite-vec | UI leser aldri blokkert av indeksering. |
| 6 | Bildedekoding | **Plattformen dekoder** (PhotoKit, MediaStore/ImageDecoder, filsystem) og leverer en 256 px gråtone/RGBA-miniatyr til kjernen. Kjernen har ingen bildeformat-avhengighet. | HEIC, RAW og Live Photos fungerer uten egne dekodere. Én dekoding per bilde gjenbrukes av hash, uskarphet og skjermbildesjekk. | `image`-crate + libheif i kjernen (lisens, binærstørrelse, RAW-hull) | Dekoding er dominerende kostnad. Én gjennomgang holder 10 min-målet. |
| 7 | Eksakte duplikater | **BLAKE3** over filbytes, filstørrelse som forfilter (hash bare ved lik størrelse). | Rask og kollisjonssikker. Forfilteret sparer I/O. | SHA-256 (tregere); bare metadata-sammenligning (upålitelig) | Færre filer lest. |
| 8 | Uskarphet | **Varians av Laplace** på 256 px gråtone, normalisert mot bildets kontrast. Terskel kalibreres mot golden-korpus. | Billig (< 3 ms) og forklarbar («Uskarpt»-merke). | ML-kvalitetsmodell (se 3) | Ingen modell. Terskel er én konstant i `quality`. |
| 9 | Skjermbilder | **Heuristikk:** plattformflagg eller album «Skjermbilder», oppløsning lik kjent skjermstørrelse, ingen kamera-EXIF, PNG. Minst to av fire signaler gir forslag. | Rimelig presisjon uten modell. Signalene lagres og vises som begrunnelse. | Bildeklassifisering | Deterministisk og enkel å teste med fixtures. |
| 10 | Samtidighet | **std-tråder + rayon.** Ingen tokio. Én bakgrunnsarbeider for indeksering, kjernens API er synkront. | Arbeidet er CPU-bundet. Async gir bare ekstra FFI-kompleksitet. | tokio; Kotlin-coroutines i kjernen | Enklere FFI og enklere feilsøking. |
| 11 | Mobil-UI | **Native tynne UI-er** (SwiftUI, Jetpack Compose). All logikk i kjernen. UI viser kort og sender beslutninger. | Swipe < 100 ms, VoiceOver/TalkBack og gester med knappealternativ kommer best fra native. | Flutter, React Native, KMP Compose (ekstra runtime og a11y-risiko) | UI-filer er små og lette å lese for agenter. |
| 12 | Papirkurv og angre | **Tilstand i indeksen** (`asset.state`: active, trashed, purged), ikke filflytting. Hver handling skrives til `action_log` med `batch_id`. Angre = invers av siste batch i én transaksjon. Fysisk sletting skjer bare ved «Tøm» med eksplisitt bekreftelse og plattformens egen dialog. Ved utløp (30 dager, konfigurerbart) merkes elementer «utløpt» og brukeren varsles. Ingen stille sletting. | Oppfyller reglene «originaler endres aldri», «alt kan angres», «aldri hard sletting uten varsel». Handling er én rad-oppdatering. | Flytte filer til skjult mappe (iOS tillater det ikke); plattformens papirkurv alene (ingen angre per swipe) | Angre er < 5 ms. **Merk:** lagring frigjøres først ved «Tøm». UI viser «kan frigjøres» separat. |
| 13 | Språk | **nb + en** i MVP. `nn` får tom filmal med fallback til nb (avvik fra CLAUDE.md «nynorsk fra dag én», se MVP-PLAN åpent spørsmål 1). Kilde: `i18n/*.json`, generert til `.xcstrings` og Android `strings.xml`. Kjernen returnerer **begrunnelseskoder**, aldri tekst. | Tekst bor kun i UI-laget. Én kilde hindrer divergens. | Tekst i kjernen; manuell vedlikehold per plattform | Pseudo-lokalet `nb` (+30 %) fanger overflyt i test. |
| 14 | Algoritmeversjon | `fingerprint.algo_ver` per rad. Ved endret algoritme analyseres bare rader med gammel versjon på nytt. | Kalibrering vil endre seg. Full reindeksering er dyrt. | Alltid full reindeksering | Sparer batteri og CI-tid. |
| 15 | Taskrunner og CI | **`just`** som eneste inngang, **GitHub Actions** med stifiltre. Se del 5. | Ett sett kommandoer lokalt og i CI. | Make, egne skript per plattform | Agenter trenger bare `just check`. |
| 16 | Telemetri | **Ingen** i MVP. Ytelse måles i test, ikke hos brukere. | On-device først, ingen datainnsamling. | Anonym telemetri | Ingen samtykkeflyt å bygge. |
| 17 | Sletting i plattform | `delete` i adapter, kun med `PurgeTicket`; kjernen har ingen skrivende plattformkall | Oppfyller «aldri hard sletting uten varsel» med typer, ikke konvensjon | Kjerne kaller plattformen | Testbar med `FakeAdapter` |
| 18 | Kilder som forsvinner | `ChangeSet.removed`; avstemming til `purged` (`cause=external`), ikke regnet som «frigjort av Fjell» | Krasj mellom plattformsletting og DB trenger ingen journal | Egen journal | Én mekanisme for to tilfeller |
| 19 | Skjemautvidelser | Kolonner og tabeller i del 3 (protect_flags, album, fail_count, inverse_of m.m.) i migrering 1 | Unngår tidlig migreringskjede før første utgivelse | Separate migreringer per plan | Én eier av skjema |

## 2. Modulgraf for `core/`

Lagdelt og asyklisk. En modul kan bare avhenge av moduler i lavere lag. Regelen håndheves i CI (del 5).

```
L4  api
L3  pipeline
L2  cluster   suggest   actions
L1  store   hash   quality   screenshot
L0  model   (inkl. traits for AssetSource og Clock)
```

| Modul | Lag | Ansvar | Avhenger av | Grensesnitt (hovedpunkter) |
|---|---|---|---|---|
| `model` | L0 | Typer: `AssetId`, `Asset`, `Kind`, `State`, `ReasonCode`, `Hash64`. Traits `AssetSource`, `Clock`. Ingen I/O. | (ingen) | `trait AssetSource { list_changes(since) -> ChangeSet { changed: Vec<AssetMeta>, removed: Vec<SourceId>, next_token }; thumbnail(id) -> Thumb; read_bytes(id) -> Reader }` |
| `store` | L1 | SQLite: skjema, migrering, spørringer, transaksjoner. | model | `Store::open`, `upsert_assets`, `put_fingerprints`, `set_state(batch)`, `next_cards(limit)`, `log_batch`, `pop_batch` |
| `hash` | L1 | BLAKE3 over bytes, dHash og pHash fra miniatyr. Ren funksjon. | model | `content_hash(Reader) -> [u8;32]`, `perceptual(Thumb) -> (Hash64, Hash64)` |
| `quality` | L1 | Uskarphetsscore fra miniatyr. Ren funksjon. | model | `blur_score(Thumb) -> f32` |
| `screenshot` | L1 | Signaler for skjermbilde fra metadata og miniatyr. Ren funksjon. | model | `signals(AssetMeta, Thumb) -> ShotSignals` |
| `cluster` | L2 | Eksakte grupper (lik `content_hash`) og visuelle grupper (MIH + union-find). Ren funksjon på hashlister. | model | `exact_groups(&[(AssetId,[u8;32])])`, `visual_groups(&[(AssetId,Hash64,Hash64)], Params)` |
| `suggest` | L2 | Velger behold-kandidat per gruppe og lager forslag med `ReasonCode` og konfidens. Ren funksjon. | model | `rank_group(&[Member]) -> Ranked`, `suggestions(...) -> Vec<Suggestion>` |
| `actions` | L2 | Papirkurv, gjenopprett, behold, tøm, angre, utløp. Alt som batch. | store, model | `plan(req)`, `commit(plan, acks)`, `restore(ids)`, `keep(ids)`, `set_keeper`, `undo()`, `expire_due()`, `finish_purge(ticket, results)` (detaljer: PLAN-TRYGGHET). `purge` betyr «oppdater indeks etter plattformens svar», ikke selve slettingen |
| `pipeline` | L3 | Skanning, inkrementell oppdatering, parallell analyse, fremdrift, avbrudd. | store, hash, quality, screenshot, cluster, suggest, model | `scan(&dyn AssetSource, Progress) -> ScanReport`, `cancel()` |
| `api` | L4 | Eneste offentlige flate og UniFFI-grensen. Samler pipeline, actions og lesemodeller. | pipeline, actions, store, model | `Fjell::open`, `start_scan(ScanConfig{mode: Quick|Full, priority, deadline})`, `cards(kind, limit, cursor)`, `decide_many(Vec<(CardId, Decision)>)` (internt plan + commit, én batch), `set_keeper`, `begin_purge`/`confirm_purge`/`finish_purge`, `undo`, `home_summary`, `trash_summary`, `settings` |

Kommentarer:
- `Purge` krever typen `Confirmed`, som bare kan lages av `api` etter brukerbekreftelse. Dermed kan ikke hard sletting skje ved en feil.
- Selve slettingen i plattformen er adapterens `delete(ids) -> Vec<(AssetId, Deleted|Cancelled|Failed)>` (egen trait utenfor `AssetSource`, som ikke har skrivemetoder). Kalles bare av UI med `PurgeTicket` fra `api` (beslutning 17).
- Fjernede kilder (`ChangeSet.removed`) avstemmes av `pipeline` via `store` til `state = purged` med `cause = external`, uten å gå via `actions` (beslutning 18).
- Plattformadaptere (PhotoKit, MediaStore, filsystem) ligger utenfor `core/` og implementerer `AssetSource`. Kjernen kjenner ingen plattform.
- L1 og L2 (unntatt `actions`) er rene funksjoner. De testes uten database.

## 3. Datamodell (SQLite)

Alle nøkler er `INTEGER PRIMARY KEY` hvis ikke annet er oppgitt. Tider er Unix-sekunder.

| Tabell | Nøkkel | Hovedkolonner | Indekser og merknader |
|---|---|---|---|
| `asset` | `id` | `source_id TEXT UNIQUE` (plattform-ID eller sti), `kind` (photo, screenshot_hint, video), `created_at`, `mod_token`, `width`, `height`, `bytes`, `mime`, `state` (0 active, 1 trashed, 2 purged), `trashed_at`, `expires_at`, `expired_notified_at`, `protect_flags` (bitmaske: favoritt, delt album, redigert, ansikter reservert) | `idx(state)`, `idx(bytes)`. `mod_token` gir inkrementell skanning. |
| `fingerprint` | `asset_id` (FK, 1:1) | `content_hash BLOB(32)` (null til beregnet), `dhash INTEGER`, `phash INTEGER`, `blur_score REAL`, `shot_signals INTEGER` (bitmaske), `algo_ver`, `fail_count` | `idx(content_hash)`. Bare rader med `algo_ver` < gjeldende analyseres på nytt. |
| `grp` | `id` | `kind` (exact, visual), `keeper_id` (FK asset), `created_at` | Bygges om ved ny klynging for berørte grupper. |
| `grp_member` | `(grp_id, asset_id)` | `rank`, `score REAL` | `idx(asset_id)`. Hvert bilde er medlem av maks én gruppe per `kind`. |
| `suggestion` | `id` | `asset_id`, `kind` (exact_dup, visual_dup, blurry, screenshot), `grp_id` (null for blurry og screenshot), `reason_code`, `confidence REAL`, `status` (pending, accepted, rejected) | `UNIQUE(asset_id, kind)`, `idx(status, kind, confidence DESC)` brukes av `next_cards`. |
| `action_log` | `id` (autoinkrement) | `batch_id`, `ts`, `op` (trash, restore, keep, purge), `asset_id`, `prev_state`, `suggestion_id` (null tillatt), `inverse_of`, `prev_sugg_status`, `cause` (user, expiry, external, undo) | `idx(batch_id)`. Angre leser siste batch og skriver invers i én transaksjon. Aldri slettet av appen. |
| `setting` | `key TEXT` | `value` | Papirkurvfrist (30 dager), terskler, språkvalg, `scan_cursor`. |
| `album` | `id` | `name`, `shared` | Fra adapter. |
| `asset_album` | `(asset_id, album_id)` | - | `idx(asset_id)`. Brukes av konsekvensforhåndsvisning. |

Regler:
- Skjemaversjon i `PRAGMA user_version`. Hver migrering er én SQL-fil med test.
- Ingen kolonne lagrer bildedata utover hasher. Miniatyrer hentes fra plattformen ved behov.
- Originalfiler skrives aldri. Indeksen er eneste skrivemål.
- Migrering 1 inneholder hele skjemaet over, inkl. triggere (`purged` er terminal, `action_log` er append-only, `0 -> 2` blokkert). `upsert_assets` bevarer `state`, `trashed_at`, `expires_at` (beslutning 19).

## 4. Teststrategi og ytelsesbudsjett

### Teststrategi

| Nivå | Hva | Kjøres |
|---|---|---|
| Enhet (L0–L2 rene moduler) | Hash-stabilitet, Hamming-grenser, MIH mot brute-force (må gi lik mengde), keeper-rangering, uskarphet og skjermbilde på fixtures. | Hver PR (`just test`) |
| Egenskapstester (`proptest`) | Vilkårlig handlingsrekke etterfulgt av full angre gir opprinnelig tilstand. Ingen sekvens gir `purged` uten `Confirmed`. Gruppering er uavhengig av rekkefølge. | Hver PR |
| Golden-korpus (~300 genererte bilder + noen få lisensfrie) | Gruppene: kopi, nedskalert, omkodet, beskåret 5 %, uskarp, skjermbilde, og negative par (like motiver, ulike bilder). Porter: visuelt duplikat presisjon ≥ 0,98 og recall ≥ 0,90; uskarphet presisjon ≥ 0,90; skjermbilde presisjon ≥ 0,95. Tall gjelder som startverdier og justeres i dette dokumentet først. | Hver PR (kjernemoduler) |
| Uendret original | Sjekksum av fixture-mappen før og etter full skanning og alle handlinger er lik. | Hver PR |
| Migrering | Hver `user_version` migreres fra forrige og fra tom database til lik skjemadump. | Hver PR |
| Integrasjon (`fjell-cli`) | Full skanning av fixture-bibliotek mot filsystemadapter, deretter handlinger og angre. | Hver PR |
| UI | Swipe og knappealternativ gir samme resultat. VoiceOver/TalkBack-etiketter finnes. Skjermbilder med pseudo-lokale `nb` (+30 %) og `en` uten avkapping. | Native UI-tester ved endring i `app/` |
| i18n | Nøkkelparitet nb/en, ingen hardkodet tekst i UI-filer (lint). | Hver PR |
| Ytelse | Se under. | Se under |

### Ytelsesbudsjett

| Mål | Budsjett | Måling |
|---|---|---|
| Handling (swipe, trash, undo, behold) p95 | **< 100 ms** ende til ende. Kjernekall < 10 ms (sikte < 5 ms). | `perf`-test i `fjell-cli`: 1 000 kall mot database med 50 000 bilder, p95 rapporteres. PR feiler over 10 ms for kjernekall. Ende til ende måles i UI-test på referanseenhet. |
| Neste kort | Forhåndslast 3 miniatyrer. Spørring bruker `idx(status, kind, confidence)`. < 5 ms. | Samme perf-test |
| Første indeksering | **< 10 min per 10 000 bilder**, dvs. < 60 ms veggtid per bilde. | Nattlig kjøring på referanseenhet (mellomklasse-telefon) med syntetisk 10 000-bibliotek. |
| Klynging og forslag (10 000 bilder) | < 5 s | `criterion`-benchmark |
| Kald oppstart | < 1,5 s (åpne database, første kort) | UI-test på referanseenhet |

Budsjett per bilde, antatt 3 effektive kjerner (60 ms veggtid gir 180 ms CPU):

| Steg | Budsjett (CPU) |
|---|---|
| Plattformdekoding til 256 px | ≤ 100 ms |
| Innlesing og BLAKE3 (bare ved lik størrelse) | ≤ 25 ms |
| dHash + pHash | ≤ 2 ms |
| Uskarphet | ≤ 3 ms |
| Skjermbildesignaler | ≤ 1 ms |
| Batchet databaseskriving | ≤ 1 ms |
| Reserve | ≥ 45 ms |

Proxy i CI: kjernens egen beregning (uten dekoding) ≤ 10 ms per bilde på runner. Brudd stopper PR.
Batteri: skanning kjører i bakgrunn bare ved lading og Wi-Fi, og er avbrytbar og gjenopptagbar via `mod_token`.

## 5. Repostruktur, bygg og CI

```
fjell/
  core/                  Cargo-workspace
    crates/fjell-core/   modulene fra del 2 (ett crate, moduler som mapper)
    crates/fjell-ffi/    UniFFI-omslag rundt `api` (ingen logikk)
    crates/fjell-cli/    testsele, perf, filsystemadapter
    testdata/            golden-korpus + generator
  app/ios/               SwiftUI, bruker xcframework
  app/android/           Compose, bruker AAR med .so
  desktop/               tomt i MVP (Tauri senere)
  i18n/                  nb.json, en.json, nn.json (stub)
  tools/                 gen-i18n, gen-bindings, check-layers
  docs/                  spesifikasjon, docs/mvp/
  justfile
  .github/workflows/
```

**Bygg**
- `just check` = fmt + clippy (`-D warnings`) + test + check-layers. `just bench`, `just bindings`, `just i18n`.
- Android: `cargo-ndk` (arm64-v8a, x86_64 for emulator) til AAR. iOS: `cargo` for arm64 og simulator til xcframework.
- Bindinger genereres av UniFFI ved bygg og sjekkes ikke inn.
- Én `Cargo.lock`. Avhengigheter holdes få: `rusqlite`, `blake3`, `rayon`, `uniffi`, `thiserror`, `proptest` (dev), `criterion` (dev).

**CI (GitHub Actions, stifiltre for å spare minutter)**

| Jobb | Utløser | Innhold |
|---|---|---|
| `core` | endring i `core/` eller `i18n/` | `just check`, golden-porter, migrering, `cargo-deny` (lisens og sårbarheter) |
| `layers` | som `core` | `tools/check-layers` feiler på `use` mot høyere lag eller på syklus |
| `android` | endring i `app/android/` eller `core/` | bygg AAR, enhetstester, a11y-lint |
| `ios` | endring i `app/ios/` eller `core/` | bygg xcframework (macOS-runner), enhetstester |
| `i18n` | endring i `i18n/` eller `app/` | nøkkelparitet, hardkodet-tekst-lint, pseudo-lokale |
| `nightly` | tidsplan | 10 000-bilders indekseringsbudsjett, 50 000-bilders handlings-p95 |

**Arbeidsmåte (agentvennlig, tokensparende)**
- Én fase = én gren = én PR (jf. CLAUDE.md). Hver modul i `core/` er én fil på høyst ca. 400 linjer med 10-linjers topptekstkommentar om grensesnitt (`store` og `actions` kan være kataloger med undermoduler).
- Agenter leser bare `api` og berørt modul. Rene moduler kan utvikles og testes parallelt av separate agenter fordi grensene er traits og enkle typer i `model`.
- Endres et designvalg, oppdateres dette dokumentet først.

## Åpne punkter (avklares før kode)
- Kalibrering av terskler (Hamming, uskarphet) mot golden-korpus. Startverdier: Hamming ≤ 6 på både dHash og pHash, uskarphet ved laveste 10 % av normalisert score.
- Referanseenheter for ytelsestester (én iOS, én Android mellomklasse).
- Hvordan «Tøm» skal presenteres siden plattformdialogen styrer sletteflyten (UX, ikke arkitektur). Løst i PLAN-UX-I18N 3.6.
- Er nynorsk lanseringskrav (CLAUDE.md) eller kan `nn` være stub med fallback i MVP?
