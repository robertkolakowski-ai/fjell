# V1: Indeksering (kontroll og innstillinger)

**Status: bygget og kjørt på Linux. Windows-delene (strømstatus, trådprioritet) er skrevet men ikke verifisert på Windows.**
Kontrakten står i [ROBUSTHET-KONTRAKT.md](ROBUSTHET-KONTRAKT.md); dette dokumentet beskriver det som er bygget i grenen `indexing`.

## Hva brukeren får

- Bakgrunnsindeksering som kan **settes på pause, fortsettes og avbrytes** med knapper. Alt som er analysert er lagret; en ny kjøring gjør bare resten.
- **Rolig fremdrift:** tilstand, fremdriftslinje, «x av y», «z igjen» og et grovt tidsanslag (10 sekunder under 10 minutter, ellers hele minutter). Ingen skremmende tekst.
- **Gjennomgang er aldri blokkert.** Raden sier at forslagene oppdateres underveis og at det som er funnet kan gås gjennom nå.
- **Strupet bruk:** antall tråder er konfigurerbart (0 = automatisk), og prioriteten er lav som standard.
- **Pause på batteri** (standard på). På PC uten batteri, eller når strømstatus er ukjent, pauses det aldri.
- **Frakoblet disk** gir tilstanden «venter på at disken kobles til igjen». Ingenting regnes som slettet.

## Tilstander (koder)

| Kode | Betyr | Hvem setter den |
| --- | --- | --- |
| `idle` | Ingen kjøring | `ScanControl::state` |
| `running` | Analyserer | |
| `paused` | Brukeren har satt på pause | `indexing_pause` |
| `paused_on_battery` | Venter på strøm | `pause_on_battery` + `PowerProbe::on_battery` |
| `disk_offline` | Alle mapper/disker er frakoblet (bare i ventemodus) | `scan_controlled` (`list_changes` gir `Unavailable`) |

`disk_offline` finnes i kjernen (`ControlState::DiskOffline`) og i frontend (`stateKey`, tekster `indexing.state.disk_offline`). Den delte modellen (`IndexState`) har ingen slik kode ennå, så `indexing_status` svarer `paused` for den til en kontraktrunde legger til `IndexState::Offline` (se «Til integratøren»).

## Hvordan det henger sammen

```
UI (IndexingStatus.svelte, Settings)        <- CoreBridge: get/pause/resume/cancel/settings
  Shell (indexing.rs)                       <- koble strøm og trådprioritet (PowerAdapter, wire_platform)
    Fjell (api/fjell_backup.rs)             <- indexing_*, innstillinger i `setting`
      pipeline::control::ScanControl        <- pause (Condvar), tråder, prioritet, batteri, tempo/ETA
      pipeline::scan_controlled             <- som scan, men styrt og tåler frakoblet disk
      pipeline::GuardedSource               <- tidsavbrudd mot hengende kilder
```

- **Pause midt i batch.** `Ctx::stop()` (kalles mellom bilder av alle arbeidstråder) går via `ScanControl::checkpoint`, som parkerer tråden mens skanningen står på pause. Bilder som er i gang blir ferdige; ingen nye startes. Skrivetråden i full fase skriver det den har mottatt etter 400 ms stillstand, så en pause aldri holder ferdig arbeid igjen i minnet. I rask fase (BLAKE3) skrives resultatet per bit på 128 filer.
- **Gjenopptak uten dobbeltarbeid.** Arbeidssettet utledes fra databasen (`work_set`), så et avbrutt eller avsluttet løp starter der det slapp. Testet: pause + avbrudd + ny kjøring gir nøyaktig `N` miniatyrhentinger totalt.
- **Avbrudd** setter `cancel` og vekker parkerte tråder (de sjekker avbruddsflagget hver 100 ms). Det som er funnet beholdes.
- **Prioritet og tråder.** `threads = 0` gir halvparten av kjernene (1–8); ellers det brukeren valgte (høyst 64). Poolen bygges av `ScanControl::build_pool`; ved lav prioritet kjører skallets krok først på hver tråd (Windows: `SetThreadPriority(THREAD_MODE_BACKGROUND_BEGIN)`, som senker CPU-, disk- og minneprioritet for akkurat den tråden). Endret trådantall gjelder fra neste kjøring; batteri og pause gjelder med en gang.
- **Batteri.** Kjernen kjenner ingen plattform: skallet gir en `PowerProbe` som svarer ja/nei. Strømkilden spørres høyst annenhver halve sekund.
- **«Fortsett» på batteri** overstyrer batteripausen for resten av kjøringen, eller til strømmen har vært tilbake (da pauser neste batteriperiode igjen). Brukerens uttrykkelige valg veier tyngre enn den automatiske pausen, og knappen er aldri en blindvei. Dette svarer åpent spørsmål 6 i kontrakten foreløpig; eier bekrefter.
- **Frakoblet rot (beslutning 21).** Er alle røtter ute, gir `list_changes` `Unavailable`, og avstemmingen kjøres *ikke* (ingen bilder regnes som fjernet). Standard: skanningen avslutter med feilkoden, som før (`tests_core::production_source_treats_empty_mount_as_offline_not_removed` krever at en skanning avsluttes). Med `Fjell::set_offline_wait(Some(intervall))` markeres i stedet `disk_offline`, og det prøves igjen til disken er tilbake eller brukeren avbryter. Ventemodus er **av som standard** og må slås på av integratøren sammen med en Hjem som kan vise og avbryte ventingen. Er bare noen røtter ute, hoppes de over av kilden som før.
- **Tidsavbrudd og hengende tråder (stor mappe/nettverksstasjon).** `GuardedSource` kjører `thumbnail` og `read_bytes` på en egen tråd med frist (standard 60 s). Når fristen går ut, eller avbrudd settes, svarer omslaget `Unavailable` med en gang: bildet telles som «utilgjengelig» (ikke som feil mot bildet), og skanningen går videre. Tråden som sitter fast kan ikke drepes i std, men den eier bare sitt eget kall og slippes når kilden svarer. Er 8 kall fastlåst samtidig, nektes nye kall (`Unavailable`) til noen er tilbake, så antallet hengende tråder aldri vokser uten grense. Antall tidsavbrudd telles (`ScanControl::timeouts`). `list_changes` har ikke frist (full opplisting kan være lang) men avbrytes ved avbrudd mellom stegene.
- **Innstillinger** lagres i `setting` (`indexing.threads`, `indexing.priority`, `indexing.pause_on_battery`). Ugyldig verdi gir `indexing_invalid_settings` uten å endre noe.

## Brukergrensesnitt

- `IndexingStatus.svelte` har to visninger. Kompakt (Hjem, uten `controls`): bare det skannlinjen ikke dekker (batteri, frakoblet disk, gjenstående tid) og beskjeden om at forslag oppdateres. Full (`controls`, brukt øverst i Innstillinger → Indeksering, og egnet i en bunnlinje): tilstand, fremdriftslinje, tall, gjenstående tid og knappene Pause/Fortsett/Avbryt.
- **Tilgjengelighet:** tilstandsendringer kunngjøres i en `aria-live`-region, én melding per endring (ikke per polling); første måling kunngjør bare stille tilstander. Alt kan gjøres med knapper (ingen skjulte hurtigtaster). Fremdriftslinjen har navn. Statusen hentes bare mens vinduet er synlig.
- **Språk:** alle tekster ligger under `indexing.*` i `i18n/nb.json` og `en.json`; pseudo-lokalet gir ingen horisontal rulling.

## Ytelse

- Kjernetest `ui_actions_stay_under_100ms_p95_while_indexing_a_large_library` (i `api/fjell_backup/indexing_tests.rs`) indekserer et syntetisk bibliotek (standard 1 500 bilder, `FJELL_INDEXING_PERF_N` setter antallet) mens status, Hjem-oppsummering, kortliste, pause, fortsett og innstillinger kalles gjentatte ganger, og krever p95 < 100 ms per handling. Målt i debug-bygg på Linux: status p95 ≈ 7 µs, Hjem ≈ 0,3 ms, kortliste ≈ 0,5 ms, pause/fortsett ≈ 1–30 µs; enkeltmålinger av Hjem opp mot 50 ms under skriving.
- Playwright (`e2e/indexing.spec.ts`) måler at Pause/Fortsett i UI svarer < 100 ms (p95 av 8 målinger) mot MockBridge. Det sier ikke noe om Tauri-IPC på Windows.
- **Ikke gjort:** måling mot et virkelig stort bibliotek (50 000+ bilder) i release med `just bench`. `fjell-cli bench` er utenfor denne grenens omfang. Oppskrift i `docs/mvp/YTELSE-STORT.md` gjelder fortsatt og bør kjøres på integrasjonsgrenen.

## Hva som er verifisert hvor

| Del | Status |
| --- | --- |
| `pipeline::control` (pause midt i batch, gjenopptak, avbrudd, batteri, frakoblet disk, tidsavbrudd) | Kjørt og grønt, kun Linux |
| `Fjell::indexing_*`, innstillinger, responstid under indeksering | Kjørt og grønt, kun Linux |
| Skall (`indexing.rs`, `tests/indexing.rs`) mot ekte kjerne | Kjørt og grønt, kun Linux |
| Frontend: vitest, svelte-check, kontrast, build, Playwright mot MockBridge (axe lys/mørk × bred/smal, pseudo) | Kjørt og grønt, Chromium på Linux |
| `fjell-fs::windows::power` (`GetSystemPowerStatus`, `SetThreadPriority`) | **Ikke verifisert på Windows** (ikke kompilert her; ren tolkning `classify_windows` er testet på Linux) |
| Faktisk strømbytte (lader ut/inn) og at lav prioritet merkes i Ressursmonitor | **Ikke verifisert** (krever bærbar PC) |
| Frakoblet ekstern disk og treg nettverksstasjon i virkeligheten | **Ikke verifisert** (simulert med syntetiske kilder) |
| Narrator og NVDA på fremdriftsraden og tilstandsmeldingene | **Ikke testet.** Ny manuell test trengs (`docs/BETA.md`) |

## Begrensninger og åpne punkter

- Lavt batteri (prosentgrense) er ikke eget: `PowerStatus` har bare `Ac`/`Battery`/`Unknown`, så «på batteri» dekker også lavt batteri. En egen terskel trenger prosent i `PowerStatus` (kontraktrunde).
- Trådprioritet utenfor Windows har ingen virkning (`NoPriority`), jf. åpent spørsmål 7.
- Frakobling midt i en kjøring (etter at opplistingen er ferdig) gir `Unavailable` per bilde (hoppes over uten feiltelling og tas neste kjøring). Skanningen går da til ende i stedet for å vente; ett tidsavbrudd per bilde kan gjøre det tregt til 8 kall er fastlåst, og da nektes nye kall raskt.
- Analysatorsteget (øyne/ansikter) kjøres etter skanningen og venter ved pause før det starter, men har ikke egen pause midt i seg.
- Hjem har sin egen skannlinje med Pause/Fortsett/Avbryt via `Shell::pause_scan` (avbryt-og-start-på-nytt). Den bør kobles til `pauseIndexing`/`resumeIndexing` (se integrasjonsnotatene) så det er én pause.
