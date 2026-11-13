# Red team (Sonnet): funn mot Fjells sikkerhetsregler

Dato: 2026-11-13. Omfang: `fjell-fs` (quarantine, delete, path, source), `fjell-core` (actions, store, api) og
`fjell-cli` (flow), via offentlig API. Ingen kildekode er endret; funnene er ikke fikset (egen pakke).
Reprodusenter: `core/crates/fjell-cli/tests/redteam_sonnet.rs`, alle `#[ignore = "BUG: ..."]`.

Kjør alle reprodusenter (skal feile på dagens kode):

    cd core && cargo test -p fjell-cli --test redteam_sonnet -- --ignored

Vanlig `cargo test` er grønn (angrep som holdt kjører som vanlige tester). Kjørt på Linux; alle sju
Linux-reprodusenter ble kjørt med `--ignored` og feilet av årsaken som er beskrevet. RT-07 er Windows-only og er IKKE kjørt.

| Id | Alvor | Funn | Sted | Reprodusent | Status |
|----|-------|------|------|-------------|--------|
| RT-04 | middels | Symlenker i biblioteket: sløyfe (`loop -> .`) gir samme fil under 41 `source_id`-er, fil-symlenke (`a.jpg -> x.jpg`) regnes som eksakt duplikat. Appen foreslår da en fil som duplikat av seg selv; «slett duplikatet» kan flytte den eneste kopien, og keeper peker på samme (nå flyttede) fil. Angre virker, men forslaget er feil og bryter «keeper beholdes». | `fjell-fs/src/source.rs` `walk` (`is_dir()` følger lenker, ingen kanonisering/dedup) | `rt04a_symlink_loop_lists_each_physical_file_once`, `rt04b_file_symlink_is_not_a_duplicate_of_its_target` | åpen |
| RT-01 | middels | `stage` med samme id to ganger i ett kall: andre `rename` feiler, og `drop_entries(failed)` fjerner alle oppføringer for id-en. Filen ligger i karantenen uten manifest (foreldreløs); `unstage` gir `NotFound`, Angre/Gjenopprett umulig uten manuell redning. Krever duplikat i inputlisten (kjernen dedupliserer vanligvis). | `fjell-fs/src/quarantine.rs` `stage` (failed-håndtering) og `drop_entries` | `rt01_stage_duplicate_id_in_one_call_keeps_file_restorable` | åpen |
| RT-02 | middels | Id-gjenbruk (indeks nullstilt, ids starter på 1 igjen): `stage(1, "new.jpg")` svarer `Ok(())` uten å flytte når id 1 allerede er staged for «old.jpg». Indeks og karantene spriker, og `locate(1)` (brukt av «Tøm») peker på den gamle filen, så Tøm kan slette en annen fil enn den brukeren bekreftet. Avmildet av at oppstartsavstemming vanligvis gjenoppretter gamle oppføringer først. | `quarantine.rs` `stage`: `if already.contains(&id.0) { Ok(()) }`; `delete.rs` `path_for` | `rt02_stage_reused_id_must_not_report_ok_without_moving` | åpen |
| RT-03 | lav | `manifest.json` er data fra disk (kan komme via synket mappe), men `stored` valideres ikke. `stored = "../../../x"` får `unstage` til å flytte en fil utenfor karantenen inn i biblioteket; `locate` + `FsDeleter` kan på samme måte slette en vilkårlig fil ved «Tøm». | `quarantine.rs` `locate`/`move_back`, `manifest.rs` `Entry.stored` | `rt03_manifest_stored_traversal_does_not_touch_foreign_files` | åpen |
| RT-05 | lav | Vernet «indeksfilen må ligge utenfor kildemappen» er leksikalsk (`db.starts_with(dir)`). Kilde-sti med `..` (eller symlenke/relativ vs. absolutt) omgår det, og SQLite + WAL skrives i brukerens mappe. Originalfiler berøres ikke. | `fjell-cli/src/flow.rs` `open` | `rt05_index_db_inside_source_dir_is_rejected_even_with_dotdot` | åpen |
| RT-06 | lav | `FjellApi::cards(kind, usize::MAX, ..)`: `limit + 1` uten overflow-vern. Panikk i debug (kan krysse FFI), i release blir grensen 0 og listen tom (kort ser ut til å forsvinne). | `fjell-core/src/api/fjell_ops.rs:179` | `rt06_cards_limit_max_does_not_panic_or_return_empty` | åpen |
| RT-07 | lav (uverifisert) | `path::resolve` splitter bare på `/`. På Windows er `\` skilletegn, så `..\..\x.jpg` og `C:\x.jpg` passerer og `root.join` forlater roten (absolutt sti erstatter roten). Krever manipulert indeks/manifest. Utledet fra koden; ikke kjørt (windows-latest i CI er eneste Windows-kjøring). | `fjell-fs/src/path.rs` `resolve` | `rt07_resolve_rejects_backslash_traversal_and_drive_paths_on_windows` (`#[cfg(windows)]`) | åpen, ubekreftet |

## Observasjon uten test
- Etter én vellykket «Tøm» (purge-batch) gir `undo()` `NotUndoable` for alltid, så tidligere batcher kan ikke
  angres via angre-stakken (bare via Gjenopprett fra «Nylig slettet»). Dokumentert i PLAN-TRYGGHET (T13), men
  konsekvensen for eldre batcher er verdt en produktbeslutning.
- `flow::purge --fs-temp` kopierer med `is_dir()` (følger symlenker): sløyfe eller lenke til stor mappe fyller
  temp og kopien ryddes ikke ved feil. Bare utviklerverktøy; samme rot som RT-04.

## Angrepsflater undersøkt uten brudd (dekket av vanlige tester i samme fil)
- Sti-traversering i `stage` (`..`, absolutt, tom): avvist, ingenting flyttet.
- Restore til opptatt sti: aldri overskriving, gir `navn (gjenopprettet).ext`.
- Rundtur stage/unstage med Unicode, mellomrom og 200+ tegn: bytes uendret.
- Dobbel `stage`/dobbel `unstage` (sekvensielt): idempotent, ingen tap.
- Frakoblet rot: `reconcile` gir `Unavailable`, ingenting markeres borte, alt gjenopprettes ved tilkobling.
- Krasj mellom manifest og flytting: `NotMoved`, filen urørt.
- `FsDeleter`: permanent uten `PermanentAck` gir `AckRequired`; `PermanentAck::new` avviser tom avkrysning og antall 0.
- Lest, men ingen brudd funnet: billett-/digestlogikk i `actions/purge.rs` og `commit.rs` (engangsbillett, digest-binding,
  `NotTrashed`/`AlreadyPurged`), `decide` (ukjent/avgjort kort stopper hele runden, Trash+Keep på samme bilde avvises),
  `guard_groups` (I8), undo-rekkefølge og hopp over avvikende rader. Disse er lest, ikke fuzzet.

## Ikke undersøkt
Samtidige skall-kall og `desktop/src-tauri/shell` (decide/empty/restore/undo), WindowsRecycleBin, skymapper/UNC
(logikken i `windows.rs` er Windows-only), filer som endres midt i en handling (TOCTOU i `restore_target`: sjekk
`exists()` og `rename` er ikke atomiske; på Unix overskriver `rename` en fil som dukker opp i mellomtiden).
