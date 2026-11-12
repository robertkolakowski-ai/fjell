# Ytelse og robusthet i kjernen (M8, område C)

Målt tall og hva som ikke er verifisert. Kommandoen er `just bench` (= `fjell-cli perf`, release). Budsjettene er fra
`PLAN-KJERNE.md` («Ytelsesmål», K6-4, K7-6) og `ARKITEKTUR.md` del 4. Tallene under er målt på Linux, 4 vCPU
(Xeon 2,1 GHz), 16 GB, ikke på Windows-referansemaskinen.

## 1. Målte tall (release, `just bench`)

| Mål | Budsjett | Målt | Status |
|---|---|---|---|
| Full pipeline, 10 000 syntetiske bilder, per bilde | ≤ 10 ms | 0,40 ms veggklokke (inkl. 0,20 ms CPU til å lage den syntetiske miniatyren) | holder |
| Full pipeline, 10 000 bilder, totalt | < 10 min | 4,0 s | holder (se forbehold) |
| Skanning uten endring, 10 000 bilder | < 2 s | 60 ms | holder |
| Inkrementell, 100 endret av 10 000 | < 3 s | 184 ms | holder |
| K6-4: `upsert` av 10 000 uendrede rader | < 300 ms, 0 endrede rader | 108 ms, `total_changes` uendret (test `store::perf_tests`) | holder |
| `cards` p95, 50 000 rader | < 10 ms | 0,44 ms | holder |
| `decide_many` p95 (1–20 kort), 50 000 rader | < 10 ms | 1,07 ms | holder |
| `undo` p95, 50 000 rader | < 10 ms | 0,87 ms | holder |
| `decide_many`/`undo` p95 under pågående skann (50 000 + 5 000 nye, 3 984 runder) | < 10 ms | 1,44 / 0,91 ms | holder |
| Gruppeoppdatering i bakgrunnen etter én handling, 50 000 rader | ingen port | p95 162 ms | se under |

Pipeline-delene (10 000 bilder): enumerer 66 ms, rask fase 52 ms (3 198 filer hashet; størrelseskollisjonen er ca. 32 %
fordi 10 % er bevisste kopier), full fase 3,8 s, klynging 34 ms (utenom de tidlige oppdateringene hver 2 000. bilde).
`trash`/`undo` på 50 000 rader uten gruppeoppdatering er i tillegg dekket av `actions::tests::perf` (M3-porten).

Syntetiske bilder (`SynthSource`): deterministiske 256×171 RGBA-miniatyrer (grov 17×13-flate pluss støy), ca. 10 % eksakte
kopier, 10 % nesten like, 5 % uskarpe, 3 % skjermbilder; ingen filer, ingen nettverk. Miniatyren lages i hver
`thumbnail`-kall og er med i tiden, så 0,40 ms/bilde er en øvre grense for kjernen alene.

### Forbehold (ikke verifisert)

- **Windows-referansemaskinen er ikke kjørt.** «10 000 bilder < 10 min» gjelder ekte dekoding (JPEG/HEIC) på Windows
  (< 60 ms per bilde); her er dekodingen utelatt (`SynthSource` leverer ferdige miniatyrer), så tallet viser bare at
  kjernen (hash, blur, signaler, klynging, skriving) bruker ca. 0,4 ms/bilde og ikke er flaskehalsen. Dekodetid på
  ekte filer må måles på referansemaskinen (K8).
- Minnetopp under skanning (RSS < 150 MB) er ikke målt.
- Cold start < 1,5 s er ikke målt her (hører til skallet).
- CI kjører `just bench` ikke automatisk ennå; kommandoen avslutter med feil ved brudd, så den kan kobles på `nightly`.

## 2. Funn og rettelser

Første måling (10 000 rader, før rettelsene) brøt budsjettet kraftig; målingen er det som avdekket årsakene:

| Hva | Før | Etter | Årsak og rettelse |
|---|---|---|---|
| `decide_many` p95, 10 000 rader | 307 ms | 1–4 ms (50 000 rader: 1,1 ms) | Etter hver handling regnet kjernen om *alle* grupper (visuell klynging 210 ms ved 9 000 bilder) mens indekslåsen var holdt. |
| `cards` p95, 10 000 rader | 28 ms | 0,4 ms | Den korrelerte delspørringen i «synlig forslag» skannet alle ventende forslag per rad. Ny indeks `idx_suggestion_asset(asset_id, status)` (migrering 2). |
| `plan` i `decide_many`, 10 000 rader | ca. 4 ms | < 0,3 ms | `reasons_for` og `active_members_of` ble drevet fra feil tabell (full skanning av `idx_suggestion_cards` / `idx_asset_state`). `CROSS JOIN` mot `json_each` og `+a.state` tvinger oppslag per id. |

### Indekslåsen under klyngeutregning (punkt 2)

`pipeline::refresh_groups` holdt skrivelåsen under hele utregningen. Nå:

1. Lesing av fingeravtrykk og dagens grupper, utregning og diff (`store::plan_grouping`) skjer på en egen
   lesetilkobling (WAL) utenfor låsen. Låsen tas bare for å sjekke `Store::change_counter()` og for å skrive de endrede
   radene (`Store::apply_plan`).
2. `change_counter` økes av alt som endrer tilstand eller grupper (trash, angre, avstemming, keeper-valg, `apply_plan`),
   ikke av skanningens egne skrivinger. Er den endret imens, forkastes planen og utregningen gjentas (3 forsøk); deretter
   regnes det under låsen, så resultatet alltid er riktig. Minnedatabaser (ingen lesetilkobling) regner alltid under låsen.
3. `GroupEngine` holder `VisualIndex` mellom utregninger (inkrementell klynging: bare berørte komponenter bygges om).
4. Etter en handling (`decide_many`, `undo`, `restore_assets`, `finish_purge`) kjøres gruppeoppdateringen i en sammenslått
   bakgrunnstråd når indeksen har mer enn 2 000 bilder (`Fjell::sync_groups` venter). Under 2 000 bilder er den synkron.
   **Avveining:** i opptil ca. 160 ms (50 000 rader) etter en handling kan forslag i kortlisten være foreldet (kortlister
   filtrerer allerede bort slettede bilder; en keeper som ble felt får ny keeper når oppdateringen er ferdig). Handlingen
   selv er varig og kan angres straks. 162 ms er ikke en port; ønskes lavere, må utregningen gjøres inkrementell
   per berørt gruppe (ikke gjort).

## 3. K7-6 krasjtest (`fjell-cli/tests/crash.rs`)

Ekte prosessdrap, ingen simulering: testen starter `fjell-cli crash-child` og dreper den med SIGKILL
(`Child::kill`, `kill -9`) midt i skriving, og åpner så indeksen på nytt (WAL gjenopprettes).

- `scan`: full pipeline over 1 500 bilder, drept ved fremdrift 1, 100, 600 og 1 200. Etter hver kill: `Store::verify()` er
  tom (SQLite `integrity_check`, `foreign_key_check`, tilstand mot `action_log`), og gjenopptak fullfører med samme kort
  (bilde, type, begrunnelse, konfidens) som en uavbrutt skanning; ny skanning etter det analyserer 0. Gjenopptak
  analyserte 1 500 / 1 500 / 1 200 / 600 bilder, så kill skjedde midt i full fase (testen krever minst én slik runde).
- `actions`: løkke med `decide_many` (20 kort) og `undo` drept på fire ulike tidspunkt. Hver batch i `action_log` har
  nøyaktig 20 rader (ingen halv batch), tilstanden stemmer med loggen, og siste batch kan angres hel.

Kjøretid i debug ca. 70 s (kjører i `just test`). Windows: `Child::kill` er `TerminateProcess` (samme uten opprydding),
men testen er bare kjørt på Linux; den kjører i CI på `windows-latest` med resten av workspace.

## 4. Øvrig levert

- **Fjernede filer (T11), `fjell-fs/src/source.rs`:** `FsSource::with_known_ids(..)` gir kjente id-er (indeksens AKTIVE
  bilder, `Store::active_source_ids_at`); ved inkrementell skanning meldes kjente id-er som ikke finnes i en vellykket,
  full gjennomgang som `removed`. Frakoblet rot er `Unavailable`, og en tom gjennomgang når indeksen kjenner filer regnes
  også som `Unavailable` (tom monteringsmappe), aldri `removed`. Uten `with_known_ids` meldes ingenting. Papirkurvbilder
  er med vilje ikke «kjente» (de ligger i karantenen). CLI kobler det til indeksen (`FsSource::with_index`); Tauri-skallet
  må gjøre det samme (`with_known_ids`) for å få T11 i appen (ikke gjort, utenfor mitt område). Tester:
  `fjell-fs/tests/removed.rs`.
- **`keeper_reason` lagres:** migrering 2 (`grp.keeper_reason`); `CardRow.keeper_reason` (ikke i `api::Card` ennå, for å
  la grensesnittet stå uendret); nullstilles ved manuelt keeper-valg.
- **Mass-ack:** `api/hardening_tests.rs` (trash og purge: `AckRequired([MassDelete])` uten bekreftelse, ingenting skrevet,
  feil kode dekker ikke, delvis `DeleteOutcome` lar resten ligge i papirkurven, foreldet mengde gir ingen billett) og
  `fjell-cli/tests/mass.rs` (60 par: `trash`/`purge` uten `--yes` gir exit 3 og ingen endring; med `--yes` fungerer det,
  originalene er uendret).
