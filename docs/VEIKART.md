# Veikart

| Fase | Innhold |
|---|---|
| MVP (ca. 3 mnd; anslag 13 uker, se `mvp/MVP-PLAN.md`) | **Windows-PC-app** (Tauri 2; avklart 2026-11-11). Eksakte og visuelle duplikater, uskarpe og skjermbilder, kortmodus (swipe), papirkurv, angre, norsk og engelsk. Rekkefølge M0–M8: `mvp/MVP-PLAN.md`; M9 «Windows-verifisering og ekte data» følger etter M8 (`mvp/PLAN-M9.md`), og MVP er ferdig først etter M9 |
| V1 (3–6 mnd) | Serier med beste-valg, lukkede øyne, personer, lagringsdashboard, regelmotor |
| V2 (6–12 mnd; plan: `v2/V2-PLAN.md`) | Culling for fotografer, RAW, sammenligning, iPhone og Android (SwiftUI/PhotoKit, Kotlin/MediaStore; UniFFI), Lightroom-eksport |
| V3 (12+ mnd) | Naturlig språk-søk, hendelsesforslag, familiedeling, valgfri sky-analyse, NAS |

## V1-status (etter V1-fullføring, 2026-11-15)
Detaljer og åpne spørsmål: `v1/V1-PLAN.md`.

| Del | Status |
|---|---|
| Kontrakt (typer, migrering 005, API-metoder) | Ferdig |
| Beskyttelse per bilde, mappe, album og person | Kjerne ferdig og koblet til `Fjell`; krever alltid egen bekreftelse (`decide_many` og `confirm_purge`) |
| Serier med beste valg | Kjerne ferdig og koblet til skanning; rangering på skarphet, øyesignal først når en øyemodell er valgt |
| Forslagsregler (`suggest_rules`) | Kjerne ferdig og koblet oppå keeper-reglene fra MVP (`docs/mvp/REGLER.md`, uendret): evalueres etter hver gruppering og ved endret beskyttelse; foreslår bare; keeper-reglenes beskyttede mapper får aldri forslag |
| Personer og ansiktsindeks | Kjerne ferdig og koblet; indeksen er av som standard; ingen ansiktsmodell valgt ennå |
| Dashboard | Kjerne ferdig og koblet |
| Innstillinger og terskler (sensitivitet per forslagstype) | Lagres og styrer serier, visuelle duplikater og uskarphet. Kalibrert bare mot syntetisk korpus (`v1/KALIBRERING-V1.md`); standardene er uendret (se under) |
| CLI (`protect`, `series`, `dashboard`, `people`, `rules`) | Ferdig, ingen sletting av bilder |
| Tauri-skall (DTO-er, kommandoer, `bridge.ts`) | Ferdig. V1-feilkoder løftes fra `detail` til `code` i `TauriBridge` (`normalizeError`) |
| Skjermer (`desktop/`, `docs/v1/SKJERMER.md`) | Ferdig mot MockBridge og integrert, med konformitetsfixtur mot både kjerne og mock. Dekket av vitest, axe, skalering 100–200 %, høykontrast, pseudo, ytelse og nonet. `listAlbums` mangler (utforming i SKJERMER del 9); til da bør «i album» skjules i regelbyggeren i den ekte appen |
| Ende-til-ende-test (`api/fjell_v1_tests.rs`, `fjell-cli/tests/cli_v1.rs`) | Ferdig |
| Modellramme (`fjell-models`) | Ferdig: manifest, lisens- og størrelsesport, lasting, `Model*Analyzer` bak trekkene. Ingen modell er valgt, og kjøremotor (`ort`/`tract`) er ikke lagt inn; featuren `onnx` gir bare en feilende fabrikk. `Null*`-analysatorene gjelder fortsatt i appen (`v1/MODELLER.md`) |
| Kalibrering og golden-tester per forslagstype | Verktøy og tester ferdig (`core/testdata`, `v1/KALIBRERING-V1.md`), kjørt på syntetisk korpus |
| UniFFI-speil av V1-flaten (`fjell-ffi`) | Ferdig med paritetstest mot `FjellApi` (`v1/FFI.md`). Ingen iOS-/Android-kode (etter MVP) |
| Windows-verifiseringssett for V1 | Skrevet (`v1/WINDOWS-V1.md`, e2e-jobbene `v1-mock` og `v1-shell`), **ikke kjørt på Windows** |
| Robusthet (backup, diagnose, styrt indeksering, personvern) | Kjerne, skall og skjermer integrert 2026-11-15: indeksbackup og trygg modus med «Bygg på nytt» (`v1/BACKUP.md`), diagnoserapport og feillogg (`v1/DIAGNOSE.md`), pause/prioritet/batteri (`v1/INDEKSERING.md`), behandlingsoversikt og GDPR-notat (`juridisk/`). Kjørt og grønt kun på Linux. **Ikke koblet:** `set_env`/`perf_observe` i Tauri-limet, mappenavn-bryter i diagnose, `IndexState::Offline`, Hjem-pause mot ekte pause. Manuelle Windows-punkter: `v1/WINDOWS-V1.md` 7b, `BETA.md` 8b |
| V2 | Plan skrevet: `v2/V2-PLAN.md` (Raw og par, mobil, eksport). Ingen kode |

### Hva som gjenstår før V1 er ferdig
1. **Ekte modell, valgt av Robert**: øyemodell og ansiktsmodell (lisens, ≤ 30 MB, ONNX), kjøremotor bak `onnx` (må passere `cargo deny` og `--locked`), per-modell-omformer, `THIRD_PARTY.md`, ev. økt `ALGO_VER`, og installasjonsløype for modellmappen. Ansiktsindeksen er av som standard og øyesignalet mangler til dette er gjort.
2. **Ekte kalibrering** på Roberts eget testsett (`mvp/KALIBRERING.md`). Anbefalingen om High `blur_scale` 1,3 til 1,1 er ikke tatt inn: den bygger på syntetisk korpus uten ekte harde negativer, og et lite funn (presisjon 0,878 mot 0,901) er ikke nok til å endre en brukersynlig sensitivitet. Standardene (Low 0,7 / Normal 1,0 / High 1,3 for uskarphet) står til ekte data finnes. Øyesikkerhet er plassholder.
3. **Manuell Windows-verifisering** av V1-skjermene (Narrator, NVDA, skalering, høykontrast, ytelse på ekte bibliotek) og kjøring av V1-jobbene på `windows-latest`.
4. Kjernefunn å avgjøre (se `v1/V1-PLAN.md`, avklaringene og «Åpne funn etter integrasjon»): manuelt valgt keeper vs. øyeanalyse, ny gruppering ved endret sensitivitet, mappebeskyttelse på tegnnivå og Windows-skrivemåte, `older_than_days` 0, overføring av beskyttelse ved flytting, `listAlbums`.

## Forretningsmodell
- **Gratis:** skann, eksakte duplikater, manuell kortsortering, månedlig grense
- **Pro:** ubegrenset opprydding, AI-culling, serier, regler, RAW, Lightroom-eksport
- **Familie:** deling av regler og beskyttede personer
- Ingen skremmekampanjer om lagring. Tydelig pris.

## Suksessmål
- Tid til første sletting < 2 min
- < 1 % av slettinger gjenopprettes som «feil»
- > 5 GB frigjort i snitt i første økt
- Handlinger < 100 ms (p95), kald oppstart < 1,5 s
- NPS > 50, 4,7+ i app-butikkene

## Hovedrisikoer
| Risiko | Tiltak |
|---|---|
| Sletting av uerstattelige bilder | Papirkurv, angre, beskyttede bilder, konsekvensvisning |
| AI-feilvurdering | Forklaring, justerbar terskel, aldri automatisk sletting |
| Personvernbekymringer | On-device, åpen personvernerklæring, ingen opplasting som standard |
| Maskinbelastning (og batteri/varme på mobil) | Inkrementell bakgrunnsindeksering med lav prioritet (mobil: lading og Wi-Fi) |
| Windows-slettebegrensninger (nettverksstasjoner uten Papirkurv, skyspeilede mapper, antivirus) | Appintern karantene med angre, Windows Papirkurv som standard for «Tøm», permanent sletting bare etter eksplisitt bekreftelse |
| Distribusjon utenfor butikk (SmartScreen, kodesignering) | Signert installasjonsprogram; åpent spørsmål i `mvp/MVP-PLAN.md` |
