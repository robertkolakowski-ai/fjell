# Ytelse på store bibliotek (100 000 og 250 000 bilder)

Oppfølging av funn 20 i `MVP-PLAN.md` og av `YTELSE.md`. Alle tall er målt på **Linux, 4 vCPU (delt maskin), release**, ikke på
referansemaskinen (Windows) og ikke på iPhone. Maskinen kjørte andre arbeidstrær samtidig (load 7–15 på 4 kjerner), så
**veggklokketidene under er støyfulle** (opptil 2x). CPU-tid (`user`) og RSS er mer pålitelige og er oppgitt der de finnes.
Måling: `FJ_BUILD_ONLY=1 fjell-cli perf --n 200 --rows N` (bygger indeksen, skriver tid per trinn og topp RSS, og en hash av alle
kort for før/etter-sammenligning). `FJ_PROF=1` skriver i tillegg RSS etter bygging i del 3.

## 1. Før og etter

| Rader | Veggklokke før | Veggklokke etter | CPU-tid per rad før → etter | Topp RSS før → etter |
|---|---|---|---|---|
| 25 000 | 8,4 s | 17 s (load 10, CPU 25,5 s) | 1,04 → 1,02 ms | 53 → 41 MB |
| 50 000 | 20,6 s (28,6 s ved load) | 34 s (load 12, CPU 50,7 s) | 1,14 → 1,01 ms | 92 → 70–73 MB |
| 100 000 | 70–72 s | **32,6 s** (rolig kjøring), 54–66 s ved load 8–12 | 1,26 → 1,04 ms | 176 → **122–128 MB** |
| 250 000 | 223 s | 131–163 s (load 7–12) | ikke målt → 1,04 ms | 437 → 277–280 MB |

CPU-tid per rad er flat etter rettelsene (1,02 / 1,01 / 1,04 / 1,04 ms ved 25k / 50k / 100k / 250k, forhøyet av delt maskin)
og var stigende før. Veggtiden per rad var før 0,34 / 0,41 / 0,71 / 0,89 ms. Ved 100 000 er veggtiden redusert med ca. 55 % i
rolig kjøring. 250 000 ble lest ut for første gang. Hva en rolig maskin gir ved 250 000 er ikke målt.

**Samme resultat:** utdatahashen (alle kort: type, bilde, keeper, begrunnelse, konfidens, byte, beskyttelse, sortert) er lik
før og etter ved 25 000 (`b0fc4f4d64e2dd41`), 50 000 (`a9a2338de59aefa9`) og 100 000 (`81c79b8d6c37f799`). Alle 27 testresultater i
`cargo test --workspace` og alle golden-/testdata-tester er grønne uten endring i eksisterende tester.
Ved 250 000 er utdata **ikke deterministisk**, verken før (73 383 og 72 675 kort i to kjøringer) eller etter (71 769 og 72 151
i to kjøringer); se «Funn som ikke er rettet» under.

## 2. Hva som ble funnet (profilering per trinn)

Midlertidig tidtaking i `refresh_groups` (lesing, utregning, plan, skriving), fjernet igjen. Ved 100 000 rader kjørte full fase
58 tidlige gruppeoppdateringer (hver 2 000. bilde) og **skriveren stod i dem i ca. 29 s av 70**; analysen (syntetisk miniatyr,
hash, uskarphet) er lineær ved ca. 0,28 ms/bilde. Mens skriveren regner, blokkerer den begrensede kanalen og arbeiderne står.
Summert per kilde (ms, 100 000 rader):

| Del av `refresh_groups` | Før | Etter alle rettelser |
|---|---|---|
| lese alle rader | 2 600 | 700 |
| utregning (`compute_with`, MIH) | 17 400 | 3 300 |
| plan (`plan_grouping`) | 3 000 | 500 |
| skriving (`apply_plan`) | 5 800 | 440 |
| antall oppdateringer | 58 | 20 |

## 3. Rettelser (ingen funksjonsendring)

1. **`store/003_suggestion_grp_index.sql`: indeks på `suggestion(grp_id)`.** Kolonnen har `ON DELETE SET NULL`; uten indeks
   skannet SQLite hele forslagstabellen for hver slettet gruppe (kvadratisk). Skriving 5,8 s → 0,44 s ved 100 000. Ny migrering
   (versjon 3), `idx_suggestion_grp`; test `suggestion_grp_id_has_index_for_group_deletes` sjekker spørringsplanen.
2. **`cluster/visual.rs`, `neighbors`.** Alle kandidater fra åtte bøtter ble satt inn i et `BTreeSet` før avstanden ble sjekket
   (ca. N/32 kandidater per nøkkel, så kvadratisk med stor konstant). Nå sjekkes avstanden underveis og bare treffene
   sorteres og dedupes: samme utdata (sortert, uten duplikater), utregning 17,4 s → 6,9 s. Bøttene og MIH-oppsettet er
   uendret (kandidatmengden er den samme, så garantien for `max_dist` ≤ 7 og `bucket_cap` er uendret).
3. **`pipeline/full.rs`: geometrisk tidlig visning.** Avstanden mellom tidlige gruppeoppdateringer er `max(2 000, analysert / 4)` i
   stedet for fast 2 000. Hver oppdatering leser og regner over hele indeksen, så fast avstand ga kvadratisk total. Nå 20 i stedet
   for 58 ved 100 000 (ca. 25 ved 250 000). Første visning etter 2 000 bilder er som før; sluttresultatet regnes alltid til slutt.
4. **`cluster/visual.rs`, `update`: ingen komponentoppføring for enslige nøkler uten gruppe.** Det vanlige tilfellet (de fleste
   bilder er ikke duplikater) kostet en `BTreeSet` og to kart-oppføringer, ca. 250 byte per bilde. Slike nøkler finnes igjen som
   naboer når en ny nøkkel settes inn (avstand er symmetrisk, og lik bøtteverdi gir samme bøtte), så resultatet er likt.
   `bucket_cap` gjør naboskapet asymmetrisk (en overfull bøtte hoppes over bare fra egen side), men en enslig nøkkel uten
   gruppe kan bare bygges om via en komponent som inneholder den, og slike har fortsatt oppføring; utdata er derfor uendret
   (kontrollert med utdatahash og golden-tester). Hashen dekker ikke kort-rekkefølge eller gruppe-id. RSS ved
   100 000: 166 → 122–128 MB.
5. **`pipeline/mod.rs`: `drop(changes)`** rett etter at endringslisten er skrevet (metadata for alle nye bilder lå ellers i
   minnet gjennom hele analysen).
6. **`fjell-cli/src/perf.rs`:** `FJ_BUILD_ONLY` (skaleringsmåling med utdatahash), `FJ_PROF` (RSS etter bygging i del 3), nytt
   budsjett (se under) og tydeligere RSS-tekst.

## 4. Budsjetter (Linux, 4 vCPU, ikke referansemaskin)

| Mål | Budsjett | Målt (100 000 rader) | Status |
|---|---|---|---|
| Topp RSS etter bygging (selve skanningen), opptil 100 000 rader | ≤ 150 MB (**håndheves nå i `just bench`**, bare release) | 122–128 MB | holder |
| Topp RSS etter bygging, 250 000 rader | ca. 1,1 KB per rad, ikke budsjettert | 277–280 MB | over 150 MB |
| CPU-tid per rad, hele byggingen | omtrent konstant (ingen ny automatisk port; flat i målingen over) | 1,0 ms (delt maskin) | holder |
| `cards` / `decide_many` / `undo` p95 | < 10 ms | 0,49 / 4,5 / 0,56 ms | holder |
| Gruppeoppdatering i bakgrunnen etter handling | ikke en port | 224 ms ved 100 000 (79 ms ved 50 000) | uendret |

Standardkjøringen `just bench` (10 000 + 50 000) gikk grønn etter rettelsene (topp RSS 148 MB for hele prosessen, 75 MB etter
bygging). `just bench --rows 100000` ga to ganger brudd på **decide_many under pågående skann** (19,1 og 12,1 ms mot 10 ms),
og en kjøring med `MALLOC_ARENA_MAX=2 --rounds 20` gikk grønn. Målingen skjedde på en maskin med load 11–15, så om bruddet
skyldes støy eller reelt tap ved 100 000 er **ikke avgjort**.

## 5. Funn som ikke er rettet, og hva som gjenstår

- **Topp RSS for hele `just bench --rows 100000` er 190–301 MB** (funn 20 oppga 294 MB), ikke 150. Byggetrinnet er 122–128 MB;
  resten kommer i del 3 (inkrementell skanning med 10 000 nye bilder mens handlinger kjører). Der kan en bakgrunnsoppdatering og
  skannerens egen oppdatering regne samtidig og hver ha egen kopi av alle rader og kart (anslått 50–60 MB per oppdatering; ikke
  profilert). glibc sine per-tråd-arenaer står for en stor del: med `MALLOC_ARENA_MAX=2` var byggetrinnet 100 MB og hele
  kjøringen 154 MB (én kjøring, `--rounds 20`). Budsjettet 150 MB er derfor holdt for byggetrinnet, ikke for hele del 3.
  Neste skritt: begrense til én gruppeutregning om gangen (eller dele `rows`), og sette `MALLOC_ARENA_MAX` eller en annen
  allokator i skallene (Linux/Windows/iOS har ulik allokator, ikke målt).
- **`bucket_cap` (500) gir ikke-deterministisk og avkuttet klynging ved ca. 128 000 bilder og over.** Med 8-bits blokker er
  en bøtte ca. N/256 nøkler, så over ca. 128 000 hoppes bøtter over i `neighbors`, og hvilke par som mistes avhenger av
  rekkefølgen nøklene kom inn i (tidspunkt for tidlig visning). Dette er et resultat, ikke en ytelsesfeil, så det er
  **ikke endret** (oppdraget forbyr funksjonsendring). At tiden per rad er flat ved 250 000 skyldes delvis at slike bøtter
  hoppes over og dermed er billige. Rettelse krever et designvalg (flere/ulike blokker, eller `bucket_cap` som funksjon av N) og
  ny golden-baseline.
- Resterende minne er ca. 1,1 KB per rad og lineært: `WorkItem`-listen (tre `String` per bilde) i full fase, `IndexRow`-lista og
  tre `BTreeMap` per oppdatering, `VisualIndex` (åtte bøtter med `BTreeSet<Key>`), og SQLite-cache. Strømming av arbeidssettet i
  id-områder og kompakte bøtter (`Vec` i stedet for `BTreeSet`) er mulige tiltak, ikke gjort.
- Det er ikke lagt inn automatisk kompleksitetstest (tid per rad ved 2 størrelser). Anbefalt: kjør `FJ_BUILD_ONLY` ved 25k og
  100k i nightly og krev at CPU-tid per rad ikke vokser mer enn 25 %.
- `docs/mvp/MVP-PLAN.md` rad 20 er ikke oppdatert (utenfor mitt område); den bør henvise hit.
