# Lagringsoversikt (kjerne)

Status: kjernen er på plass (bølge 1). Skall og UI kommer i bølge 2. Kode: `core/crates/fjell-core/src/storage/`.

## Hva den gir
Én rent lesende oppsummering av indeksen: `Fjell::storage_summary()` (eller `storage::summarize`). Alt regnes
fra `asset.bytes`, uten dekoding og uten filtilgang. Video og bilder som ikke er lastet ned teller derfor
med full størrelse. Ingenting skrives, og ingen handling (angre, karantene, Tøm) berøres.

| Felt | Innhold |
|---|---|
| `total_count`, `total_bytes` | aktive bilder (ikke i «Nylig slettet», ikke purget) |
| `per_kind` | alltid Foto, Video, Annet (`ScreenshotHint` er Annet); også med 0 |
| `per_root` | én rad per rot. Rot 0 i dag; `root_of` i `storage/scan.rs` er eneste sted som antar én rot |
| `per_folder` | topp-N mapper (standard 20) med bytes og antall, rot-relative, aggregert opp til N nivåer (standard 3) |
| `largest_files` | topp-N filer (standard 20) med id, sti, størrelse og type; `Fjell::largest_files(n)` gir bare denne |
| `reclaimable_by_category` | eksakte duplikater, like, uskarpe og skjermbilder, alltid fire rader |
| `trash_count`, `trash_bytes` | innhold i «Nylig slettet» |

## Regler og valg
- Mapper er hierarkiske: en forelder teller alt under seg, så rader med ulik `depth` overlapper. Ikke summer dem.
  Filer rett i roten inngår bare i `per_root`. Mapper identifiseres på full rot-relativ sti, så like navn under
  ulike foreldre holdes fra hverandre.
- «Plass bak forslag» bruker samme synlighetsregel som kortene (ventende forslag på aktivt bilde, høyest
  prioritet vinner), så hvert bilde teller i én kategori og kategoriene kan summeres
  (`reclaimable_total_bytes`). Det er plass forslagene peker på, ikke noe appen vil slette: ingenting
  slettes automatisk, og alt går via karantene og kan angres. Samsvar med `Store::kind_totals` er testet.
- Sortering er deterministisk: størst først; lik størrelse gir sti (og rot) stigende, så id.
- Summer er `u64` med metning, aldri omløp. Filstørrelse lagres som `i64` i indeksen (tak `i64::MAX` per fil).
- Ytelse: ett strømmende pass over aktive rader (minne = antall mapper, ikke rader) pluss to lette pass for
  forslag og papirkurv i én lesetransaksjon. For filbasert indeks leses det via `Store::reader()` slik at
  skanning og handlinger ikke blokkeres. Minnedatabase leses under indekslåsen.

## Flere røtter
Skjemaet har ingen rotkolonne ennå. Feltene `root` finnes allerede i `RootUsage`, `FolderUsage` og
`FileUsage`, og aggregeringen er nøklet på rot. Når flere røtter innføres: legg til kolonnen og la `root_of`
lese den. Kontrakten mot skallet endres ikke.

## Måling
`cargo test -p fjell-core perf_100k` bygger en filbasert indeks med 100 000 rader (to forslagslag, 5 % i
papirkurv) og krever < 150 ms i release (faktor 25 i debug). Resultat: se PR-beskrivelsen.
