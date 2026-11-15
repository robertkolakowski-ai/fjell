# Behandlingsoversikt (art. 30-stil)

**Utkast for juridisk gjennomgang. Ikke juridisk rådgivning.** Status 2026-11-15. Kontrollert mot koden (se «Kontroller»), ikke bare mot dokumenter.

Forutsetning: Fjell er en lokal app. Utvikleren mottar ingen data (ingen server, ingen konto, ingen telemetri). Brukeren er den eneste som har tilgang til dataene på sin PC (pluss andre med tilgang til Windows-kontoen/disken). Om utvikleren er «behandlingsansvarlig» for noe som aldri forlater brukerens maskin er et spørsmål til jurist (se `GDPR-ANSIKTSINDEKS.md`). Oversikten er likevel skrevet i art. 30-stil for å dokumentere hva som finnes.

Felles for alle rader: **tilgang** = bare brukeren; **overføring** = ingen, med ett unntak: oppdateringssjekken.

| Datatype | Formål | Lagringssted | Lagringstid | Sletting | Overføring | Kilde i koden |
|---|---|---|---|---|---|---|
| Originalbilder | Leses ved skann for å finne duplikater m.m. | Brukerens egne mapper | Ikke Fjells data | Innholdet endres aldri av Fjell; filer flyttes bare til karantene | Ingen | `fjell-fs` (karantene), CLAUDE.md «Ufravikelig» |
| Filstier, filnavn, størrelse, endringstid, EXIF-tid | Identifisere filer i indeksen | `index.db` (SQLite, tabell `asset`) | Til brukeren sletter indeksen | Slett `index.db` eller avinstaller | Ingen | `store/migrations/001_init.sql` |
| Hasher (BLAKE3 og 64-bit visuelle), uskarphetsscore, skjermbildesignaler | Finne duplikater, uskarpe bilder og skjermbilder | `index.db` (`fingerprint`) | Som over | Som over | Ingen | `001_init.sql` |
| Miniatyrer / visningsbilder | Vise bilder raskt | `%LOCALAPPDATA%\Fjell\cache\` | Standard maks 1 GB, eldste fjernes først | Slett mappen (ingen tap) | Ingen | `fjell-fs/src/thumbcache.rs` |
| Forslag, grupper, angrelogg (`action_log`) | Vise forslag og kunne angre | `index.db` | Til indeksen slettes | Som over | Ingen | `001_init.sql`, `005_v1.sql` |
| Karantene og `manifest.json` | Flyttede filer (kan angres), opprinnelig sti | `<disk>\.fjell-karantene\<gruppe>\` | 30 dager som standard (konfigurerbar); ingen stille tømming | «Tøm» (Windows Papirkurv som standard; permanent bare med eget valg og ekstra bekreftelse) | Ingen | `fjell-fs/src/quarantine.rs`, `manifest.rs` |
| Innstillinger (språk, frist, sensitivitet, `face_index_enabled`, automatisk oppdateringssjekk) | Drift | `index.db` (`setting`); språk også i `shell.json` og WebView-`localStorage` (`fjell.language`) | Til slettet | Som over | Ingen | `001_init.sql`, `shell/src/config.rs` |
| Albumnavn og beskyttelser (sti, mappe, album, person) | Beskytte mot opprydding | `index.db` (`album`, `protection`) | Som over | Som over. Person-beskyttelse gjøres om til bilde-beskyttelse ved ansikts-wipe | Ingen | `005_v1.sql`, `people.rs` |
| Regler (navn og vilkår, fritekst) og treff | Egne forslagsregler | `index.db` (`rule`, `rule_hit`) | Som over (myk sletting via `deleted_at`) | Som over | Ingen | `005_v1.sql` |
| Øyeanalyse (antall ansikter, antall lukkede, sikkerhet) | Rangere serier | `index.db` (`eye_result`) | Som over | Røres ikke av ansikts-wipe (funn 3) | Ingen | `005_v1.sql` |
| **Ansikts-embeddings** (BLOB, boks, kvalitet, algoritmeversjon, bilde-id) | Gruppere bilder etter person (valgfritt, av som standard) | `index.db` (`face_embedding`) | Til brukeren sletter. «Slå av» stopper bare ny analyse | «Slett ansiktsindeks»: embeddings og personer i én transaksjon, `secure_delete`, WAL tømmes. Kan ikke angres | Ingen | `store/people.rs` (`face_index_wipe`) |
| **Personnavn** (fritekst fra brukeren) | Navngi personer | `index.db` (`person.name`) | Som over | Samme wipe | Ingen | `005_v1.sql`, `people.rs` |
| Loggfiler | Finnes ikke | Ingen loggfil. Skallet skriver `eprintln!` til prosessens stderr ved feil (kan inneholde sti, f.eks. «kunne ikke flytte indeksen»), ikke til fil | Ikke lagret | – | Ingen | `shell/src/config.rs` (ingen `log`/`tracing`) |
| Sikkerhetskopi av indeksen | Gjenoppretting | **Planlagt**, ikke bygget (`docs/v1/BACKUP.md`). En kopi av indeksen vil inneholde embeddings og navn | Ikke bestemt | Ikke bestemt. Wipe må også dekke kopier (funn 1) | Ingen | `ROBUSTHET-KONTRAKT.md` |
| Diagnoserapport | Feilsøking | **Planlagt**, ikke bygget (`docs/v1/DIAGNOSE.md`; `diag_*` gir tom rapport / `not_implemented`). Bare tall og koder, ingen stier, filnavn eller navn; vises før eksport | Ikke lagret av appen | Ikke bestemt (eksportmåte er åpent spørsmål) | Ingen. Appen sender den aldri selv | `ROBUSTHET-KONTRAKT.md` |
| Oppdateringssjekk | Finne ny beta-versjon | Valget (av/på) i innstillinger; standard av | Til slettet | Ikke trykk / la stå av | **GitHub** (`latest.json`): appversjon, OS/arkitektur; GitHub ser IP og tidspunkt. Aldri bilder, stier, filnavn eller ID-er | `tauri.conf.json` (updater), `capabilities/default.json` |
| Installasjon | Hente WebView2 hvis mangler | – | – | – | Installasjonsprogrammet (ikke appen) kontakter Microsoft | `installer-hooks.nsh` |

## Kontroller mot koden (2026-11-15, kun Linux)

- Nettverk: eneste eksterne endepunkt i konfigurasjonen er updater-URL-en (GitHub) i `desktop/src-tauri/tauri.conf.json`, pluss en lenke-konstant i `shell/src/lib.rs`. CSP har `connect-src ipc: http://ipc.localhost` (dev-CSP tillater localhost). Capabilities: `core:default`, `dialog:allow-open`, `updater:default`, `process:allow-restart`; ingen fil-, shell- eller http-plugin.
- Logging/telemetri: ingen `log`/`tracing`-bruk; grep etter skriving (`fs::write`, `OpenOptions`, `File::create`) i ikke-testkode gir bare miniatyrcache, karantenemanifest og README, og `shell.json`.
- **Ikke verifisert:** faktiske stier og oppførsel på Windows (`%LOCALAPPDATA%`, Papirkurv, WebView2-data). Hva WebView2 lagrer i brukerprofilen utover `fjell.language` er ikke inventert.

## Funn til eier

1. Planlagt sikkerhetskopi kopierer `face_embedding` og `person`; ansikts-wipe når ikke kopier. Wipe må også rense kopier, eller kopier må ekskludere ansiktsdata.
2. `set_face_index(enabled)` tar en vanlig `bool`. Samtykket er et UI-krav (dialog), ikke en type slik `Confirmed`/`PurgeTicket` er for sletting. Vurder en typet `FaceConsent` i kjerneflaten (utenfor denne fasen).
3. Wipe fjerner ikke `eye_result`. Lav sensitivitet, men bør være bevisst.
4. «Slå av» sletter ikke eksisterende ansiktsdata (dokumentert for brukeren).
5. Indeksen er ukryptert.
