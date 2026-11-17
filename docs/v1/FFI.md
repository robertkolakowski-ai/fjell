# V1: FFI (fjell-ffi)

`fjell-ffi` er UniFFI-speilet av `FjellApi`. Det har ingen logikk: bare typekonvertering og delegering til
`fjell_core::api::Fjell`. Formålet er at en senere iPhone/Android-fase kan bruke V1-flaten uendret.
Kjernen holdes plattformnøytral, og det bygges ingen iOS-/Android-kode i MVP eller V1 (CLAUDE.md).

## Regler for typene

- Alt er koder, tall og ID-er (`u64`). Aldri UI-tekst; UI oversetter via i18n.
- Eneste tekst er brukerens egne data: regelnavn, personnavn, mappestier (`ProtectScope::Folder`,
  `RuleCondition::InFolder`, `FolderStat`), MIME-typer og plattformens `source_id`.
- Feil går som `FjellError` (kode + tall). `AckRequired { codes }` bæres uendret og besvares ved å sende
  samme kall på nytt via `*_confirmed` med kodene brukeren har bekreftet.
- Ingen metode sletter filer. Sletting skjer i plattformen etter `begin_purge` → `confirm_purge` →
  `finish_purge` (billetten `PurgeTicket` er ugjennomsiktig og kan ikke lages av plattformen).

## Hva som er speilet

Alle `FjellApi`-metoder fra V1 (36 av 36), som `pub fn` på objektet `Fjell`. Robusthetsflaten (sikkerhetskopi, indeksstyring, diagnose; 13 metoder, `docs/v1/ROBUSTHET-KONTRAKT.md`) er bevisst ikke speilet ennå og står i `NOT_MIRRORED_API`:

| Gruppe | Metoder |
| --- | --- |
| Skanning og kort | `start_scan`, `cancel`, `cards` (kort har nå `detail: ReasonDetail`) |
| Beslutninger | `decide_many`, `decide_many_confirmed`, `set_keeper`, `undo` |
| Purge | `begin_purge`, `confirm_purge`, `confirm_purge_confirmed`, `finish_purge` |
| Oversikt | `home_summary`, `trash_summary`, `dashboard`, `dashboard_full` |
| Beskyttelse | `protect_set`, `unprotect`, `protections` (omfang: `Asset`, `Folder`, `Album`, `Person`) |
| Regler | `rules_list`, `rule_save`, `rule_preview`, `rule_toggle`, `rule_delete`, `rule_restore` |
| Personer | `people_list`, `person_name`, `person_merge`, `person_unmerge`, `person_protect` |
| Ansiktsindeks | `face_index_enabled`, `face_index_set`, `face_index_wipe` |
| Serie og terskler | `series_compare`, `thresholds` |
| Innstillinger | `settings`, `set_settings` (nå med `sensitivity` per forslagstype og `face_index`) |

I tillegg, kun i omslaget: `open`, `wait_scan`, `sources_for`, `ticket_sources`, `core_version`.

Merknader:

- `set_keeper`: feilene (`UnknownAsset`, og `AckRequired` hvis kjernen krever bekreftelse) er speilet som
  koder. Kjernens `FjellApi` har ingen egen `set_keeper_confirmed` ennå; kommer den, må den inn i speilet
  (paritetstesten fanger det).
- `set_settings`: `sensitivity` kan være delvis; forslagstyper som mangler beholder gjeldende verdi.
  Ved lesing er det alltid én oppføring per forslagstype.
- `rule_save` returnerer regel-ID som `u64`; tom regel avvises av kjernen.

## Paritetstest

`core/crates/fjell-ffi/tests/parity.rs` leser trait `FjellApi` fra `fjell-core/src/api/mod.rs` og krever at
hver metode står i `MIRRORED_API` (og finnes som `pub fn` i `lib.rs`) eller i `NOT_MIRRORED_API` med
begrunnelse. Testen feiler også på foreldede og dobbelte oppføringer. Får `FjellApi` en ny V1-metode uten at
speilet oppdateres, blir testen rød. `tests/v1.rs` kjører V1-flaten gjennom omslaget mot ekte kjerne.

## Bevisst ikke speilet

`NOT_MIRRORED_API` er tom: ingen `FjellApi`-metode er utelatt. Disse finnes på `Fjell` i kjernen, men ikke på
traiten, og er derfor ikke en del av speilet (paritetstesten ser dem ikke):

- `set_keeper_rules`, `keeper_rules`, `preview_keeper_rules`, `apply_keeper_rules` (behold-regler).
- `restore_assets`, `relocate_source`, `sync_groups`, `last_report`, `with_store`, `storage_summary(_with)`,
  `largest_files`, `with_analyzers`: desktop- og CLI-spesifikke hjelpere (lagringsoversikt, flytting av
  kilde, direkte tilgang til indeksen). Mobil trenger dem ikke i V1; tas inn når de flyttes til traiten.
- Analysatorer (`EyeAnalyzer`, `FaceAnalyzer`) og modellfiler: kjernen eier dem. Plattformen leverer ikke
  ML-callbacks i V1.

## Hva som gjenstår for mobil (ikke gjort, ikke verifisert)

- Generering av Swift/Kotlin-bindinger (`just bindings`, feature `bindgen`) er ikke kjørt her og sjekkes ikke inn.
  Det er ikke lagt til noe genereringssteg som krever nettverk.
- Plattformimplementasjon av `AssetSourceCallback` (PhotoKit / MediaStore) og plattformens egen sletting.
- Bygg for iOS/Android-mål, signering og pakking; ingenting av dette er forsøkt (beslutning 2026-11-11).
- Ytelse og minne over FFI-grensen på ekte enheter er ikke målt.
- Leseflaten er synkron. `wait_scan` blokkerer og må kalles utenfor UI-tråden; mobil må pakke den i egen tråd/coroutine.
- Ansiktsindeks og personer krever en plattform- eller kjerneleverandør av ansiktsmodell; ikke avklart for mobil.
