# Plan: kjerne (`core/`)

Status: utkast fra spesialist KJERNE. Grunnlag: `ARKITEKTUR.md` (bindende) og `PRODUKT.md` del 1–2.
Omfang: indeksering, eksakt hash, pHash/dHash, valg av vinner, uskarphet, skjermbilde. Handlinger (papirkurv/angre) og UI er andre planer; her står bare grensesnittet mot dem.
Alle terskler er **startverdier** som kalibreres mot golden-korpus (K8). Endres en verdi, oppdateres `ARKITEKTUR.md` først.

## 0. Avgrensning mot PRODUKT.md (arkitekturen gjelder)

| PRODUKT.md sier | MVP gjør | Grunn |
|---|---|---|
| SHA-256/xxHash | BLAKE3 med størrelsesforfilter | Beslutning 7 |
| pHash/dHash **og CLIP** | Bare dHash + pHash | Beslutning 3 |
| Vinner: oppløsning/kompresjon, skarphet, eksponering, øyne/smil, metadataoverføring | Bare punkt 1 og 2 (oppløsning, kompresjon, skarphet). Ingen eksponering, ansikter eller metadataoverføring | Ingen modell; originaler skrives aldri |
| Uskarphet: Laplacian **+ lært modell** | Bare Laplace-varians | Beslutning 8 |
| Serier/burst (nivå 3) | Utenfor MVP | Etter MVP |
| Over-/undereksponert, lukkede øyne, utilsiktede, estetikk | Utenfor MVP | Ikke i MVP-omfanget |
| Video | Bare eksakt hash (ingen miniatyr, hash, uskarphet) | Forenkling, antakelse |

## 1. Leveranser og rekkefølge

| ID | Leveranse | Moduler | Avhenger av | Kan kjøre parallelt med |
|---|---|---|---|---|
| K0 | Fundament: typer, traits, testdobler, golden-korpus og harness | model, testdata | repoplanens skjelett (workspace, `just`) | – |
| K1 | Eksakt hash og eksakte grupper | hash, cluster | K0 | K2a, K3, K4, K6 |
| K2a | dHash + pHash | hash | K0 | K1, K3, K4, K6 |
| K2b | Visuell klynging (MIH + union-find) | cluster | K2a | K3, K4, K6 |
| K3 | Uskarphetsscore | quality | K0 | K1, K2, K4, K6 |
| K4 | Skjermbildegjenkjenning | screenshot | K0 | K1, K2, K3, K6 |
| K5 | Valg av vinner og forslag | suggest | K1, K2b, K3, K4 | K6 |
| K6 | Lagring for fingeravtrykk og arbeidskø | store | K0 | K1–K5 |
| K7 | Indeksering: inkrementell, bakgrunn | pipeline | K1–K6 | – |
| K8 | Kalibrering og ytelsesgate | alle | K7 | – |

Hver modul er én fil på ca. 400 linjer med 10-linjers topptekst (arbeidsmåten i ARKITEKTUR del 5). K1–K4 og K6 er uavhengige og kan gis til separate agenter fordi grensen er `model`-typene.

## 2. Felles (K0)

### Oppgaver
1. `model`: `Thumb { w, h, fmt: Gray8|Rgba8, data }` (lengste side 256, bildeforhold bevart, orientering allerede brukt), `AssetMeta`, `Hash64`, `ReasonCode`, `Kind`, `State`, `AlgoVer`.
2. `AssetMeta` felt jeg krever: `source_id, kind, created_at, mod_token, width, height, bytes, mime, platform_screenshot_hint: bool, has_camera_exif: Option<bool>`, og fra andre planer: `is_local` (UX A1), `protect_flags` + albumliste (id, navn, delt) + valgfri `BackupState` (TRYGGHET). Alle felt landes i K0 slik at `model` ikke endres senere.
3. Traits `AssetSource`, `Clock`. Testdobler `MemSource` (i minnet, kan injisere feil og endringer) og `FakeClock`.
4. Golden-generator i `core/testdata/` (egen binær, kan bruke `image`-crate; **ikke** en avhengighet i `fjell-core`). Deterministisk via fast seed. Skriver `manifest.json` med fasit.
5. Harness: leser manifest, kjører modul, beregner presisjon/recall, feiler under porten og skriver tallene.
6. Hash-vektorfil `hash_vectors.json` (faste miniatyrer med forventede hasher og scorer). Test feiler med meldingen «algoritme endret: øk `ALGO_VER`».

### Testdata (golden-korpus, ca. 300 bilder)

| Gruppe | Antall (ca.) | Innhold | Fasit |
|---|---|---|---|
| Original | 60 | Prosedyrebilder (gradienter, tekstur, former, støy med fast seed), pluss ca. 20 lisensfrie ekte foto (CC0/PD, `LICENSES.md`) | – |
| Eksakt kopi | 20 | Bytevis lik, annet filnavn og tid | eksakt gruppe |
| Nedskalert | 30 | 50 % og 25 % | visuell gruppe |
| Omkodet | 30 | JPEG q30/60/85, PNG til JPEG | visuell gruppe |
| Beskåret 5 % | 20 | Jevn beskjæring fra alle kanter | visuell gruppe (vanskeligste) |
| Uskarp | 40 | Gauss σ = 0,5 / 1 / 2 % av bredden, bevegelsesuskarphet | `blurry=true` fra σ ≥ 1 % |
| Skarpe kontroller | 40 | Samme motiv, skarpe, også lav kontrast (hvit vegg, himmel) | `blurry=false` |
| Skjermbilde | 30 | Syntetisk UI (flate felt, tekstlignende blokker) i kjente enhetsoppløsninger; varianter uten flagg, uten EXIF, PNG og JPEG | `screenshot=true` |
| Skjermbilde-negativer | 20 | PNG uten EXIF i vilkårlig størrelse (nedlastet-type), kamerabilde i skjermstørrelse | `screenshot=false` |
| Negative par | 30 | Lik komposisjon/farger, ulikt innhold (annen seed), lik scene med ulik innramming | ingen gruppe |
| Degenererte | 10 | Helsvart, helhvit, enfarget, 1×1, ekstremt smal | ikke visuell gruppe, ikke uskarp |

Generatoren lager også 256 px-miniatyrene med en **referansenedskalering** (area-average), slik at kjernetester ikke trenger dekoder.

## 3. K1: Eksakt hash

### Oppgaver (rekkefølge)
1. `hash::content_hash(Reader) -> [u8;32]`: BLAKE3, strømmende, buffer 64 KiB, ingen full innlesing i minnet.
2. `store`: spørring `needs_content_hash()` = aktive filer med `content_hash IS NULL` og `bytes` lik minst én annen aktiv fil (`GROUP BY bytes HAVING COUNT(*)>1`). Bruker `idx(bytes)`.
3. `cluster::exact_groups(&[(AssetId,[u8;32])]) -> Vec<Group>`: gruppering på lik hash, ren funksjon, stabil rekkefølge (sortert på `AssetId`).
4. Keeper for eksakte grupper (i `suggest`, se K5): eldste `created_at`, deretter lavest `AssetId`.
5. Unik fil (ingen størrelseskollisjon) får aldri hash, og kan få det senere hvis en ny fil med lik størrelse dukker opp (lat beregning).

### Akseptkriterier

| ID | Kriterium | Test |
|---|---|---|
| K1-1 | Hash av kjente vektorer (tom fil, 1 byte, 1 MiB, 100 MiB) er lik offisielle BLAKE3-testvektorer | Enhet |
| K1-2 | Strømming gir samme hash uavhengig av lesestørrelse (1 B, 4 KiB, 1 MiB) | proptest |
| K1-3 | Alle eksakte kopier i korpus havner i én gruppe, ingen falske medlemmer (presisjon = 1,0, recall = 1,0) | Golden |
| K1-4 | Fil med unik størrelse blir aldri lest (teller på `MemSource.read_bytes`) | Enhet |
| K1-5 | Ny fil med lik størrelse som eksisterende uhashet fil gir hash på **begge** | Integrasjon |
| K1-6 | Samme størrelse, ulikt innhold gir ulike hasher og ingen gruppe | Enhet |
| K1-7 | Gruppering er uavhengig av inndatarekkefølge | proptest |
| K1-8 | Lesefeil på én fil stopper ikke de andre, og filen forsøkes på nytt neste skanning | Enhet med feilinjeksjon |

### Ytelsesmål
- Kjerne: ≥ 500 MB/s per tråd (criterion, minnebuffer, CI-runner). I praksis I/O-bundet.
- Per fil ≤ 25 ms ved typisk 4 MB (arkitekturbudsjett).
- `exact_groups` for 10 000 hasher < 20 ms.

## 4. K2: dHash, pHash og visuell klynging

### K2a: hashing

Oppgaver:
1. Deterministisk nedskalering i kjernen (area-average, `f32`) fra miniatyr (Gray8 eller RGBA til luma med faste vekter).
2. dHash: 9×8, bit = venstre piksel > høyre. 64 bit, fast bitrekkefølge dokumentert.
3. pHash: 32×32, 2D DCT (forhåndsberegnet tabell), lavfrekvent 8×8 uten DC, bit = verdi > median. 64 bit.
4. `perceptual(Thumb) -> (Hash64, Hash64)` og `low_info(Thumb) -> bool` (standardavvik for luma < 4): lavinformasjonsbilder utelates fra visuell klynging.
5. `ALGO_VER` som konstant. Vektorfil fra K0 låser resultatet.

Akseptkriterier:

| ID | Kriterium | Test |
|---|---|---|
| K2a-1 | Bit-identisk resultat for samme miniatyr på macOS, Linux og Windows (CI-matrise) | Vektorfil |
| K2a-2 | Samme bilde nedskalert, omkodet (q ≥ 30): Hamming ≤ 6 på **både** dHash og pHash for ≥ 95 % av parene | Golden |
| K2a-3 | Utgått fra MVP (kryssplattform-gate hørte til Android; nummer beholdt) | - |
| K2a-4 | Negative par: Hamming > 6 på minst én av hashene for ≥ 98 % | Golden |
| K2a-5 | Gråtone- og RGBA-inndata av samme bilde gir lik hash | Enhet |
| K2a-6 | Miniatyr med annen størrelse enn 256 (f.eks. 200×150, 256×1) gir ikke panikk | proptest |

### K2b: visuell klynging

Oppgaver:
1. MIH i minnet: 8 blokker × 8 bit over pHash. Kandidater = lik blokk i minst én posisjon. Verifiser med Hamming ≤ 6 på dHash **og** pHash.
2. Union-find over verifiserte par. Bare aktive bilder, kind = photo, ikke `low_info`.
3. Input er **én representant per eksakt gruppe** (eksakt-keeper). Andre eksakte kopier hører bare til den eksakte gruppen, så ett bilde gir ikke to forslag.
4. Kjedevern: komponent med > 30 medlemmer, eller medlem med Hamming > 10 til noen annen medlem, klynges om med skjerpet terskel (≤ 4). Terskler som `Params`.
5. Inkrementell variant: `visual_groups_for(changed, index)` bygger bare om komponenter som berøres.
6. Bøttekapp: blokkbøtte > 500 oppføringer behandles som avvik og logges som metrikk (beskyttelse mot degenererte hasher).

Akseptkriterier:

| ID | Kriterium | Test |
|---|---|---|
| K2b-1 | MIH gir **nøyaktig samme** mengde par som brute force (Hamming ≤ 6) på 2 000 tilfeldige og 300 korpus-hasher | proptest + golden |
| K2b-2 | Pigeonhole: par med avstand 0…6 finnes alltid, med konstruerte tester ved 6 (6 biter i 6 ulike blokker) | Enhet |
| K2b-3 | Visuelt duplikat: presisjon ≥ 0,98, recall ≥ 0,90 (port fra ARKITEKTUR del 4), **uten beskåret** (rapporteres i K2b-4) | Golden |
| K2b-4 | Beskåret 5 %: egen rapportert recall (informativ, ikke port) | Golden |
| K2b-5 | Gruppering er uavhengig av rekkefølge | proptest |
| K2b-6 | Ingen gruppe > 30 etter kjedevern, og ingen negativt par slått sammen via kjede | Konstruert kjedetest |
| K2b-7 | Degenererte bilder (helsvart osv.) danner aldri gruppe | Golden |
| K2b-8 | Inkrementell oppdatering gir lik resultat som full ombygging | proptest |

### Ytelsesmål
- `perceptual` ≤ 2 ms per miniatyr (mål < 1 ms), kjernemål på CI-runner.
- `visual_groups` 10 000 bilder < 5 s (arkitekturbudsjett), mål < 500 ms. 50 000 bilder < 5 s (nattlig).
- Minnebruk for indeks ≤ 5 MB ved 50 000 bilder.

## 5. K3: Uskarphetsscore

### Oppgaver
1. `quality::blur_score(Thumb) -> f32`: luma som `f32` 0–255, Laplace-kjerne 3×3 `[0 1 0; 1 -4 1; 0 1 0]` på indre piksler, `score = Var(L) / (Var(I) + 1,0)`.
2. Lavkontrast (Var(I) < 4): returner `LOW_CONTRAST = 1000,0` («ikke vurderbar», aldri uskarp). Dokumentert konstant.
3. Terskel: bilde er uskarpt hvis `score < min(P10, ABS_MAX)`. P10 beregnes over biblioteket (minst 50 bilder med score, ellers bare `ABS_MAX`). `ABS_MAX` settes i K8 fra korpus, slik at «laveste 10 %» ikke alltid flagger noe i et skarpt bibliotek.
4. Konfidens (0–1) fra avstand under terskelen. Begrunnelseskode `Blurry`, merket i UI viser prosent fra kjernens konfidens.
5. Terskelberegning i `suggest` (ren funksjon over scorelisten), ikke i `quality`.

### Akseptkriterier

| ID | Kriterium | Test |
|---|---|---|
| K3-1 | Flatt bilde gir `LOW_CONTRAST`. Skakkrutemønster gir høyere score enn det samme uskarpet | Enhet |
| K3-2 | Score faller monotont med økende Gauss-σ på samme motiv (10 motiver × 5 σ) | Golden |
| K3-3 | Uskarphet: presisjon ≥ 0,90 (port) på uskarpe (σ ≥ 1 %) mot skarpe kontroller | Golden |
| K3-4 | Hvit vegg og himmel (lav kontrast, skarpe) flagges ikke | Golden |
| K3-5 | Samme motiv ved to oppløsninger gir score innen ±25 % | Golden |
| K3-6 | Deterministisk: lik score bit for bit på alle plattformer | Vektorfil |
| K3-7 | Miniatyr mindre enn 3×3 gir `LOW_CONTRAST`, ikke panikk | proptest |

### Ytelsesmål
- ≤ 3 ms per 256 px miniatyr (mål < 0,5 ms). Én gjennomgang, ingen heap-allokering utover radbuffer.

### Kjent begrensning (rapporteres til arkitekt)
Nedskalering til 256 px skjuler lett uskarphet (4000 px til 256 px krymper blur 16 ganger). MVP fanger bare tydelig uskarphet og bevegelsesuskarphet. Korpusets σ er derfor oppgitt relativt til bildebredden. Etter MVP: valgfritt 256 px **utsnitt i full oppløsning** fra plattformen.

## 6. K4: Skjermbildegjenkjenning

### Oppgaver
1. `screenshot::signals(AssetMeta, Thumb) -> ShotSignals` (bitmaske, lagres i `fingerprint.shot_signals`).
2. Fire signaler: **S1** plattformflagg eller album «Skjermbilder» (`platform_screenshot_hint`), **S2** oppløsning lik kjent skjermstørrelse (begge retninger), **S3** ingen kamera-EXIF (`has_camera_exif == Some(false)`; `None` teller ikke), **S4** `image/png`.
3. Tabell med kjente skjermstørrelser i egen fil `screen_sizes.rs` (iPhone, iPad, desktop 1080p/1440p/4K/Retina). Endring i tabellen øker `ALGO_VER`.
4. Forslag når minst to signaler. Konfidens: 2 → 0,70, 3 → 0,90, 4 → 0,99 (startverdier).
5. Miniatyren brukes ikke i MVP (signaturen beholdes fra arkitekturen). Kandidat senere: ensfarget statuslinje.
6. Konfigurerbart `require_strong` (minst S1 eller S2). Standard følger arkitekturen (to av fire). Skjerping foreslås, se del 9.

### Akseptkriterier

| ID | Kriterium | Test |
|---|---|---|
| K4-1 | Hver av de 16 signalkombinasjonene gir riktig bitmaske og riktig forslag/ikke forslag | Tabelltest |
| K4-2 | Skjermbilde: presisjon ≥ 0,95 (port) over skjermbilde og skjermbilde-negativer | Golden |
| K4-3 | PNG uten EXIF i vilkårlig størrelse flagges ikke med `require_strong`, og tallet uten er rapportert | Golden |
| K4-4 | Ingen kamera-EXIF = `None` gir ikke S3 | Enhet |
| K4-5 | Rotert oppløsning (landskap) treffer S2 | Enhet |
| K4-6 | Begrunnelsen kan vises: kodene `ShotPlatformFlag, ShotScreenSize, ShotNoCameraExif, ShotPng` returneres fra bitmasken | Enhet |

### Ytelsesmål
- ≤ 1 ms per bilde (forventet < 10 µs).

## 7. K5: Valg av vinner og forslag

### Oppgaver
1. `suggest::rank_group(&[Member]) -> Ranked`. `Member = { id, width, height, bytes, blur_score, created_at, protect_flags }`.
2. Rangering, leksikografisk og deterministisk (steg 0 fra TRYGGHET 4.2):
   0. Medlem med `protect_flags` ≠ 0 foretrekkes som keeper (ingen egen begrunnelse; går videre til neste steg blant like). Hvis flere er beskyttet, avgjør steg 1–5.
   1. Flest piksler (`w·h`), `HigherResolution`.
   2. Høyest `bytes / piksler` (mindre komprimering), `LessCompressed`.
   3. Høyest `blur_score` (skarpest, bare hvis begge er ≠ `LOW_CONTRAST` og forskjellen > 10 %), `Sharper`.
   4. Eldst `created_at`, `Older`.
   5. Lavest `AssetId`.
3. Keeperens begrunnelse = første kriterium som skilte den fra nest beste, lagres som `reason_code`. Eksakte grupper: `Older`.
4. `suggestions(...)`: alle ikke-keepere i gruppe får `exact_dup` eller `visual_dup`. Uskarpe får `blurry`, skjermbilder får `screenshot`, begge uten `grp_id`.
5. Konfidens: eksakt = 1,0. Visuell = `1 − maxHamming/7`, avrundet ned til 0,05-steg. Uskarp og skjermbilde som i K3/K4.
6. Kjernen setter aldri `accepted` og forhåndsvelger ingenting. Valg skjer i UI via `decide_many`. Om UI forhåndsmerker duplikater (UX-planen) er et UX-valg, avklart av produkteier 2026-11-11 (forhåndsmerket: alt unntatt keeper; beskyttede aldri).
7. Nye forslag overskriver ikke `rejected`. `UNIQUE(asset_id, kind)` bevarer brukerens avvisning.

### Akseptkriterier

| ID | Kriterium | Test |
|---|---|---|
| K5-1 | Høyere oppløsning vinner uansett skarphet (blant like beskyttelsesnivå, steg 0) | Enhet |
| K5-2 | Lik oppløsning: den med høyest bytes/piksel vinner | Enhet |
| K5-3 | Lik oppløsning og komprimering: skarpest vinner, men ikke ved < 10 % forskjell | Enhet |
| K5-4 | Fullstendig likhet gir eldst, deretter lavest id | Enhet |
| K5-5 | Resultat uavhengig av medlemsrekkefølge | proptest |
| K5-6 | Eksakt kopi av nedskalert versjon: den store beholdes (korpus: 100 % av nedskalerte par) | Golden |
| K5-7 | Hver gruppe har nøyaktig én keeper, og keeper får ikke forslag | proptest |
| K5-8 | Avvist forslag gjenopprettes ikke ved ny skanning | Integrasjon |
| K5-9 | Hver `reason_code` finnes i i18n-nøkkellisten (paritetstest fra i18n-planen) | CI |

### Ytelsesmål
- `rank_group` < 10 µs per gruppe på ≤ 30 medlemmer. `suggestions` for 10 000 bilder: sammen med klynging innenfor 5 s.

## 8. K6 og K7: Lagring og indeksering

### K6: lagring (store, de delene indekseringen trenger)
Oppgaver:
1. Migrering 1: hele skjemaet fra ARKITEKTUR del 3 inkl. `action_log`, album-tabeller, triggere og kolonnene fra TRYGGHET (beslutning 19). K6 eier filen, TRYGGHET leverer trigger-SQL og tester.
2. `upsert_assets(&[AssetMeta])` i transaksjoner på 500 rader. Endret `mod_token` nullstiller fingeravtrykk (beholder raden, `algo_ver = 0`). Bevarer `state`, `trashed_at`, `expires_at` (I13).
3. `work_set()` = aktive assets uten fingerprint, eller `algo_ver < ALGO_VER`, eller `mod_token` endret.
4. `put_fingerprints(batch)` i én transaksjon per 100 rader, via én skrivetråd. Lesetilkobling er separat.
5. Kolonne `fingerprint.fail_count` (nytt, se del 9). Ved ≥ 3 feil hoppes bildet over til algoritmeversjon øker.
6. `all_hashes()` for MIH-bygging og `grp`-utskifting i én transaksjon.

Akseptkriterier:

| ID | Kriterium | Test |
|---|---|---|
| K6-1 | Migrering fra tom database og fra forrige versjon gir lik skjemadump | Migreringstest |
| K6-2 | Lesing blokkeres ikke mens skriver pågår (WAL) | Samtidighetstest |
| K6-3 | `work_set` etter full skanning er tom, og etter `ALGO_VER`-økning inneholder den alle | Enhet |
| K6-4 | `upsert` av 10 000 uendrede rader endrer ingen rader og tar < 300 ms | Perf |

### K7: indeksering (pipeline)

Flyt:

| Steg | Innhold |
|---|---|
| 1. Enumerer | `list_changes(since)` fra lagret `scan_cursor` i `setting`. `upsert_assets`. Fjernede assets håndteres (se del 9). |
| 2. Rask fase (A) | `needs_content_hash()`, BLAKE3 parallelt. Eksakte grupper og forslag er klare **uten dekoding**. Gir onboarding-resultatet «312 eksakte duplikater» raskt. |
| 3. Full fase (B) | `work_set()`. `rayon`-pool henter `thumbnail(id)`, kjører `perceptual`, `blur_score`, `signals`. Resultater sendes over `sync_channel` (begrenset, `2 × tråder`) til én skrivetråd. |
| 4. Klynging | Etter B (og ved hver 2 000. bilde for tidlig visning): inkrementell visuell klynging og forslag for berørte grupper. |
| 5. Ferdig | Lagre `scan_cursor`, returner `ScanReport` (nye, endrede, fjernede, feilede, tid per steg). |

Oppgaver (rekkefølge):
1. `ScanReport`, `Progress` (trådsikker, maks 4 oppdateringer/s), `ScanConfig { mode: Quick|Full, priority: Foreground|Background, deadline: Option<Duration> }`. `Quick` = steg 1–2 (ingen dekoding, hopper over `!is_local`), `Full` = alle steg (UX K6).
2. Steg 1 og 2 med `MemSource`.
3. Steg 3 med feilinjeksjon: `catch_unwind` per bilde, `Unavailable` (f.eks. iCloud ikke lastet ned) hoppes over uten feiltelling, øvrige feil øker `fail_count`.
4. Avbrudd: `cancel()` (`AtomicBool`) sjekkes mellom bilder. Pågående batch committes. Gjenopptak skjer automatisk fordi `work_set` utledes fra databasen.
5. `deadline` for bakgrunnsoppdrag (iOS `BGProcessingTask`): stopp ryddig før fristen.
6. Prioritet: `Foreground` bruker alle kjerner, `Background` bruker 1–2 tråder (varme og batteri). Valg av lading/Wi-Fi tas av plattformen, ikke kjernen.
7. Steg 4 koblet inn, deretter integrasjonstest i `fjell-cli` mot filsystemadapter.

Akseptkriterier:

| ID | Kriterium | Test |
|---|---|---|
| K7-1 | Full skanning av korpus gir samme grupper og forslag som modulene hver for seg | Integrasjon |
| K7-2 | Ny skanning uten endringer: 0 miniatyrforespørsler, 0 bytelesinger | Teller i `MemSource` |
| K7-3 | Én ny fil: bare den analyseres (+ eventuell hash-partner), og gruppene oppdateres | Integrasjon |
| K7-4 | Endret `mod_token` gir ny analyse av akkurat den filen | Integrasjon |
| K7-5 | `cancel()` midt i: databasen er konsistent, neste skanning fullfører og gir samme sluttresultat som uavbrutt | proptest på avbruddspunkt |
| K7-6 | Krasj (prosessdrap) midt i skriving: ingen halv batch, gjenopptak fullfører | Test med tvungen avslutning |
| K7-7 | Én bilde som feiler (panikk, `Unavailable`, korrupt) stopper ikke skanningen og rapporteres i `ScanReport` | Feilinjeksjon |
| K7-8 | Eksakte grupper er tilgjengelige før første miniatyr er hentet (rask fase) | Rekkefølgetest |
| K7-9 | `ALGO_VER`-økning analyserer bare rader med gammel versjon | Integrasjon |
| K7-10 | Originalfiler uendret: sjekksum av fixture-mappen lik før og etter | Arkitekturens test |
| K7-11 | Fremdrift er monotont økende og når 100 % | Enhet |
| K7-12 | Ingen `tokio`, ingen `unsafe` i pipeline (`cargo-deny`, `forbid(unsafe_code)`) | CI |

### Ytelsesmål (indeksering)

| Mål | Budsjett | Måling |
|---|---|---|
| Kjerneberegning per bilde (uten dekoding) | ≤ 10 ms på CI-runner (stopper PR) | `fjell-cli perf` |
| Første indeksering 10 000 bilder | < 10 min på referanseenhet (< 60 ms per bilde) | Nattlig |
| Rask fase (eksakt), 10 000 filer, 10 % størrelseskollisjon, filsystemadapter | < 10 s kjernetid (startverdi) | `criterion` |
| Skanning uten endringer, 10 000 bilder | < 2 s kjernetid (startverdi) | Perf |
| Inkrementell skanning, 100 endrede av 10 000 | < 3 s kjernetid (startverdi) | Perf |
| Databaseskriving | ≤ 1 ms per bilde (batchet) | Perf |
| Minnetopp under skanning | < 150 MB (begrenset kø, ingen full bildeinnlesing) | Perf med RSS-måling |
| Handling mens skanning pågår | p95 < 10 ms kjernekall (egen lesetilkobling) | Perf med bakgrunnslast |

## 9. K8: Kalibrering og herding

Oppgaver:
1. Kjør korpus, sveip Hamming-terskel (4–8) og lag presisjon/recall-tabell. Oppdater `ARKITEKTUR.md` med valgt verdi.
2. Sett `ABS_MAX` for uskarphet og sjekk P10-regelen på skarpt og uskarpt bibliotek.
3. Verifiser `require_strong` for skjermbilde (K4-3) og bestem standard.
4. Mål mot golden-korpus på iPhone og sett tolerance.
5. Aktiver ytelsesgater i CI (`nightly` og PR-proxy).
6. Frys `ALGO_VER = 1` og vektorfil.

Akseptkriterium: alle porter fra ARKITEKTUR del 4 (visuelt dup. presisjon ≥ 0,98 / recall ≥ 0,90, uskarphet presisjon ≥ 0,90, skjermbilde presisjon ≥ 0,95) er grønne i `just check`, og tallene er skrevet inn i dokumentet.

## 10. Risiko

| Risiko | Konsekvens | Tiltak |
|---|---|---|
| Beskjæring 5 % bryter pHash-avstand | Recall under 0,90 | K2b-4 måler separat. Fallback: beskjæringsrobust ekstra hash først etter MVP, eller portjustering i dokumentet |
| Ulik plattformdekoding gir hashdrift | Falske negativer | Måling på iPhone i K8, krav til miniatyr (del 11) |
| Nedskalering skjuler uskarphet | Lav recall for lett uskarphet | Kjent begrensning, relativ σ i korpus, utsnitt i full oppløsning etter MVP |
| Degenererte hasher gir enorme bøtter/kjeder | Treg klynging, falske grupper | `low_info`, bøttekapp, kjedevern |
| Syntetisk korpus er for enkelt | Porter grønne, virkelighet dårligere | ca. 20 ekte lisensfrie foto, beta-test mot eget bibliotek utenfor CI |
| Falske skjermbilder (PNG uten EXIF) | Lav presisjon | `require_strong` |

## 11. Åpne spørsmål til arkitekt (krever endring i `ARKITEKTUR.md` først)

Status etter review: 1, 2, 3 og 6 er besluttet i ARKITEKTUR (beslutning 18, 19). 4 avgjøres i K8. 5 er post-MVP.

1. `AssetSource::list_changes(since) -> Vec<AssetMeta>` kan ikke melde **slettede** assets. Foreslår `ChangeSet { changed: Vec<AssetMeta>, removed: Vec<SourceId>, next_token }`.
2. Hva skjer i indeksen når en fil forsvinner utenfor appen (ikke via «Tøm»)? Forslag: sett `state = purged`, fjern fra grupper og forslag, ingen `action_log`-rad, vises ikke som «frigjort av Fjell».
3. Ny kolonne `fingerprint.fail_count` (K6).
4. Skjerpe skjermbilderegelen til «minst to av fire, og minst én av S1/S2» (K4-3)?
5. Utsnitt i full oppløsning for uskarphet etter MVP: legg `crop_thumbnail(id)` til `AssetSource` da.
6. `AssetMeta` får `platform_screenshot_hint` og `has_camera_exif: Option<bool>` (del 2 under K0).

## Grensesnitt mot andre planer

### Kjerneplanen krever

| Fra | Krav |
|---|---|
| Plattform (iOS, filsystemadapter) | Implementerer `AssetSource`. `thumbnail(id)`: lengste side 256 px, bildeforhold bevart, EXIF-orientering **brukt**, sRGB, Gray8 eller RGBA8, samme nedskalering (høy kvalitet, area/Lanczos, ikke nærmeste nabo). Feil som `Unavailable` (ikke lastet ned) skilles fra `Corrupt`. |
| Plattform | `AssetMeta` fylt ut: `mod_token` stabil og endres ved redigering, `mime`, `bytes`, `width/height` etter orientering, `platform_screenshot_hint`, `has_camera_exif`. `read_bytes` strømmende. |
| Plattform | Bakgrunnsplanlegging (lading, Wi-Fi, `BGProcessingTask`), kall `start_scan` med `deadline`, kall `cancel()` ved utløp. |
| Repo/CI | Workspace, `just check`, `check-layers`, `cargo-deny`, `nightly`-jobb med 10 000/50 000-bibliotek, kjøring på referanseenhet. Matrise macOS/Linux/Windows for hashdeterminisme. |
| Handlinger (papirkurv/angre) | Eier `action_log` og tilstandsendringer. Etter `trash/restore/purge` kaller `api` `pipeline::refresh_groups(affected)` (actions kan ikke kalle pipeline, lag L2 mot L3). Bare `state = active` deltar i klynging. |
| i18n | Nøkler for alle `ReasonCode` (se under) i `nb` og `en`. Paritetstest. |
| UI/UX | Bekrefter at ett kort kan vise forklaring fra `reason_code` og konfidens, og at ingenting forhåndsvelges. |

### Kjerneplanen leverer

| Til | Leveranse |
|---|---|
| `api`, UI | Forslag (`exact_dup`, `visual_dup`, `blurry`, `screenshot`) med `reason_code` og `confidence`. Grupper med keeper og rangering. Fremdrift og `ScanReport`. Eksakte grupper før dekoding er ferdig (rask fase). |
| `api`, UI | `Fjell::start_scan(config)`, `cancel()`, `ScanReport { nye, endrede, fjernede, feilede, tid_per_steg }`. |
| i18n | Komplett liste over begrunnelseskoder, kjernen returnerer aldri tekst: `ExactCopy, VisualDuplicate, Blurry, Screenshot, HigherResolution, LessCompressed, Sharper, Older, ShotPlatformFlag, ShotScreenSize, ShotNoCameraExif, ShotPng`. |
| Handlinger | Stabile `AssetId`, `grp` og `suggestion` med `UNIQUE(asset_id, kind)`. `next_cards` bruker `idx(status, kind, confidence DESC)` (< 5 ms). Behov: `next_cards` skal ikke vise samme asset i flere kort samtidig (prioritet exact_dup > visual_dup > screenshot > blurry avgjøres i `api`). |
| Repo/CI | Golden-korpus med generator, `manifest.json` og porter. Hash-vektorfil. `criterion`-benchmarker og `fjell-cli perf`. |
| Plattform | Kontrakt for `thumbnail` og `AssetMeta` (over), samt `ALGO_VER` slik at plattformen ikke trenger å kjenne til reindeksering. |
| Alle | «Merk»-regel: lagring frigjøres først ved «Tøm». Forslag viser `bytes` slik at UI kan summere «kan frigjøres», men kjernen påstår aldri frigjort plass før `purge`. |
