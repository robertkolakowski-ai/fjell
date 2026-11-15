# V1: Diagnose

Lokal feillogg og «Eksporter diagnose» uten bilder eller stier. Kontrakten står i [ROBUSTHET-KONTRAKT.md](ROBUSTHET-KONTRAKT.md).

Status: skallet (`desktop/src-tauri/shell/src/diag.rs`) og skjermen (`SettingsDiagnostics.svelte`) er bygget og testet på Linux.
Tilkoblingen i Tauri-limet er ikke gjort (delte filer; se «Må kobles av integratøren»). Ingenting er verifisert på Windows.

## Regler (CLAUDE.md og docs/PERSONVERN.md)

- Bare feilkoder, kategorier, versjoner, tidsstempler og tall. Aldri bilder, stier, filnavn, albumnavn, personnavn, regelnavn eller embeddings.
- Ingen opplasting, ingen e-postklient, ingen nye nettverkskall og ingen nye Tauri-capabilities. Brukeren sender filen selv.
- Brukeren ser nøyaktig teksten som lagres før noe lagres.

## Feillogg

- Plassering: `<app-data>/logs/fjell.log`, `fjell.log.1`, `fjell.log.2`. Tak 1 MB per fil, tre filer; den eldste forsvinner (bare loggen).
- Én post per linje: `ts=<unix> cat=<kategori> code=<kode> n=<tall> v=<kjerneversjon>`.
  - Kategorier: `panic`, `core`, `scan`, `index`, `store`, `shell`.
  - `code` må være en kjent feilkode (`ROBUST_ERROR_CODES` pluss skallets egne koder). Alt annet, også en feilmelding som inneholder en sti, blir `other`.
- Redaksjon skjer ved konstruksjon: det finnes ikke noe tekstfelt i en post. Ved lesing forkastes linjer som ikke har nøyaktig formen over (en manipulert loggfil kan ikke føre noe inn i rapporten).
- Panikkhook (`install_panic_hook`): skriver `cat=panic code=panic n=<linjenummer>`. Panikkmeldingen og filstien brukes aldri. Kaller deretter forrige hook.
- Skallet logger med `Shell::diag_log(kategori, rå_kode, n)`; `rå_kode` kan trygt være en vilkårlig feilmelding.

## Rapport («Eksporter diagnose»)

Innhold (`kode=verdi`, én per linje): rapportformat, appversjon, kjerneversjon, OS, arkitektur, OS-versjon, skjermskalering,
kjernens målinger (antall bilder i Nylig slettet, forslag, regler, beskyttelser, kopier; indeksens skjemaversjon når kjernen leverer den),
integritet, indekseringstilstand, ytelsestellere (`perf.<kode>.count|total_ms|max_ms`), antall og de siste 50 loggpostene.

- `redact_report` er siste sikkerhetsnett: bare linjer med nøkkel `[a-z0-9_.]` og verdi `[A-Za-z0-9_.,|+-]` slipper gjennom (ingen skråstrek, kolon, mellomrom eller tegn utenfor ASCII).
- Forhåndsvisningen (`Shell::diag_preview`) og filen (`Shell::diag_export`) er samme tekst (testet byte for byte). Rapporten har ikke tidsstempel for «laget»; det står i filnavnet.
- Lagring: `<app-data>/diagnose/fjell-diagnose-<unix>.txt`. Stien returneres i `DiagSavedDto.saved_path` og vises for brukeren; den står aldri i selve rapporten.
- OS-versjon og skalering kommer fra `diag::set_env(os_version, scale_percent)`. Uten kall står `unknown` (på Linux leses kjerneversjonen fra `/proc`). Det finnes ingen kall som gjetter.

### Mappenavn (valgfritt, samtykke)

`DiagOptions { folder_names: Option<FolderNamesConsent> }`. `FolderNamesConsent::new(checked)` gir `None` uten avkrysning, så standard er alltid AV.
Med samtykke kommer bare siste stidel per rot (`folder.1=...`), uten skilletegn, kolon, kontrolltegn og med høyst 40 tegn; aldri hele stier.
Skjermen har foreløpig ingen bryter: `previewDiagnostics()`/`exportDiagnostics()` har ingen argumenter i den delte broen, og en bryter uten ekte virkning ville villedet.
Teksten på skjermen sier at mappenavn ikke er med. Se «Må kobles av integratøren».

## Skjermen

`SettingsDiagnostics.svelte` (montert i Innstillinger av kontrakten): «Med» og «Ikke med», forhåndsvisning (skrivebeskyttet tekstfelt), «Lagre rapport i diagnosemappen», bekreftelse med sti (hvis skallet oppgir den) og beskjed om at brukeren selv sender filen. Tekst i `diag.*`, `privacy.*`.

## Hvorfor ikke «Åpne mappe» eller lagre-dialog

Begge krever en Tauri-tillatelse (`opener`/`shell` eller `dialog:allow-save`) som ikke er gitt, og som frontend ikke skal ha. Valgt løsning er alternativ (d) i kontrakten: skallet skriver i appens egen diagnosemappe og stien vises som tekst.
Alternativ ved behov: en Rust-kommando som viser mappen i Utforsker. Det krever beslutning fra eier om tillatelse.

## Må kobles av integratøren (delte filer)

1. `shell/src/lib.rs`: `pub mod diag;` (nå `mod diag;`; modulen har `#![allow(dead_code)]` til dette er gjort), og kall `diag::install_panic_hook(&data_dir)` og `diag::set_env(..)` ved oppstart i `fjell-desktop`.
2. `dto_robust.rs`/Tauri-kommando `diag_export`: returner `DiagSavedDto` (`report` + `savedPath`), og la `TauriBridge.exportDiagnostics` legge `savedPath` på `DiagReport` (`types-robust.ts`). Frontend leser allerede `savedPath` hvis den finnes.
3. Valgfritt: `previewDiagnostics(opts)`/`exportDiagnostics(opts)` med `includeFolderNames`, koblet til `FolderNamesConsent::new(..)`, og en avkrysning i skjermen.
4. Ytelsestellere: kall `diag::perf_observe("scan"|"card_page"|"decide"|"thumb"|"startup", ms)` der Tauri-limet allerede måler.
5. `docs/PERSONVERN.md` (eies av diag-grenen etter kontrakten) bør få en linje om loggen og diagnosemappen.

## Testet

- Linux, grønt: `diag_tests.rs` (13 enhetstester: kodetillatelsesliste, strenge loggposter, rullering og størrelsestak, manipulert loggfil, `redact_report`, mappenavn, lekkasjetest mot syntetisk bibliotek med æøå og sti over 260 tegn, panikkhook med sti i panikkmeldingen), `shell/tests/diag.rs`, `src/screens/diagFlow.test.ts`, `e2e/diag.spec.ts` (inkl. axe lys/mørk, smal skjerm, pseudo, ingen nettverkskall).
- Ikke verifisert: Windows (OS-versjon og skalering via `set_env` er ikke kodet i Tauri-limet), Narrator og NVDA på den nye teksten og tekstfeltet, 200 % skalering og høykontrast på skjermen, `fjell-desktop`.
