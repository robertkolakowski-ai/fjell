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

## 4b. Faktisk status per milepæl (integrasjon M9, 2026-11-12, Linux)

Alt under er målt på Linux (4 vCPU). Windows og Tauri er ikke kjørt her. «Kode» betyr at koden finnes og er testet på Linux; «DoD» betyr at hele DoD i del 4 er oppfylt. Ingenting er committet ennå (arbeidstreet `m8/beslutninger`, M9-integrasjon).

Målt i M9-integrasjonen (alle exit 0): `just check` (231 tester i kjernen, 0 feilet), `just i18n` (278 nøkler, 20 selvtester), `just desktop-check` (fjell-shell 34/34, vitest 181/181, svelte-check, vite build; `fjell-desktop` hoppet over, mangler webkit2gtk), `cargo deny check` for `core/` og for `desktop/src-tauri` (egen `deny.toml`), Playwright i system-Chrome på Linux: 40 bestått, 8 hoppet over (skjermbildefiler uten `SHOTS=1`), 0 feil.

| M | Status | Det som er målt / grønt | Det som mangler mot DoD |
|---|---|---|---|
| M0 | DoD oppfylt | `just check` (fmt, clippy -D warnings, test, testdata, layers) exit 0; `check-layers` feiler på bevisste brudd (selvtest); `cargo deny check` på core: advisories, bans, licenses, sources ok | CI-kjøring ikke sett her (kjøres i GitHub) |
| M1 | DoD oppfylt (Linux) | Korpus og manifest, K6-1, `api`-mock, sjekksumtest for originaler grønne | - |
| M2 | Delvis | K1…K5 grønne i `just check`. Uskarphet (K8-algoritmen i `quality.rs`): eval P 0,956 / R 0,919, kontrollsett P 0,956 / R 0,961, gammelt korpus P 0,968 / R 1,000. Kjerne 0,40 ms/bilde | Hash-port med uskarpe varianter regnet som duplikater: recall 0,891 (< 0,90). Kandidat `max_span` 12 / `tight_dist` 5 gir 0,913 hovedsett / 0,931 holdout, men er IKKE innført: falske sammenslåinger kan ikke måles uten ekte foto, og bredere grupper forhåndsmerker flere bilder. Se `K8-ANALYSE.md`. Kun syntetiske motiver |
| M3 | DoD oppfylt (Linux) | I1–I16, T1–T24 grønne; `decide_many` p95 1,19 ms, `undo` 0,77 ms på 50 000 rader | - |
| M4 | DoD oppfylt (Linux) | K7-1…12 grønne, inkl. ekte `kill -9` (K7-6); `just bench`: 10 000 bilder 4,1 s, skann uten endring 57 ms, inkrementell 153 ms, alle budsjetter holdt | `with_known_ids` er koblet i skallet (kun aktive id-er; test på Linux grønn), men ikke prøvd i ekte Tauri/Windows |
| M5 | Kode, ikke DoD | `fjell-fs` 40 tester grønne på Linux; Windows-koden (IFileOperation-Papirkurv, stasjoner, lange stier, UNC) type-sjekket og clippy-ren mot `x86_64-pc-windows-gnu`; `fjell-shell` 34/34 | Ingen Windows-kjøring: Papirkurv, lange stier, UNC, stasjoner, `tests/windows.rs`, `desktop`-jobb, `fjell-desktop` er ikke kompilert (mangler webkit2gtk). `thumbcache` er koblet inn i `FsSource` (cache under app-data, ignoreres hvis den ligger i biblioteket; testet på Linux, ingen hastighetsmåling). 20 `device_thumbs/` ikke levert |
| M6 | DoD oppfylt (Linux) | Kontrast: 81 par i 3 temaer består WCAG AA; `MockBridge`/`CoreBridge` har samme grensesnitt | - |
| M7 | Delvis | vitest 181/181, svelte-check 0 feil, vite build ok; Playwright axe 0 brudd (lys/mørk × bred/smal, pluss kortmodus-slutt, tom papirkurv, Hjem); flyt 4/4; handling til DOM p95 14,8 ms (mål < 100 ms; MockBridge, ikke Tauri) | Ingen skjermleser; kjernen har ett bibliotek, så «Legg til mappe» erstatter forrige; mappevelger ikke koblet (sti skrives inn); «Pause» er avbryt og start på nytt; ingen miniatyrer i Papirkurv |
| M8 | Delvis | UX-6 automatisert (forced-colors lys/mørk, redusert bevegelse, skalering 100/150/200 % og minstevindu med pseudo-lokale, ARIA, ingen nettverk) grønn i Chrome: 40 bestått, 8 hoppet over (skjermbildefiler, bare med `SHOTS=1`); inkluderer 11 nye tester for filer som ikke støttes (`unsupported.spec.ts`: lang sti/UNC/volum uten Papirkurv blir liggende i karantenen, permanent sletting bare med eget valg og avkrysning; axe 0 brudd lys/mørk × bred/smal). Dokumentene `BETA.md`, `RELEASE.md`, `PERSONVERN.md`, `THIRD_PARTY.md` finnes; `just third-party-check` ok | Se «MVP-restanse». `ALGO_VER` er ikke frosset |

Kjente åpne funn (ikke skjult): `Review.svelte` setter inn `role=status` med ferdig tekst (`test.fail` i `aria.spec.ts`, se `BETA.md` del 8); bunnlinjens regionnavn er lik sidetittelen; `cargo deny` for `desktop/src-tauri` kjører nå grønt med egen `deny.toml` (tokio tillatt; RUSTSEC-2024-0370 `proc-macro-error` ignorert med begrunnelse, kun Linux/gtk-tre); nytt CI-jobb `deny` er ikke kjørt i GitHub. `desktop/src-tauri/Cargo.lock` manglet `windows` og er oppdatert med én linje (ellers feilet `--locked`).

## 4c. MVP-restanse

| # | Punkt | Kategori | Hva som trengs |
|---|---|---|---|
| 1 | Windows-kjøring av `fjell-fs` (Papirkurv via IFileOperation, lange stier, UNC, stasjoner, karantene) og `tests/windows.rs` | Krever Windows-maskin | `desktop`-jobben på `windows-latest`; manuell kontroll av at filen faktisk havner i Papirkurv |
| 2 | `fjell-desktop` (Tauri-lim) er ikke kompilert; `connect-src 'self'` fjernet i `tauri.conf.json`, kan ødelegge HMR i `tauri dev` | Krever Windows-maskin | `devCsp` med `ipc:` og `ws://localhost:1420` er lagt til (produksjons-`csp` urørt); `tauri build`/`tauri dev` er ikke kjørt, `cargo check` for `fjell-desktop` ligger i `nightly` (ikke sett grønn) |
| 3 | NSIS-installasjon, WebView2-bootstrapper, røyktest (stille installasjon, start, avinstaller), norsk NSIS-språk | Krever Windows-maskin | Kjør `desktop.yml`; sjekk om Tauri støtter norsk i NSIS. Anbefalt format: NSIS per bruker + WebView2-bootstrapper (spørsmål 12). Gjort i konfig (ikke kjørt): NSIS-språk Norwegian og English, `minimumWebview2Version` 110.0.1587.40 (gulvverdi, må bekreftes); `desktop.yml` bygger NSIS usignert. Ingen installasjon er sett, og norsk tekst er ikke sett i installasjonsprogrammet |
| 4 | Narrator og NVDA, ekte Windows-skalering og høykontrasttemaer (testet bare emulert i Chrome) | Krever Windows-maskin | Manuell avkrysning i `BETA.md` (alle felt er uutfylt) |
| 5 | Ytelse: «10 000 bilder < 10 min», dekodetid (JPEG/HEIC via WIC), kald oppstart < 1,5 s, RSS-topp, handling p95 i ekte Tauri | Krever Windows-maskin | Referansemaskinen (åpent spørsmål 8); `just bench` på `nightly` |
| 6 | Hash-recall med uskarpe varianter som duplikater er 0,891 (< 0,90); kandidatparametere 12/5 gir 0,913/0,931 på syntetisk korpus, men er ikke innført før ekte kalibrering; uskarphet er kun testet på syntetiske motiver; svakheter: treverk, periodisk vev, fokusfeil på tekst, grensesone σ 0,3–0,5 | Krever ekte bilder | Etikettert korpus av ekte foto (HEIC/JPEG/RAW) og ny kalibrering; deretter fryse `ALGO_VER = 1` |
| 7 | Eldre indekser har utdaterte `blur_score` siden algoritmen endret seg før frysing | Delvis lukket | Reindeksering ved neste fulle skann er låst med test (aktive rader regnes om, papirkurv-rader og beslutninger bevares; hurtigskann reindekserer ikke). `ALGO_VER` er ikke frosset |
| 8 | Brukbarhetstest (5 deltakere), onboarding ≤ 60 s målt på ekte maskin | Krever ekte bilder | Rekruttering og ekte bibliotek |
| 9 | Kodesignering og SmartScreen (åpent spørsmål 11), installasjonsformat (12), Windows 10-støtte (13) | Krever beslutning | Produkteier. Anbefalinger står i del 6 (anbefalt, avventer produkteiers ja) |
| 10 | Bibliotek med flere mapper (kjernen har ett), mappevelger-plugin, ekte pause, miniatyrer i Papirkurv, språkvalg lagret i skallet i stedet for `localStorage` | Krever beslutning | Avklar om dette er MVP eller kuttliste (del 5). Delvis: skallet lagrer språk (`Shell::language`/`set_language`, `shell.json`, testet), men Tauri-kommandoer og frontend-kobling (`bridge`, `i18n/index.ts`) er ikke skrevet; språk ligger fortsatt i `localStorage` |
| 11 | Prisene i `RELEASE.md` er veiledende; åpne spørsmål 5–17 | Krever beslutning | Produkteier |
| 12 | Tauri-skallet må kalle `FsSource::with_known_ids` (T11) og koble `thumbcache` inn i `FsSource` | Ikke verifisert | Koblet og testet på Linux (fjell-fs og fjell-shell, mutasjonssjekket). Windows-test gjenstår |
| 13 | `just bench` og Playwright er ikke koblet på CI; Playwright kjørt bare i system-Chrome på Linux | Ikke verifisert | `nightly.yml` (bench på ubuntu og windows, hele Playwright, cargo check for `fjell-desktop`) er skrevet; ingen av jobbene er kjørt i GitHub. Playwright er målt grønt lokalt i Chrome på Linux |
| 14 | `cargo deny` for `desktop/src-tauri` feiler (se over) | Lukket lokalt | Egen `desktop/src-tauri/deny.toml`; `cargo deny` exit 0 lokalt. CI-jobben `deny` er ikke sett grønn i GitHub; unntaket for `proc-macro-error` bør revurderes ved Tauri-oppdatering |

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
10. Skal rekkefølgen M0–M9 inn i `docs/VEIKART.md` (CLAUDE.md sier den styrer)? Anbefalt ja; ført inn i `VEIKART.md` (planlagt i M9-PR, avventer produkteiers ja).
11. Kodesignering: hvilket sertifikat eller signeringstjeneste, hvem eier det, og når må det være på plass (senest før beta)?
    - **Anbefalt, avventer produkteiers ja:** Azure Trusted Signing eller OV-sertifikat (organisasjonsvalidert). Usignert bare for lukket beta med kjente testpersoner. Forvent SmartScreen-advarsel («Windows beskyttet PC-en din») for nytt og lite kjent program, også med OV; omdømme bygges over tid. Dette er ikke prøvd her: ingen signering er gjort og ingen SmartScreen-oppførsel er observert. Eier og frist må produkteier navngi.
12. Installasjonsformat: NSIS (forslag, per bruker uten admin) eller MSIX? WebView2 via bootstrapper (forslag) eller offline-installasjon (+ ca. 130 MB)?
    - **Anbefalt, avventer produkteiers ja:** NSIS per bruker (uten admin) og WebView2-bootstrapper. Offline-installasjon (+ ca. 130 MB) bare som egen nedlasting hvis testpersoner mangler nett. Ikke verifisert på Windows: bygging og stille installasjon kjøres først i M9 (`desktop.yml`).
13. Windows-versjoner: støtter vi Windows 10 22H2 (ute av støtte hos Microsoft) i tillegg til Windows 11? Forslag: begge.
    - **Anbefalt, avventer produkteiers ja:** støtt Windows 10 22H2 og Windows 11; ingenting eldre. `windows-latest` dekker bare ny Windows, så Windows 10 må testes manuelt på egen maskin eller VM (ikke gjort).
14. Skal filbaserte bibliotek støtte skyspeilede mapper (OneDrive, Dropbox)? Berører plassholdere, synk av karantene og sletting, og risiko for å slette i skyen. Forslag: ikke støttet i MVP; advar og hopp over plassholdere.
    - **Anbefalt, avventer produkteiers ja:** skymapper er ikke støttet i MVP. Plassholdere (`is_local = false`) leses aldri og hoppes over, og brukeren advares med antall hoppede filer. Kode og tester finnes på Linux/typesjekk (se 4b); ekte OneDrive er ikke prøvd.
15. «Tøm» sender til Windows Papirkurv som standard og permanent sletting krever eget valg og ekstra bekreftelse (ARKITEKTUR 22). Godkjenn.
16. CLAUDE.md-regelen «originalfiler endres aldri» er presisert til «innhold endres aldri; filer flyttes til appens karantene og kan angres» (ARKITEKTUR 20). Godkjenn presiseringen, eller velg ren indekstilstand uten flytting (alternativ A).
17. Kortmodus: høyre piltast er «Hopp over» i MVP (album utenfor MVP) og blir «Til album» når Samlinger kommer. Godkjenn.

### Anbefalte beslutninger fra M8-integrasjonen (alle avventer produkteiers ja)

Tallene gjenbrukes fra 4b; ingenting nytt er målt eller verifisert på Windows.

- **Egen deny-konfigurasjon for skallet (4c nr. 14):** `desktop/src-tauri` får egen `deny.toml`. Kjernens bannlysting av `tokio` gjelder ikke Tauri-skallet. Uvedlikeholdt `proc-macro-error` via Tauri føres som navngitt, tidsbegrenset unntak med begrunnelse, ikke som stille ignorering. CI utvides til å kjøre deny på begge. Anbefalt, avventer produkteiers ja; planlagt i M9-PR.
- **Lange stier uten 8.3-navn:** kan ikke flyttes trygt til karantene, og gir utfallet `Unsupported`. Filen blir liggende urørt i karantenen, ikke i indeksen som slettet. UI gir to valg: «Hopp over» (filen forblir der den er) eller «Åpne i Utforsker» for manuell håndtering. Ingen automatisk sletting. Anbefalt, avventer produkteiers ja. Vist med temp-kopi på Linux/typesjekk; ikke prøvd på ekte Windows-filsystem.
- **Linjer som lukkes av M9 (planlagt i M9-PR, integratoren justerer):** 4c nr. 1–3, 5, 12–14 får en CI- eller manuell kjøring; de regnes lukket først når kjøringen er sett grønn. Nr. 4, 6–8 blir åpne til produkteier leverer referansemaskin, ekte bilder og testpersoner (se `PLAN-M9.md`).
