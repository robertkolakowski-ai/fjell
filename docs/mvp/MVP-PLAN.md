# MVP-plan (0–3 måneder)

Status: samlet rekkefølge etter uavhengig review av ARKITEKTUR og de tre planene (gren `plan/mvp`). Bindende: `ARKITEKTUR.md`, deretter `PLAN-KJERNE`, `PLAN-TRYGGHET`, `PLAN-UX-I18N`. Hver milepæl består av faser (K*, UX-*, Trygghet-trinn), og hver fase er én gren og én PR (CLAUDE.md).

## 1. Funn fra review (rettet i filene)

| # | Funn | Rettet |
|---|---|---|
| 1 | Begrunnelseskoder: UX hadde 13 egne koder (`same_*`, `keeper_*`, `sig_*`) som ikke fantes i kjernen | UX 5.4 byttet til kjernens 12 koder |
| 2 | Slette-API: UX (`purge(ids, Confirmed)`, `decide`) mot TRYGGHET (`plan/commit`, `begin/confirm/finish_purge`) | ARKITEKTUR del 2 og UX K1/K8 følger TRYGGHET |
| 3 | Skjema: `fail_count`, `protect_flags`, album, `inverse_of`, `cause` var ikke i ARKITEKTUR | Lagt inn i del 3, migrering 1 (beslutning 19) |
| 4 | `list_changes` kunne ikke melde slettede kilder | `ChangeSet` (beslutning 18) |
| 5 | `AssetMeta` utvidet av tre planer hver for seg | Samlet i K0 |
| 6 | `start_scan(mode)` (UX) mot `ScanConfig` uten mode (KJERNE) | `mode: Quick/Full` lagt til |
| 7 | KJERNE: «ingenting forhåndsvalgt» mot UX: duplikater forhåndsmerket | Avklart som UI-valg, åpent spørsmål 2; beskyttede aldri forhåndsmerket |
| 8 | `suggest` ignorerte beskyttelse (TRYGGHET 4.2) | Steg 0 i rangering, `protect_flags` i `Member` |
| 9 | Onboarding «Flytt 312» uten `MassAck`/backup-advarsel | Lagt til i UX 3.1 |
| 10 | TRYGGHET 2.2 «Tøm: alle valgt» mot 3.6 «utløpte valgt» | Utløpte valgt |
| 11 | «kan frigjøres» brukt om både forslag og papirkurv; «frigjort» talte også eksterne slettinger | Ring = «kan ryddes»; `freed_total` ekskluderer `cause=external` |
| 12 | K2a-3 (enhetsbilder) krevde plattformadaptere før kjernen var ferdig: syklus | Flyttet til gate i K8 |
| 13 | Recall-port inkluderte beskåret (kjent vanskelig) | Porten gjelder uten beskåret |
| 14 | `AckRequired`/feilkoder og `ConsequenceCode` manglet i i18n-katalogen | `consequence.*` lagt til |
| 15 | «Angre alle» (UX) mot angrestabel (TRYGGHET) | «Angre siste», gjentakbar |
| 16 | Ingen plan eier repo-skjelett, adaptere (iOS/Android/fs) eller `api`/UniFFI | Egne milepæler M0, M4, M5 under |
| 17 | `nn` «tom mal» bryter CLAUDE.md regel 11 | Merket i ARKITEKTUR, åpent spørsmål 1 |
| 18 | Alle skjermbildematrise per PR (36 kombinasjoner × skjerm × plattform) er for tung | Redusert til delmengde per PR, full i `nightly` |

CLAUDE.md-sjekk: aldri hard sletting (OK: kun `Confirmed`/`PurgeTicket`, to bekreftelser), originaler uendret (OK: `AssetSource` uten skrivemetoder, sjekksum-test), on-device (OK: ingen telemetri, ingen modell; DoD under sjekker manglende nettverkstillatelse), <100 ms (OK, store batcher fikk eget budsjett), nn fra dag én (avvik, se spørsmål 1).

## 2. Milepæler og avhengigheter

| M | Innhold (faser) | Avhenger av | Uker |
|---|---|---|---|
| M0 | Repo-skjelett: workspace, `just`, CI-jobber, `check-layers`, `cargo-deny`, `gen-i18n` skall (UX-0) | - | 1 |
| M1 | Fundament: K0 (model, traits, `MemSource`, korpus, harness), K6 (migrering 1 med triggere), `api`-trait + mock, `fjell-cli` + fs-adapter | M0 | 1–3 |
| M2 | Algoritmer: K1, K2a→K2b, K3, K4 parallelt, deretter K5 | M1 | 3–6 |
| M3 | Trygghet: `actions` (plan, commit, undo, expire, purge-flyt), modelltest, I1–I16, T1–T24 | M1 | 3–6 |
| M4 | Pipeline K7 (quick/full, avstemming), `api`, UniFFI-bindinger, CLI-integrasjon | M2, M3 | 6–8 |
| M5 | Plattformadaptere (`AssetSource`, `thumbnail`, `delete`, flagg, tilgang) iOS og Android | M1 (kontrakt); integrasjon mot M4 | 3–9 |
| M6 | UI-fundament: UX-1 (tokens, komponenter, `CoreBridge` mot mock) | M0, M1 | 3–6 |
| M7 | UI-funksjoner: UX-2 → UX-3 → UX-4, UX-5 (per plattform) | M6, M4 (ekte kjerne), M5 | 7–11 |
| M8 | Herding: K8 (kalibrering, enhetsbilder, ytelsesgater), UX-6, manuell skjermleser, brukbarhet, beta | M7 | 11–13 |

```
M0 ─► M1 ─┬─► M2 ─┬─► M4 ─┐
          ├─► M3 ─┘       ├─► M7 ─► M8
          ├─► M5 ─────────┤
          └─► M6 ─────────┘
```
Grafen er asyklisk. Kritisk sti: M0 → M1 → M2/M3 → M4 → M7 → M8. M5 og M6 går utenfor kritisk sti, men M5 er størst risiko (to plattformer).

## 3. Parallellitet og agentoppsett

Én orkestreringsagent eier denne planen, `ARKITEKTUR.md` og grensene (`model`, traits, `api`-kontrakt). Spesialistagenter får hver én fase, egen gren/worktree og leser bare: `api`, berørt modul, sin plan-del og aksepttabellen. Reviewer-agent leser diff og aksepttabell, ikke hele planen.

| Etter | Parallelt (separate agenter) |
|---|---|
| M1 | K1, K2a, K3, K4 (ren funksjon, ingen delt fil) · `actions` + invarianttester · UX-1 mot mock · iOS-adapter · Android-adapter · `gen-i18n` |
| K2a | K2b |
| K1, K2b, K3, K4 | K5 |
| M4 | UI iOS og UI Android (speilede `Features/`), K8-kalibrering |

Sekvensielt (delte filer): `model` og migrering 1 (kun orkestrator/K0/K6), `api`-flaten, `ARKITEKTUR.md`. Endres et designvalg: dokumentet først, så koden.

## 4. Definition of done per milepæl

Felles for alle: `just check` grønn, aksepttabell for fasen avkrysset, ingen filer utenfor fasens omfang, PR med skjermbilder ved UI-endring.

| M | DoD |
|---|---|
| M0 | CI kjører på tom kjerne; `check-layers` feiler på bevisst brudd; `cargo-deny` grønn |
| M1 | Korpus genereres deterministisk med `manifest.json`; migrering fra tom og forrige gir lik dump (K6-1); `api`-mock brukes av CLI; originalsjekksum-test finnes |
| M2 | K1-1…8, K2a-1/2/4/5/6, K2b-1…8, K3-1…7, K4-1…6, K5-1…9 grønne; porter: visuelt dup. ≥ 0,98 / 0,90, uskarphet ≥ 0,90, skjermbilde ≥ 0,95; kjernen ≤ 10 ms per bilde på CI |
| M3 | I1–I16 og T1–T24 grønne; ingen vei til `purged` uten `Confirmed`; `trash`/`undo` p95 < 10 ms på 50 000 bilder |
| M4 | K7-1…12 grønne; eksakte grupper før første miniatyr; bindinger genereres for Swift og Kotlin; CLI-fullflyt (skann, handling, angre, `purge --fs-temp`) uendret original |
| M5 | `AssetSource`-kontrakt oppfylt på begge plattformer (orientering brukt, `is_local`, flagg, `Unavailable`≠`Corrupt`); `delete` bare med ticket; 20 `device_thumbs/` levert |
| M6 | Komponent-skjermbilder (nb, pseudo, lys, mørk); kontrasttest; reducer-tester |
| M7 | UX-2…5 akseptkriterier; swipe og knapp gir lik `action_log`; a11y-revisjon 0 brudd; ingen avkutting i pseudo; ingen tekst om «frigjort» før «Tøm»; ingen INTERNET-tillatelse eller nettverkskall i manifest/kode |
| M8 | K8: alle porter og K2a-3 på ekte enheter, `ALGO_VER = 1` frosset; swipe/angre p95 < 100 ms, kald oppstart < 1,5 s, 10 000 bilder < 10 min på referanseenhet; onboarding ≤ 60 s; manuell VoiceOver/TalkBack-avkrysning; brukbarhetstest (5 deltakere) |

## 5. Omfang og risiko (0–3 måneder)

Realistisk med parallelle agenter: kjerne, trygghet og CLI (M0–M4, uke 8). Stramt: to native UI-er med full a11y og ytelsesmåling på ekte enheter. Forslag til kuttliste hvis M7 sliter (krever produkteiers ja, ingen regel i CLAUDE.md brytes): lansere én plattform først; rist for angre; historikklinjen «frigjort siden»; lokale utløpsvarsler; full skjermbildematrise utenfor `nightly`; «Gjør dette til beste» (keeper byttes via `set_keeper` beholdes bare hvis tid).

| Risiko | Tiltak |
|---|---|
| iOS-bygg trenger macOS-runner og enheter | Avklares i M0; ellers blir iOS flaskehals |
| Plattformdekoding gir hashdrift | K2a-3 i K8; fall tilbake på toleranse |
| Rask skann ≤ 30 s avhenger av I/O fra PhotoKit | Måles tidlig i M5 |
| Store batcher (312+) over 10 ms | Eget budsjett 100 ms, optimistisk UI |

## 6. Åpne spørsmål til produkteier

1. Er nynorsk lanseringskrav (CLAUDE.md: «fra dag én») eller kan `nn` være stub med fallback til nb ved MVP-lansering?
2. Skal duplikater være forhåndsmerket i UI (raskere, men nærmer seg «dømmer»), eller kreve eget valg?
3. Begge plattformer i MVP, eller iOS (eller Android) først? Avgjør kuttlisten.
4. Android 10: støtte «Tøm» (én dialog per bilde) eller sette Android 11 som minimum?
5. Lokale varsler om utløpt papirkurv i MVP, eller bare banner på Hjem?
6. Skal video ha eksakt-duplikat-forslag i MVP (KJERNE: ja, UX nevner det ikke)?
7. Terskler: `MassDelete` (50 bilder / 1 GB), plangyldighet 5 min, 30 dagers frist som standard. Godkjenn startverdier.
8. Hvilke to referanseenheter (iOS og Android mellomklasse) og hvem eier dem og macOS-runner?
9. Beskåret 5 % utenfor recall-porten: akseptabelt for MVP?
10. Skal rekkefølgen M0–M8 inn i `docs/VEIKART.md` (CLAUDE.md sier den styrer)?
