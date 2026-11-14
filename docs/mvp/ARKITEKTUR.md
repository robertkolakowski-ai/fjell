# MVP-arkitektur

Status: besluttet (gren `plan/mvp`; MVP-plattform byttet til Windows-PC 2026-11-11 på `plan/pc`). Omfang: eksakte og visuelle duplikater, uskarpe bilder og skjermbilder, kortmodus (swipe), papirkurv, angre, norsk og engelsk. **Windows-PC-app (Windows 10/11)**; iPhone, Android og nynorsk kommer etter MVP. Linux/macOS-bygg er bonus, ikke krav.
Prinsipp: færrest mulige bevegelige deler. Ingen ML-modell, ingen vektorindeks og ingen async-runtime i MVP.

## 1. Beslutninger

| # | Valg | Beslutning | Begrunnelse | Forkastet | Konsekvens (kvalitet / tokenbruk) |
|---|---|---|---|---|---|
| 1 | Delt kjerne | **Rust**. Tauri-backend bruker `fjell-core` direkte som Rust-avhengighet. **Ingen UniFFI/`fjell-ffi` i MVP**; UniFFI (Swift, Kotlin) legges til i mobilfasen. | Én implementasjon på PC (app og CLI) og klar for iPhone/Android senere. Rask hashing uten GC. Samme kode i tester, CLI og app. | Kotlin Multiplatform (svak iOS-historie, tung runtime); separate implementasjoner (tre ganger feil og tre ganger kontekst) | Én logikkbase gir identisk oppførsel. Ingen FFI-grense å vedlikeholde i MVP. |
| 2 | Desktop-skall | **Tauri 2** (Rust-backend + WebView2) er MVP-plattformen. Frontend har ingen filsystem-, shell- eller nettverkstilgang (Tauri-capabilities); alle filoperasjoner skjer i Rust bak kommandoer. | Tauri bruker Rust-kjernen direkte og bundler ikke Chromium (WebView2 følger Windows). Liten installasjon. | Electron (Chromium og Node ved siden av Rust); native WinUI (egen UI-stack uten gjenbruk til mobil-design) | `fjell-cli` er fortsatt testsele og ytelsesmåler. WebView2-avhengighet (se 21 og risiko i MVP-PLAN). |
| 3 | Embeddings og estetikkmodeller | **Ingen i MVP.** Perseptuelle hasher (dHash + pHash, 64 bit hver). Modellvalg utsettes til serier og «beste bilde» (fase etter MVP). Krav da: permissiv lisens, ≤ 30 MB, ONNX. | Visuelle duplikater (samme bilde, ny størrelse eller komprimering) løses av hasher. Modeller gir lisens-, størrelse- og kalibreringsrisiko uten MVP-gevinst. | CLIP-lignende embeddings nå; sky-analyse | Ingen modellfiler, ingen runtime og ingen NNAPI/Core ML-forskjeller å teste. |
| 4 | Vektorindeks | **Ingen.** Multi-index hashing i minnet over 64-bit hasher (8 blokker × 8 bit, Hamming ≤ 6). | 10 000 hasher er 80 kB. Pigeonhole-garanti gir full recall for terskelen. | HNSW i SQLite (passer for embeddings, ikke for biter) | Klynging for 10 000 bilder < 5 s. Ingen indeks å vedlikeholde i databasen. |
| 5 | Indeks | **SQLite** via `rusqlite` (bundled), WAL, én skrivetråd og én lesetilkobling. Migrering via `PRAGMA user_version`. | Allerede bestemt i TEKNIKK.md. Bundled gir lik versjon på alle plattformer. | Egen filbasert indeks; Realm; sqlite-vec | UI leser aldri blokkert av indeksering. |
| 6 | Bildedekoding | **Adapteren dekoder** og leverer en 256 px gråtone/RGBA-miniatyr til kjernen (EXIF-orientering brukt). Windows-adapter: `image`-crate for JPEG, PNG, WebP, BMP, TIFF, GIF (deterministisk, JPEG nedskaleres under dekoding); HEIC og RAW via WIC bare når Windows-kodeken er installert, ellers `Unavailable`. Kjernen har ingen bildeformat-avhengighet. | Én dekoding per bilde gjenbrukes av hash, uskarphet og skjermbildesjekk. Eget dekodevalg i adapteren gir stabile hasher på tvers av Windows-versjoner. | `image`-crate + libheif i kjernen (lisens, binærstørrelse, RAW-hull); Windows skall-miniatyrer for analyse (størrelse og kvalitet varierer, hashdrift) | Dekoding er dominerende kostnad. HEIC/RAW er avhengig av Microsoft Store-utvidelser (risiko, MVP-PLAN). |
| 7 | Eksakte duplikater | **BLAKE3** over filbytes, filstørrelse som forfilter (hash bare ved lik størrelse). | Rask og kollisjonssikker. Forfilteret sparer I/O. | SHA-256 (tregere); bare metadata-sammenligning (upålitelig) | Færre filer lest. |
| 8 | Uskarphet | **Varians av Laplace** på 256 px gråtone, normalisert mot bildets kontrast. Terskel kalibreres mot golden-korpus. | Billig (< 3 ms) og forklarbar («Uskarpt»-merke). | ML-kvalitetsmodell (se 3) | Ingen modell. Terskel er én konstant i `quality`. |
| 9 | Skjermbilder | **Heuristikk:** plattformflagg eller album/mappe «Skjermbilder» (PC: mappenavn), oppløsning lik kjent skjermstørrelse, ingen kamera-EXIF, PNG. Minst to av fire signaler gir forslag. | Rimelig presisjon uten modell. Signalene lagres og vises som begrunnelse. | Bildeklassifisering | Deterministisk og enkel å teste med fixtures. |
| 10 | Samtidighet | **std-tråder + rayon.** Ingen tokio. Én bakgrunnsarbeider for indeksering, kjernens API er synkront. | Arbeidet er CPU-bundet. Async gir bare ekstra FFI-kompleksitet. | tokio; async i kjernen | Enklere FFI og enklere feilsøking. |
| 11 | UI | **Web-frontend i Tauri: TypeScript + Svelte 5 + Vite**, ingen UI-bibliotek, ingen CSS-rammeverk (CSS-variabler fra tokens), systemfonter (`Segoe UI Variable`, `Segoe UI`, `system-ui`), ikoner som bundlede inline-SVG. Alt bundles, ingen CDN, ingen eksterne fonter. All logikk i kjernen; UI viser kort og sender beslutninger via `CoreBridge` (Tauri `invoke`). | Svelte kompileres bort (liten kjøretid), gir a11y-advarsler ved bygg og enkel tilstand. Raskt nok for < 100 ms. Piltaster, fokus og ARIA kommer fra standard HTML. | Vanilla TS + web components (mer kode for tilstand og maler); React (større kjøretid); native WinUI | UI-filer er små og lette å lese for agenter. Narrator og NVDA mot WebView2 må testes manuelt (MVP-PLAN). |
| 12 | Papirkurv og angre | **Tilstand i indeksen** (`asset.state`: active, trashed, purged) er sannheten. Hver handling skrives til `action_log` med `batch_id`. Angre = invers av siste batch i én transaksjon. På PC speiles `trashed` av en fysisk flytting til appens karantene (beslutning 20). Fysisk sletting skjer bare ved «Tøm» med eksplisitt bekreftelse (beslutning 22). Ved utløp (30 dager, konfigurerbart) merkes elementer «utløpt» og brukeren varsles. Ingen stille sletting. | Oppfyller «innhold endres aldri», «alt kan angres», «aldri hard sletting uten varsel». Handling er én rad-oppdatering. | Plattformens papirkurv alene (ingen angre per beslutning) | Angre er < 5 ms i kjernen. **Merk:** lagring frigjøres først ved permanent sletting. UI viser «kan ryddes» separat. |
| 13 | Språk | **nb + en** i MVP. `nn` utsettes til etter MVP (avklart 2026-11-11); nøkkelstruktur og nøkkelvis fallback til nb beholdes i designet. Kilde: `i18n/*.json`, generert til typede JSON/TS-filer for frontend. Kjernen returnerer **begrunnelseskoder**, aldri tekst. | Tekst bor kun i UI-laget. Én kilde hindrer divergens. | Tekst i kjernen; manuell vedlikehold per plattform | Pseudo-lokalet `nb` (+30 %) fanger overflyt i test. |
| 14 | Algoritmeversjon | `fingerprint.algo_ver` per rad. Ved endret algoritme analyseres bare rader med gammel versjon på nytt. | Kalibrering vil endre seg. Full reindeksering er dyrt. | Alltid full reindeksering | Sparer batteri og CI-tid. |
| 15 | Taskrunner og CI | **`just`** som eneste inngang, **GitHub Actions** med stifiltre. Se del 5. | Ett sett kommandoer lokalt og i CI. | Make, egne skript per plattform | Agenter trenger bare `just check`. |
| 16 | Telemetri | **Ingen** i MVP. Ytelse måles i test, ikke hos brukere. | On-device først, ingen datainnsamling. | Anonym telemetri | Ingen samtykkeflyt å bygge. |
| 17 | Sletting i plattform | `delete` (og `Quarantine`) i adapter; `delete` kun med `PurgeTicket`; kjernen har ingen skrivende plattformkall. Kalles bare fra Rust-kommandoer i Tauri, aldri fra frontend. | Oppfyller «aldri hard sletting uten varsel» med typer og tilgangsgrenser, ikke konvensjon | Kjerne kaller plattformen | Testbar med `FakeAdapter` |
| 18 | Kilder som forsvinner | `ChangeSet.removed`; avstemming til `purged` (`cause=external`), ikke regnet som «frigjort av Fjell» | Krasj mellom plattformsletting og DB trenger ingen journal | Egen journal | Én mekanisme for to tilfeller |
| 19 | Skjemautvidelser | Kolonner og tabeller i del 3 (protect_flags, album, fail_count, inverse_of m.m.) i migrering 1 | Unngår tidlig migreringskjede før første utgivelse | Separate migreringer per plan | Én eier av skjema |
| 20 | Karantene (PC) | Adapter-trait `Quarantine { stage(ids), unstage(ids) }` (utenfor `AssetSource`; flytter, sletter aldri). Mappe per volum: `<volumrot>\.fjell-karantene\<batch_id>\` (samme volum: `MoveFileEx`-rename, atomisk, ingen kopiering, ingen ekstra plass) med `manifest.json` (opprinnelig sti, id, størrelse) og README. Rekkefølge: `commit` i indeks, så `stage` på seriell bakgrunnskø; feilede id-er rulles tilbake som egen invers batch (`cause=undo`) og UI melder «N filer ble ikke flyttet». `undo`/`restore`: `unstage` først, så indeks. Gjenoppretting til opptatt sti gir `navn (gjenopprettet).ext`. Avstemming ved oppstart sammenholder manifest og indeks (fil i karantene uten `trashed` → `unstage`; `trashed` uten fil verken i karantene eller original → `purged`, `cause=external`). Skanning ekskluderer karantenemappen og regner staged filer som tilstedeværende. **Fallback:** der flytting ikke er mulig (nettverksstasjon uten skriverett, skyplassholder, fil i bruk) forblir `trashed` ren indekstilstand, UI sier «Merket, ikke flyttet». | Trygghetsvurdering: (A) ren indekstilstand har lavest filrisiko, men filen ligger fortsatt i mappen og i andre programmer, så «Flytt til papirkurv» blir misvisende og «Tøm» blir første fysiske endring. (B) direkte til Windows Papirkurv per handling gir ingen angre i appen og finnes ikke på nettverksstasjoner. Karantene på samme volum gir sann tilstand, atomisk og tapsfri flytting og angre som rename tilbake; A beholdes som fallback. | A alene; B alene; felles karantenemappe på systemdisk (kopi mellom volumer, kan feile midtveis) | Innhold endres aldri, men filen skifter sti (andre programmer, f.eks. Lightroom, mister referansen til den er gjenopprettet). `.fjell-karantene` bør ekskluderes fra sikkerhetskopi og skyspeiling (åpent spørsmål). |
| 21 | Windows-adapter og data | Én adapterkrate `fjell-fs` med `AssetSource`: portabel filsystemadapter (mapper, eksterne disker, UNC/nettverksstasjoner; testbar på Linux) og modul `windows/` (`cfg(windows)`) for: lange stier (`\\?\`), `source_id` = normalisert sti, `mod_token` = (endringstid, størrelse), EXIF-orientering, miniatyrer (beslutning 6), metadata, skyplassholdere (`RECALL_ON_DATA_ACCESS`/`OFFLINE` ⇒ `is_local = false`, aldri nedlasting under rask skann), frakoblet rot (ekstern disk ute) ⇒ `Unavailable`, **aldri** `removed`. Indeks i `%LOCALAPPDATA%\Fjell\index.db`; visningsbilder (skjermstørrelse) i diskcache `%LOCALAPPDATA%\Fjell\cache\` (standard 1 GB, LRU, kan slettes uten tap). Brukeren velger mapper i onboarding (ingen systemtillatelse). Flytting utenfor Fjell ses som borte + ny (avstemmes, ingen filID-sporing i MVP). | Adapteren kjenner Windows, kjernen gjør det ikke. Frakoblede disker må ikke se ut som slettede bilder. | Egen crate for Windows; filID-sporing (komplekst på nettverksstasjoner) | Én adapter å teste i CI; Windows-kode isolert. |
| 22 | «Tøm» på PC | `begin_purge` → appens ark → `confirm_purge` → Rust-kommando `delete(ids, mode)` med `PurgeTicket` → `finish_purge`. **Standard: Windows Papirkurv** (`IFileOperation`, `FOFX_RECYCLEONDELETE`, per-id-utfall, ingen systemdialog). **Permanent sletting** er et eget valg i arket med ekstra bekreftelse (`PermanentAck`: avkrysning + knapp med antall). Volum uten Papirkurv (nettverksstasjon, enkelte eksterne disker) gir `Unsupported`; da tilbyr UI permanent sletting med egen advarsel, aldri stille fallback. Ticket bærer modus. `freed_total` og «frigjort» teller bare permanent; Papirkurv-modus viser «Flyttet til Windows Papirkurv. Plassen frigjøres når den tømmes.» | Best trygghet: alt som kan gjenopprettes av Windows forblir gjenopprettelig, og permanent sletting krever to bevisste handlinger. Oppfyller CLAUDE.md («aldri hard sletting uten bekreftelse»). | Permanent som standard; Papirkurv uten valg (nettverksstasjoner kan ikke) | `purge`-rad i `action_log` får modus (`recycle`/`permanent`, migrering 1). Plass frigjøres ikke før brukeren tømmer Windows Papirkurv. |
| 23 | Distribusjon | **NSIS-installasjonsprogram** (Tauri-bundler, per bruker, uten admin) som standard; MSIX vurderes. WebView2: `downloadBootstrapper` (kun installasjonsprogrammet kan hente Microsofts kjøretid ved behov; appen har ingen nettverkstilgang). Kodesignering og oppdateringsstrategi er åpne spørsmål (MVP-PLAN). Ingen automatisk oppdatering i MVP. | Enklest og ingen butikkavhengighet. | MSIX først (signering og sandkasse påvirker filtilgang); auto-oppdatering (nettverkskall) | Usignert installasjonsprogram gir SmartScreen-advarsel inntil signering er avklart. |

## 2. Modulgraf for `core/`

Lagdelt og asyklisk. En modul kan bare avhenge av moduler i lavere lag. Regelen håndheves i CI (del 5).

```
L4  api
L3  pipeline
L2  cluster   suggest   actions   series   people   rules   protect   dashboard   (V1)
L1  store   hash   quality   screenshot
L0  model   (inkl. traits for AssetSource og Clock)
```

| Modul | Lag | Ansvar | Avhenger av | Grensesnitt (hovedpunkter) |
|---|---|---|---|---|
| `model` | L0 | Typer: `AssetId`, `Asset`, `Kind`, `State`, `ReasonCode`, `Hash64`. Traits `AssetSource`, `Clock` (V1: `EyeAnalyzer`, `FaceAnalyzer`, `SeriesId`, `PersonId`, `RuleId`, `ProtectScope`, `RuleSpec`, `DashboardData`). Ingen I/O. | (ingen) | `trait AssetSource { list_changes(since) -> ChangeSet { changed: Vec<AssetMeta>, removed: Vec<SourceId>, next_token }; thumbnail(id) -> Thumb; read_bytes(id) -> Reader }` |
| `store` | L1 | SQLite: skjema, migrering, spørringer, transaksjoner. | model | `Store::open`, `upsert_assets`, `put_fingerprints`, `set_state(batch)`, `next_cards(limit)`, `log_batch`, `pop_batch` |
| `hash` | L1 | BLAKE3 over bytes, dHash og pHash fra miniatyr. Ren funksjon. | model | `content_hash(Reader) -> [u8;32]`, `perceptual(Thumb) -> (Hash64, Hash64)` |
| `quality` | L1 | Uskarphetsscore fra miniatyr. Ren funksjon. | model | `blur_score(Thumb) -> f32` |
| `screenshot` | L1 | Signaler for skjermbilde fra metadata og miniatyr. Ren funksjon. | model | `signals(AssetMeta, Thumb) -> ShotSignals` |
| `cluster` | L2 | Eksakte grupper (lik `content_hash`) og visuelle grupper (MIH + union-find). Ren funksjon på hashlister. | model | `exact_groups(&[(AssetId,[u8;32])])`, `visual_groups(&[(AssetId,Hash64,Hash64)], Params)` |
| `suggest` | L2 | Velger behold-kandidat per gruppe og lager forslag med `ReasonCode` og konfidens. Ren funksjon. | model | `rank_group(&[Member]) -> Ranked`, `suggestions(...) -> Vec<Suggestion>` |
| `actions` | L2 | Papirkurv, gjenopprett, behold, tøm, angre, utløp. Alt som batch. | store, model | `plan(req)`, `commit(plan, acks)`, `restore(ids)`, `keep(ids)`, `set_keeper`, `undo()`, `expire_due()`, `finish_purge(ticket, results)` (detaljer: PLAN-TRYGGHET). `purge` betyr «oppdater indeks etter plattformens svar», ikke selve slettingen |
| `series` (V1) | L2 | Serier: rekker av nesten like bilder tatt tett i tid; beste bilde (inkl. lukkede øyne via `EyeAnalyzer`). | model, store | Se docs/v1/V1-PLAN.md. Lagres som `grp.kind = 'series'`. |
| `people` (V1) | L2 | Personer fra ansiktsindeks (`FaceAnalyzer`), navn, sammenslåing. Av som standard; embeddings bare lokalt. | model, store | Se V1-PLAN. `person`, `face_embedding`. |
| `rules` (V1) | L2 | Brukerregler (`RuleSpec`): evaluering, forhåndsvisning. Foreslår bare, sletter aldri. | model, store | Se V1-PLAN. `rule`, `rule_hit`. |
| `protect` (V1) | L2 | Beskyttelse per bilde, mappe, album og person (`ProtectScope`). | model, store | Se V1-PLAN. `protection`. |
| `dashboard` (V1) | L2 | Lesemodell (`DashboardData`) fra indeksen. | model, store | Se V1-PLAN. Bare lesing. |
| `pipeline` | L3 | Skanning, inkrementell oppdatering, parallell analyse, fremdrift, avbrudd. | store, hash, quality, screenshot, cluster, suggest, model | `scan(&dyn AssetSource, Progress) -> ScanReport`, `cancel()` |
| `api` | L4 | Eneste offentlige flate. I MVP kalles den direkte fra Tauri-kommandoer (Rust); UniFFI-grensen kommer med mobilfasen. Samler pipeline, actions og lesemodeller. | pipeline, actions, store, model | `Fjell::open`, `start_scan(ScanConfig{mode: Quick|Full, priority, deadline})`, `cards(kind, limit, cursor)`, `decide_many(Vec<(CardId, Decision)>)` (internt plan + commit, én batch), `set_keeper`, `begin_purge`/`confirm_purge`/`finish_purge`, `undo`, `home_summary`, `trash_summary`, `settings` |

Kommentarer:
- `Purge` krever typen `Confirmed`, som bare kan lages av `api` etter brukerbekreftelse. Dermed kan ikke hard sletting skje ved en feil.
- Selve slettingen i plattformen er adapterens `delete(ids) -> Vec<(AssetId, Deleted|Cancelled|Failed)>` (egen trait utenfor `AssetSource`, som ikke har skrivemetoder). Kalles bare av UI med `PurgeTicket` fra `api` (beslutning 17).
- Fjernede kilder (`ChangeSet.removed`) avstemmes av `pipeline` via `store` til `state = purged` med `cause = external`, uten å gå via `actions` (beslutning 18).
- Plattformadaptere (MVP: filsystem/Windows i `fjell-fs`; etter MVP: PhotoKit, MediaStore) ligger utenfor `core/crates/fjell-core` og implementerer `AssetSource`. Kjernen kjenner ingen plattform.
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

V1-utvidelse (migrering 2, `002_v1.sql`; `001` er uendret): `grp.kind` får `series`, `suggestion.kind` får `series` og `rule` (tabellomskriving med `foreign_keys` av og `foreign_key_check`), og nye tabeller `protection(scope, target, created_at)`, `eye_result`, `person` (`merged_into` for angrebar sammenslåing), `face_embedding` (BLOB, kun lokal), `rule` (`deleted_at`, aldri hard sletting) og `rule_hit`. `setting.face_index_enabled` er `0` som standard. Detaljer og eierskap: `docs/v1/V1-PLAN.md`.

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
| UI | Kort-beslutning via tastatur, mus og knapp gir samme resultat. Skjermleseretiketter finnes. Skjermbilder med pseudo-lokale `nb` (+30 %) og `en` uten avkapping, ved Windows-skalering 100–200 %. axe-revisjon 0 brudd. | Vitest (reducere), Playwright + axe mot `MockBridge` ved endring i `desktop/` |
| i18n | Nøkkelparitet nb/en, ingen hardkodet tekst i UI-filer (lint). | Hver PR |
| Ytelse | Se under. | Se under |

### Ytelsesbudsjett

| Mål | Budsjett | Måling |
|---|---|---|
| Handling (kort-beslutning, trash, undo, behold) p95 | **< 100 ms** ende til ende (til neste kort synlig; filflytting til karantene skjer etterpå på bakgrunnskø). Kjernekall < 10 ms (sikte < 5 ms). | `perf`-test i `fjell-cli`: 1 000 kall mot database med 50 000 bilder, p95 rapporteres. PR feiler over 10 ms for kjernekall. Ende til ende måles med Playwright mot Tauri-appen på referansemaskinen (Windows). |
| Neste kort | Forhåndslast 3 miniatyrer. Spørring bruker `idx(status, kind, confidence)`. < 5 ms. | Samme perf-test |
| Første indeksering | **< 10 min per 10 000 bilder** (sikte 5 min), dvs. < 60 ms veggtid per bilde, på referansemaskin med lokal SSD. HDD og nettverksstasjon måles og rapporteres, ingen port. | Nattlig kjøring på referansemaskinen (Windows) med syntetisk 10 000-bibliotek. |
| Klynging og forslag (10 000 bilder) | < 5 s | `criterion`-benchmark |
| Kald oppstart | < 1,5 s (Tauri-vindu og WebView2 klart, åpne database, første kort) | Playwright/`tauri-driver` på referansemaskinen |

Budsjett per bilde, antatt 3 effektive kjerner (konservativt: en PC har typisk 6–16 logiske kjerner; bakgrunn bruker 1–2 tråder med lav prioritet/EcoQoS) (60 ms veggtid gir 180 ms CPU):

| Steg | Budsjett (CPU) |
|---|---|
| Adapter-dekoding til 256 px | ≤ 100 ms |
| Innlesing og BLAKE3 (bare ved lik størrelse) | ≤ 25 ms |
| dHash + pHash | ≤ 2 ms |
| Uskarphet | ≤ 3 ms |
| Skjermbildesignaler | ≤ 1 ms |
| Batchet databaseskriving | ≤ 1 ms |
| Reserve | ≥ 45 ms |

Proxy i CI: kjernens egen beregning (uten dekoding) ≤ 10 ms per bilde på runner. Brudd stopper PR.
Maskinbelastning: bakgrunnsskanning kjører med lav prioritet, er avbrytbar og gjenopptagbar via `mod_token`, og pauser på bærbar ved batterisparing (valgfritt). Mobil (etter MVP): bare ved lading og Wi-Fi.

## 5. Repostruktur, bygg og CI

```
fjell/
  core/                  Cargo-workspace
    crates/fjell-core/   modulene fra del 2 (ett crate, moduler som mapper)
    crates/fjell-fs/     `AssetSource` for filsystem + `windows/`-modul (`cfg(windows)`), `Quarantine`, `delete`
    crates/fjell-cli/    testsele, perf (bruker `fjell-fs`)
    testdata/            golden-korpus + generator
  desktop/               Tauri 2-app: `src-tauri/` (Rust-kommandoer, bruker `fjell-core` + `fjell-fs`), `src/` (Svelte + TS)
  app/                   iPhone og Android etter MVP (tom i MVP; `fjell-ffi`/UniFFI opprettes da)
  i18n/                  nb.json, en.json
  tools/                 gen-i18n, check-layers
  docs/                  spesifikasjon, docs/mvp/
  justfile
  .github/workflows/
```

**Bygg**
- `just check` = fmt + clippy (`-D warnings`) + test + check-layers + frontend (`tsc`, `svelte-check`, `vitest`). `just bench`, `just i18n`, `just desktop` (Tauri-bygg), `just a11y`.
- Windows: `cargo tauri build` gir NSIS-installasjonsprogram (x64). Utvikling og `core`-tester fungerer også på Linux (bonus).
- Frontend bundles av Vite til statiske filer inne i appen (ingen CDN, ingen eksterne fonter). `package-lock.json` sjekkes inn; npm-avhengigheter holdes få (`svelte`, `vite`, `@tauri-apps/api`, dev: `vitest`, `playwright`, `@axe-core/playwright`).
- Én `Cargo.lock`. Rust-avhengigheter holdes få: `rusqlite`, `blake3`, `rayon`, `thiserror`, `image`, `windows` (Win32/WIC), `tauri` (kun `desktop/`), `proptest` (dev), `criterion` (dev).

**CI (GitHub Actions, stifiltre for å spare minutter)**

| Jobb | Utløser | Innhold |
|---|---|---|
| `core` | endring i `core/` eller `i18n/` | `just check` på `ubuntu-latest` og `windows-latest` (hashdeterminisme Linux/Windows), golden-porter, migrering, `cargo-deny` (lisens og sårbarheter) |
| `layers` | som `core` | `tools/check-layers` feiler på `use` mot høyere lag eller på syklus |
| `desktop` | endring i `desktop/`, `i18n/` eller `core/` | `windows-latest`: bygg Tauri-app (NSIS-artefakt), `just check`, Vitest, Playwright + axe (lys/mørk × nb/pseudo/en × skalering 100/200 %), skjermbilder som artefakter |
| `i18n` | endring i `i18n/` eller `desktop/` | nøkkelparitet, hardkodet-tekst-lint, pseudo-lokale |
| `nightly` | tidsplan | 10 000-bilders indekseringsbudsjett, 50 000-bilders handlings-p95, ende-til-ende-ytelse og kald oppstart på referansemaskinen (Windows, self-hosted runner) |

**Arbeidsmåte (agentvennlig, tokensparende)**
- Én fase = én gren = én PR (jf. CLAUDE.md). Hver modul i `core/` er én fil på høyst ca. 400 linjer med 10-linjers topptekstkommentar om grensesnitt (`store` og `actions` kan være kataloger med undermoduler).
- Agenter leser bare `api` og berørt modul. Rene moduler kan utvikles og testes parallelt av separate agenter fordi grensene er traits og enkle typer i `model`.
- Endres et designvalg, oppdateres dette dokumentet først.

## Åpne punkter (avklares før kode)
- Kalibrering av terskler (Hamming, uskarphet) mot golden-korpus. Startverdier: Hamming ≤ 6 på både dHash og pHash, uskarphet ved laveste 10 % av normalisert score.
- Referansemaskin for ytelsestester (én Windows 11-PC, mellomklasse, lokal SSD) og Windows-versjoner som støttes (forslag: Windows 10 22H2 og 11). Se MVP-PLAN, åpne spørsmål.
- «Tøm»: løst i beslutning 22 og PLAN-UX-I18N 3.6. Gjenstår: om `.fjell-karantene` skal støtte skyspeilede mapper (OneDrive) og sikkerhetskopi-ekskludering.

## Etter MVP
- iPhone (SwiftUI, PhotoKit-adapter, xcframework via UniFFI/`fjell-ffi`, VoiceOver, iOS 17+, macOS-runner) og Android (Kotlin, Jetpack Compose, `cargo-ndk`/AAR, MediaStore, TalkBack, Android 11+ som sannsynlig minimum for «Tøm») er utenfor MVP (iPhone flyttet 2026-11-11: «PC erstatter iPhone i MVP»). Kjernen holdes plattformnøytral (Rust, `AssetSource`-trait, ingen plattformkode i `fjell-core`) slik at mobiladaptere og UI kan legges til uten endringer i kjernen. Mobil-UI blir native tynne UI-er (opprinnelig beslutning 11, utsatt).
- Nynorsk (`nn.json`) legges til etter MVP. Nøkkelstruktur og nøkkelvis fallback til nb er allerede på plass.
