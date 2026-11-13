# Red team: Sonnet mot Fable 5.2 (én oppgave, n=1)

Dommer: nøytral Sonnet-agent. Alle reprodusenter er kjørt med `cargo test --offline -p fjell-cli --test <fil> -- --ignored` på Linux. Alle feilet på den beskrevne bruddpåstanden (ikke på oppsettsassert). Ingenting er kjørt på Windows.

## Tabell per modell

| | Sonnet | Fable 5.2 |
|---|---|---|
| Påstander | 7 (RT-01..07; RT-04 har to tester) | 5 (F1..F5) |
| Bekreftet reell feil | 6 (RT-01..06) | 5 |
| Falske positiver | 0 | 0 |
| Ubekreftet (ikke kjørbar her) | 1 (RT-07, Windows) | 0 |
| Delt med den andre | 1 (RT-03 = F3) | 1 (F3 = RT-03) |
| Unike bekreftede | 5 | 4 |
| Høyeste alvorlighet (min vurdering) | Middels | Høy |
| Angrepsflater dekket | stage/unstage med id-gjenbruk og dobbel id, symlenker i kilden, manifest, indeks-db i kildemappe, cards-grenser, traversering, Unicode/lange navn, frakoblet rot, krasj-reconcile | skall-flyt (restore, angre, Tøm), kollisjon ved gjenoppretting, purge etter bekreftelse, manifest, skysperre, plan/commit/purge-billetter, gruppevern, utløp, krasj-reconcile |

## Samlet feillisten (min alvorlighet)

| Id | Funn | Funnet av | Alvor | Merknad |
|---|---|---|---|---|
| F1 | Navnekollisjon ved gjenoppretting: skallet kaster unstage-resultatet, indeksen peker på gammel sti, neste Tøm sletter ny, aldri foreslått fil | Fable | Høy | Bryter «ingen sletting uten forslag». Vist med skallets kall gjentatt i fjell-cli-test, ikke gjennom Shell |
| F2 | Gjenopprettet etter confirm_purge: path_for faller tilbake til original sti, Tøm sletter aktiv fil | Fable | Høy | Samme forbehold; kappløpet mellom tråder er utledet, ikke kjørt |
| F4 | Angre av «Gjenopprett» gir trashed i indeks, men filen flyttes ikke tilbake til karantene; Tøm sletter fra biblioteket | Fable | Middels-høy | Brudd på «alt kan angres»-konsistens, men filen slettes bare via bekreftet Tøm |
| F3 = RT-03 | manifest.stored valideres ikke (`..`): Tøm/unstage rører fil utenfor biblioteket | Begge | Middels | Krever endret manifest (lokal angriper/korrupsjon). Begge reprodusenter feiler |
| F5 | Symlenket synkmappe omgår skysperren, stage flytter filen ut | Fable | Middels | Kun Unix-symlenke kjørt |
| RT-01 | Dobbel id i én stage-kall gir foreldreløs fil i karantene (restore gir NotFound) | Sonnet | Middels | Krever feilaktig kall; filen ligger igjen, ikke tapt |
| RT-02 | Gjenbrukt id: stage meldte Ok uten å flytte filen | Sonnet | Middels | Falsk suksess |
| RT-04 | Symlenker (fil og løkke) listes som duplikater; løkke gir 41 id-er | Sonnet | Lav-middels | Støy/duplikater, ressursbruk |
| RT-05 | Indeks-db i kildemappe avvises ikke når stien har `..` | Sonnet | Lav | |
| RT-06 | `cards(usize::MAX)` overflow-panikk (fjell_ops.rs:179) | Sonnet | Lav | Panikk kun i debug/overflow-sjekk |
| RT-07 | Windows backslash i resolve | Sonnet | Ukjent | Ikke kjørt, ikke tellet som bekreftet |

## Ærlig konklusjon

- Fable ga merverdi i alvorlighet: de to høye funnene (F1, F2) og F4 gjelder flyt på tvers av lag (skall, indeks, karantene, sletter) og truffet kjerneløftet om at ingenting slettes uten forslag. Sonnet fant ingen av dem.
- Sonnet ga merverdi i bredde på lavere nivå: 5 unike bekreftede funn (id-gjenbruk, dobbel id, symlenker, db-vern, overflow) som Fable ikke dekket, og delte ett funn.
- Ingen falske positiver hos noen. Antall påstander og rapportlengde er ikke vektet.
- Forbehold: n=1 oppgave, ett utfall. Fables høye funn er vist ved å gjenta skallets kall utenfor skallet og må bekreftes i `desktop/src-tauri/shell` før retting. Tidsbruk og tokenforbruk er ikke målt likt og er ikke sammenlignet. Funn er ikke blindt rangert av en uavhengig tredje part utover denne dommeren.
- Praktisk: bruk Fable til tverrgående flyt/sikkerhetsrevisjon, og Sonnet til bred grensetesting; de to supplerer hverandre her.

## Prioritert rettelsesliste (kun bekreftede)

1. F1: skall `core.rs:191` (bruk unstage-resultat, oppdater indeksen), store/ops.rs
2. F2: `fjell-fs/src/delete.rs:124-129` (path_for uten fallback), `Core::empty` (check_ticket per fil, lås mot restore)
3. F4: skall `lib.rs:352` (undo av Gjenopprett flytter tilbake til karantene)
4. F3/RT-03: `fjell-fs/src/quarantine.rs` locate (valider `stored`, avvis `..`/absolutt)
5. F5: `fjell-fs/src/source.rs:144` og stage (skysperre på kanonisert sti)
6. RT-02 og RT-01: stage-validering av id (avvis dobbel/gjenbrukt id, ikke Ok uten flytting)
7. RT-04: ikke følg symlenker i FsSource (eller dedupliser på kanonisk sti)
8. RT-05: kanoniser sti før db-vern
9. RT-06: `fjell-core/src/api/fjell_ops.rs:179` (saturating_add)

## Status etter kritikerrunden (arbeidstre m15, ikke committet, kun Linux)

| Id | Status | Merknad |
|---|---|---|
| F1 | Rettet i produksjon, reprodusent ÅPEN | Dekket av 6 skalltester (`tests_flow.rs`); `redteam_fable` F1 står ignorert (hjelper kopierer gammelt lim) |
| F2 | Rettet | Reprodusent grønn; Tøm blokkerer angre/gjenopprett (brukersynlig, se BESLUTNINGER) |
| F4 | Rettet i produksjon, reprodusent ÅPEN | Samme som F1 |
| F3 = RT-03 | Rettet | Strengvalidering; KR-2 (symlenket batchmappe) rettet i tillegg |
| F5, RT-04 | Rettet | Symlenker hoppes over (brukersynlig, utsatt avklaring); skysperre på kanonisert sti utsatt (KR-5) |
| RT-01, RT-05, RT-06 | Rettet | Reprodusenter grønne |
| RT-02 | Delvis, reprodusent ÅPEN | Err uten flytting; testen krever designvalg |
| RT-07 | Åpen | Windows, ikke kjørt |
| KR-1, KR-2 | Rettet | Reprodusenter grønne |
| KR-3 | Årsak rettet, reprodusent ÅPEN | Oppsett feiler etter rettelsen; se BESLUTNINGER |
| KR-4..KR-7 | Utsatt | Lav alvorlighet |

Åpne reprodusenter (`--ignored`): RT-02, F1, F4 (core) og KR-3 (skall) = 4.
