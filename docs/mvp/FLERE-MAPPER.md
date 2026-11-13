# Flere mapper og formater (M16)

Status: bygget og testet på Linux. Ikke kjørt på Windows, og Tauri-limet (`desktop/src-tauri/src`) er ikke
kompilert her. UI kommer i bølge 2; kontrakten ligger i `desktop/src/bridge/types.ts` (`LibraryRoot`).

## Modell

- Ett bibliotek = én indeks og en ordnet liste røtter (mapper). Første rot har nøkkel 1 og eier indeksen:
  `config::db_path(første rot)` er uendret, og id-ene der er som før (relativ sti, ingen prefiks). Et
  énrots-bibliotek er derfor bit-for-bit det samme som før; flere røtter utvider samme indeksfil.
- Øvrige røtter har nøkkel ≥ 2 og id `/r<nøkkel>/<relativ sti>` (`fjell_fs::roots`). En relativ sti begynner
  aldri med `/`, så prefikset kan ikke kollidere med en fil i første rot. (`r2:sti` ble vurdert og forkastet:
  `:` er lovlig i filnavn på Linux.) `path::resolve` avviser alt som begynner med `/`, så en id fra en annen
  rot kan ikke løses mot feil rot ved en feil.
- Nøkler gjenbrukes aldri (`next_key` bare øker): en gammel rad `/r2/…` kan ikke peke inn i en ny mappe.
- `shell.json`: `library` er fortsatt første rot. Nytt felt `libraries` (`#[serde(default)]`), nøklet på
  indeksfilens navn: `{ next_key, roots: [{ key, path }] }`. Gamle filer leses uendret.
- Røtter kan ikke overlappe (lik, inni eller rundt en annen), sammenlignet kanonisert (symlenker løst).
  Frakoblet mappe sammenlignes leksikalsk. Avvises med `root_overlap`.

## Skanning (`MultiSource`)

- Én `FsSource` per rot. Endringstoken: `fs1:<s>` med bare første rot (uendret), ellers `fsm1:1=<s>,2=<s>`.
  En rot uten verdi i tokenet (nylig lagt til) listes fullt; resten av biblioteket skannes inkrementelt.
- Frakoblet rot hoppes over: ingenting meldes endret eller fjernet, og tokenet står stille. Bare når ALLE
  røtter er frakoblet gir skanningen `Unavailable` (som før for én rot).
- Rader fra en rot som er tatt ut av biblioteket meldes fjernet ved neste inkrementelle skanning.

## Karantene, angre og «Tøm» per rot (`RootSet`)

- Hver rot har sin egen karantene, `<rot>/.fjell-karantene`. Flytting er alltid `rename` innenfor roten, altså
  samme volum. Det finnes ingen felles karantene og ingen flytting mellom røtter.
- All ruting skjer på rotprefikset i INDEKSENS sti: stage, angre/gjenopprett, avstemming, miniatyr av trashet
  bilde og «Tøm». En id fra rot A slås aldri opp i karantene B. En sti uten kjent rot flyttes og slettes aldri.
- Tøm-invarianten gjelder per rot og er ikke endret: én `FsDeleter` per rot, bundet til den rotens karantene
  (`locate_for(id, original)`, `safe_stored`, symlenke-vern, ingen fallback til opprinnelig sti). Én billett
  dekker hele tømmingen; `flow`-låsen holdes som før.
- En manifestoppføring i «feil» karantene (fremmed eller tuklet) regnes ikke som «i karantene» for bildet
  (`Core::staged_own`), og kan ikke få indeksen til å peke over i en annen rot (`relocate` krever samme mappe
  inkludert prefiks).
- Når røttene endres, pensjoneres den gamle kjernen under `flow`-låsen; senere kall på den gir `unavailable`.

## Skymappe-sperre per rot

- En rot i iCloud/OneDrive/Dropbox (navnebasert, som før) sperrer papirkurv for bildene i DEN roten.
  Et kall som inneholder minst ett slikt bilde avvises helt (`cloud_synced_readonly`), før noe endres.
- `StateDto.cloud_sync` settes bare når ALLE røtter er skysynket (én rot: som før); ellers `roots[].cloud_sync`.

## Skall-API

| Kall | Virkning | Feilkoder |
|---|---|---|
| `list_library_roots` | Røtter med `online`, `count`, `bytes`, `videos`, `video_bytes`, `trashed` | `no_library` |
| `add_library_root(path)` | Ny rot; bilder kommer ved neste skanning | `not_a_folder`, `root_overlap`, `root_has_quarantine` |
| `remove_library_root(key)` | Tar ut roten; ingen filer røres | `root_is_primary`, `root_has_trash`, `not_found` |
| `set_library(path)` | Som før: erstatt med én rot | som før, pluss `root_has_trash` |

- `root_has_trash` (antall i `detail`): roten har bilder i «Nylig slettet». Gjenopprett eller tøm først.
- `root_has_quarantine`: mappen har en `.fjell-karantene` med innhold fra et annet bibliotek (annen indeks).
- `StateDto.roots` har røttene uten tall og uten filsystemtilgang (pollingen skal være billig).

## Formater (`fjell_fs::source::SCAN_FORMATS`)

- Bilder: jpg, jpeg, png (dekodes overalt); heic, heif, avif, dng, cr2, cr3, nef, arw, orf, rw2, raf (dekodes
  bare via WIC på Windows). Uten dekoder: synlige og telt, bredde/høyde 0, miniatyr `Unavailable` (ikke feil),
  ingen visuell analyse, men de hashes og kan være eksakte duplikater.
- Video: mp4, m4v, mov, avi, mkv, webm, wmv, 3gp. Dekodes aldri. Telles og listes med størrelse.
- ENDRING I KJERNEN: video var tidligere holdt utenfor både hashing (`Store::hash_rows`) og eksakte grupper
  (`pipeline/groups.rs`). Begge filtrene er fjernet, så bytelik video nå gir eksakt-duplikat-forslag. Video er
  fortsatt aldri med i full fase (`work_set`), visuell klynging, uskarphet eller skjermbilde.

## Kjente begrensninger

- Første rot kan ikke fjernes eller byttes uten å bytte bibliotek (`set_library`), og da mister de andre
  røttene plassen sin (radene deres fjernes ved neste skanning; beslutninger på dem går tapt).
- Går `shell.json` tapt, er ekstra røtter ukjente. Bilder som lå i deres karantene blir liggende der (med
  README), vises i «Nylig slettet», men kan ikke flyttes tilbake av appen. Ingenting slettes.
- Overlapp mellom en frakoblet og en tilkoblet rot sammenlignes bare som tekst (Windows: `\\?\`, store/små bokstaver).
- Blandet kall med ett bilde i skyrot avvises helt; UI må dele opp eller forklare.
- AVIF/RAW via WIC er aldri kjørt mot ekte filer. Ytelse med mange røtter og store videoer er ikke målt.
