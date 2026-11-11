# MVP-plan (0–3 måneder)

Status: samlet rekkefølge etter uavhengig review av ARKITEKTUR og de tre planene (gren `plan/mvp`), revidert 2026-11-11 for MVP-plattform Windows-PC (gren `plan/pc`). Bindende: `ARKITEKTUR.md`, deretter `PLAN-KJERNE`, `PLAN-TRYGGHET`, `PLAN-UX-I18N`. Hver milepæl består av faser (K*, UX-*, Trygghet-trinn), og hver fase er én gren og én PR (CLAUDE.md).

## 1. Funn fra review (rettet i filene)

| # | Funn | Rettet |
|---|---|---|
| 1 | Begrunnelseskoder: UX hadde 13 egne koder (`same_*`, `keeper_*`, `sig_*`) som ikke fantes i kjernen | UX 5.4 byttet til kjernens 12 koder |
| 2 | Slette-API: UX (`purge(ids, Confirmed)`, `decide`) mot TRYGGHET (`plan/commit`, `begin/confirm/finish_purge`) | ARKITEKTUR del 2 og UX K1/K8 følger TRYGGHET |
| 3 | Skjema: `fail_count`, `protect_flags`, album, `inverse_of`, `cause` var ikke i ARKITEKTUR | Lagt inn i del 3, migrering 1 (beslutning 19) |
| 4 | `list_changes` kunne ikke melde slettede kilder | `ChangeSet` (beslutning 18) |
| 5 | `AssetMeta` utvidet av tre planer hver for seg | Samlet i K0 |
| 6 | `start_scan(mode)` (UX) mot `ScanConfig` uten mode (KJERNE) | `mode: Quick/Full` lagt til |
| 7 | KJERNE: «ingenting forhåndsvalgt» mot UX: duplikater forhåndsmerket | Avklart 2026-11-11: forhåndsmerkes i UI (del 6); beskyttede aldri forhåndsmerket |
| 8 | `suggest` ignorerte beskyttelse (TRYGGHET 4.2) | Steg 0 i rangering, `protect_flags` i `Member` |
| 9 | Onboarding «Flytt 312» uten `MassAck`/backup-advarsel | Lagt til i UX 3.1 |
| 10 | TRYGGHET 2.2 «Tøm: alle valgt» mot 3.6 «utløpte valgt» | Utløpte valgt |
| 11 | «kan frigjøres» brukt om både forslag og papirkurv; «frigjort» talte også eksterne slettinger | Ring = «kan ryddes»; `freed_total` ekskluderer `cause=external` |
| 12 | K2a-3 (enhetsbilder) krevde plattformadaptere før kjernen var ferdig: syklus | Flyttet til gate i K8; deretter fjernet fra MVP (kryssplattform-gate hørte til Android, utenfor MVP) |
| 13 | Recall-port inkluderte beskåret (kjent vanskelig) | Porten gjelder uten beskåret |
| 14 | `AckRequired`/feilkoder og `ConsequenceCode` manglet i i18n-katalogen | `consequence.*` lagt til |
| 15 | «Angre alle» (UX) mot angrestabel (TRYGGHET) | «Angre siste», gjentakbar |
| 16 | Ingen plan eier repo-skjelett, adaptere (fs/Windows) eller `api`/Tauri-skall | Egne milepæler M0, M4, M5 under |
| 17 | `nn` «tom mal» bryter CLAUDE.md regel 11 | Avklart 2026-11-11: `nn` utsatt til etter MVP (del 6); CLAUDE.md oppdatert |
| 18 | Alle skjermbildematrise per PR (36 kombinasjoner × skjerm × plattform) er for tung | Redusert til delmengde per PR, full i `nightly` |
| 19 | MVP-plattform byttet fra iPhone til Windows-PC (2026-11-11) | M4 uten UniFFI; M5–M8 omdefinert; karantene og «Tøm» på PC (ARKITEKTUR 20–22); a11y mot Narrator/NVDA; CI på `windows-latest` |

CLAUDE.md-sjekk: aldri hard sletting (OK: kun `Confirmed`/`PurgeTicket`, to bekreftelser), originaler uendret (OK: `AssetSource` uten skrivemetoder, sjekksum-test), on-device (OK: ingen telemetri, ingen modell; DoD under sjekker manglende nettverkstillatelse), <100 ms (OK, store batcher fikk eget budsjett), nn er ikke MVP-krav (avklart 2026-11-11, CLAUDE.md oppdatert).

## 2. Milepæler og avhengigheter

| M | Innhold (faser) | Avhenger av | Uker |
|---|---|---|---|
| M0 | Repo-skjelett: workspace, `just`, CI-jobber, `check-layers`, `cargo-deny`, `gen-i18n` skall (UX-0) | - | 1 |
| M1 | Fundament: K0 (model, traits, `MemSource`, korpus, harness), K6 (migrering 1 med triggere), `api`-trait + mock, `fjell-cli` + fs-adapter | M0 | 1–3 |
| M2 | Algoritmer: K1, K2a→K2b, K3, K4 parallelt, deretter K5 | M1 | 3–6 |
| M3 | Trygghet: `actions` (plan, commit, undo, expire, purge-flyt), modelltest, I1–I16, T1–T24 | M1 | 3–6 |
| M4 | Pipeline K7 (quick/full, avstemming), `api`, CLI-integrasjon (ingen UniFFI/bindinger) | M2, M3 | 6–8 |
| M5 | Desktop-adapter (filsystem/Windows i `fjell-fs`: `AssetSource`, miniatyrer, `Quarantine`, `delete`, stasjoner, skyplassholdere) + Tauri-skall (`desktop/`, kommandoer mot `api`, CI `windows-latest`) | M1 (kontrakt); integrasjon mot M4 | 3–9 |
| M6 | UI-fundament: UX-1 (tokens, komponenter, `CoreBridge` via Tauri-kommandoer mot mock) | M0, M1 (Tauri-skall fra M5 for ekte kall) | 3–6 |
| M7 | UI-funksjoner (PC): UX-2 → UX-3 → UX-4, UX-5 | M6, M4 (ekte kjerne), M5 | 7–11 |
| M8 | Herding: K8 (kalibrering, enhetsbilder, ytelsesgater), UX-6, manuell Narrator/NVDA, installasjonsprogram, brukbarhet, beta | M7 | 11–13 |

```
M0 ─► M1 ─┬─► M2 ─┬─► M4 ─┐
          ├─► M3 ─┘       ├─► M7 ─► M8
          ├─► M5 ─────────┤
          └─► M6 ─────────┘
```
Grafen er asyklisk. Kritisk sti: M0 → M1 → M2/M3 → M4 → M7 → M8. M5 og M6 går utenfor kritisk sti, men M5 er størst risiko (karantene, «Tøm», stasjoner, Windows-kodeker og Tauri/WebView2).

## 3. Parallellitet og agentoppsett

Én orkestreringsagent eier denne planen, `ARKITEKTUR.md` og grensene (`model`, traits, `api`-kontrakt). Spesialistagenter får hver én fase, egen gren/worktree og leser bare: `api`, berørt modul, sin plan-del og aksepttabellen. Reviewer-agent leser diff og aksepttabell, ikke hele planen.

| Etter | Parallelt (separate agenter) |
|---|---|
| M1 | K1, K2a, K3, K4 (ren funksjon, ingen delt fil) · `actions` + invarianttester · UX-1 mot mock · desktop-adapter · `gen-i18n` |
| K2a | K2b |
| K1, K2b, K3, K4 | K5 |
| M4 | UI PC, K8-kalibrering |

Sekvensielt (delte filer): `model` og migrering 1 (kun orkestrator/K0/K6), `api`-flaten, `ARKITEKTUR.md`. Endres et designvalg: dokumentet først, så koden.

## 4. Definition of done per milepæl

Felles for alle: `just check` grønn, aksepttabell for fasen avkrysset, ingen filer utenfor fasens omfang, PR med skjermbilder ved UI-endring.

| M | DoD |
|---|---|
| M0 | CI kjører på tom kjerne; `check-layers` feiler på bevisst brudd; `cargo-deny` grønn |
| M1 | Korpus genereres deterministisk med `manifest.json`; migrering fra tom og forrige gir lik dump (K6-1); `api`-mock brukes av CLI; originalsjekksum-test finnes |
| M2 | K1-1…8, K2a-1/2/4/5/6, K2b-1…8, K3-1…7, K4-1…6, K5-1…9 grønne; porter: visuelt dup. ≥ 0,98 / 0,90, uskarphet ≥ 0,90, skjermbilde ≥ 0,95; kjernen ≤ 10 ms per bilde på CI |
| M3 | I1–I16 og T1–T24 grønne; ingen vei til `purged` uten `Confirmed`; `trash`/`undo` p95 < 10 ms på 50 000 bilder |
| M4 | K7-1…12 grønne; eksakte grupper før første miniatyr; CLI-fullflyt (skann, handling, angre, `purge --fs-temp`) uendret original |
| M5 | `AssetSource`-kontrakt oppfylt på Windows (orientering brukt, `is_local` for skyplassholdere, flagg, `Unavailable`≠`Corrupt`, frakoblet rot ≠ `removed`, lange stier, UNC); `Quarantine` (manifest, avstemming, fallback) og `delete(ids, mode)` bare med ticket, T25–T28 grønne på temp-kopi; Tauri-skall kaller `api`; jobb `desktop` grønn på `windows-latest`; 20 `device_thumbs/` levert |
| M6 | Komponent-skjermbilder (nb, pseudo, lys, mørk, skalering 100/200 %, høykontrast); kontrasttest; reducer-tester; `MockBridge` og Tauri-`CoreBridge` har samme grensesnitt |
| M7 | UX-2…5 akseptkriterier; tastatur, mus og knapp gir lik `action_log`; axe 0 brudd; ingen avkutting i pseudo; ingen tekst om «frigjort» før permanent sletting; ingen nettverkskall, og ingen nettverks-, fs- eller shell-tillatelse for frontend (Tauri-capabilities og CSP) |
| M8 | K8: alle porter med Windows-dekoding, `ALGO_VER = 1` frosset; handling/angre p95 < 100 ms, kald oppstart < 1,5 s, 10 000 bilder < 10 min på referansemaskin (Windows); onboarding ≤ 60 s; manuell Narrator- og NVDA-avkrysning, skalering 100–200 % og høykontrast; NSIS-installasjonsprogram bygget (signering per åpent spørsmål); brukbarhetstest (5 deltakere) |

## 5. Omfang og risiko (0–3 måneder)

Anslag (ikke målt): med parallelle agenter og kun Windows-PC ca. **13 uker** (spenn 11–15; var 12 for iPhone). Kjerne, trygghet og CLI (M0–M4) uendret, uke 8. Spart: ingen macOS-runner, ingen enhet, ingen UniFFI/bindinger, ingen bakgrunnsoppdrag-API, web-a11y kan automatiseres med axe. Lagt til: karantene, Papirkurv/permanent og stasjonskanter (frakoblet disk, UNC, skyplassholdere), Windows-kodeker (HEIC/RAW), Tauri/WebView2, to skjermlesere (Narrator, NVDA), installasjonsprogram og signering, omarbeid av UX-planen. Netto +1 uke, mest på M5. Stramt: karantene og «Tøm» med full test, skjermleser i WebView2, ytelsesmåling på referansemaskin. Forslag til kuttliste hvis M7 sliter (krever produkteiers ja, ingen regel i CLAUDE.md brytes): mus-dra i kortmodus (tastatur og knapper beholdes); touch-tilpasning; Windows-varsler om utløp (bare banner); historikklinjen «frigjort siden»; full skjermbildematrise utenfor `nightly`; «Gjør dette til beste» (keeper byttes via `set_keeper` beholdes bare hvis tid). Ikke kuttbart: knappealternativ, angre, bekreftelse før permanent sletting.

| Risiko | Tiltak |
|---|---|
| WebView2 mangler på Windows 10, eller Narrator/NVDA oppfører seg ulikt i WebView2 | Test tidlig i M6 (komponenter) med begge skjermlesere; WebView2-bootstrapper i installasjonsprogram; ARIA-live og fokusstyring i felles komponent |
| Dekoding gir hashdrift (HEIC/RAW via WIC avhenger av installert kodek) | Deterministisk `image`-sti for vanlige formater; måling i K8; manglende kodek gir `Unavailable`, ikke feil; fall tilbake på toleranse |
| Rask skann ≤ 30 s avhenger av I/O (HDD, nettverksstasjon, OneDrive-plassholdere) | Måles tidlig i M5; plassholdere leses aldri; ingen port for nettverksstasjoner |
| Store batcher (312+) over 10 ms | Eget budsjett 100 ms, optimistisk UI |
| Karantene: antivirus, fil i bruk, nettverksstasjon uten skriverett, ekstern disk trekkes ut | Per-id-utfall, fallback til ren indekstilstand, frakoblet rot gir `Unavailable`; avstemming mot manifest (ARKITEKTUR 20–21) |
| Usignert installasjonsprogram gir SmartScreen-advarsel | Avklar signering før beta (åpent spørsmål 11) |

## 6. Åpne spørsmål til produkteier

Avklart 2026-11-11 (produkteier):
- Nynorsk er ikke MVP-krav. MVP = nb + en. `nn` utsettes til etter MVP; nøkkelstruktur og fallback til nb beholdes i designet.
- Duplikater forhåndsmerkes i UI (eksakte og visuelle: alt unntatt keeper). Beskyttede bilder aldri forhåndsmerket.
- **MVP-plattform: Windows-PC** (Windows 10/11; Tauri 2). «PC vs iPhone» er avklart: «PC erstatter iPhone i MVP». iPhone (SwiftUI/PhotoKit/UniFFI) og Android kommer etter MVP (se «Etter MVP» i ARKITEKTUR). Android 10/11-spørsmålet og K2a-3 faller bort. (Samme dag tidligere avklart «kun iPhone» er erstattet.)

Åpne:
5. Lokale varsler om utløpt papirkurv i MVP, eller bare banner på Hjem?
6. Skal video ha eksakt-duplikat-forslag i MVP (KJERNE: ja, UX nevner det ikke)?
7. Terskler: `MassDelete` (50 bilder / 1 GB), plangyldighet 5 min, 30 dagers frist som standard. Godkjenn startverdier.
8. Referansemaskin: hvilken Windows-PC (forslag: Windows 11, 8 kjerner, 16 GB, NVMe-SSD) er referanse, hvem eier den, og kan den være self-hosted runner for `nightly`?
9. Beskåret 5 % utenfor recall-porten: akseptabelt for MVP?
10. Skal rekkefølgen M0–M8 inn i `docs/VEIKART.md` (CLAUDE.md sier den styrer)?
11. Kodesignering: hvilket sertifikat eller signeringstjeneste, hvem eier det, og når må det være på plass (senest før beta)?
12. Installasjonsformat: NSIS (forslag, per bruker uten admin) eller MSIX? WebView2 via bootstrapper (forslag) eller offline-installasjon (+ ca. 130 MB)?
13. Windows-versjoner: støtter vi Windows 10 22H2 (ute av støtte hos Microsoft) i tillegg til Windows 11? Forslag: begge.
14. Skal filbaserte bibliotek støtte skyspeilede mapper (OneDrive, Dropbox)? Berører plassholdere, synk av karantene og sletting, og risiko for å slette i skyen. Forslag: ikke støttet i MVP; advar og hopp over plassholdere.
15. «Tøm» sender til Windows Papirkurv som standard og permanent sletting krever eget valg og ekstra bekreftelse (ARKITEKTUR 22). Godkjenn.
16. CLAUDE.md-regelen «originalfiler endres aldri» er presisert til «innhold endres aldri; filer flyttes til appens karantene og kan angres» (ARKITEKTUR 20). Godkjenn presiseringen, eller velg ren indekstilstand uten flytting (alternativ A).
17. Kortmodus: høyre piltast er «Hopp over» i MVP (album utenfor MVP) og blir «Til album» når Samlinger kommer. Godkjenn.
