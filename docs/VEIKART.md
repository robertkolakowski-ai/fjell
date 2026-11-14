# Veikart

| Fase | Innhold |
|---|---|
| MVP (ca. 3 mnd; anslag 13 uker, se `mvp/MVP-PLAN.md`) | **Windows-PC-app** (Tauri 2; avklart 2026-11-11). Eksakte og visuelle duplikater, uskarpe og skjermbilder, kortmodus (swipe), papirkurv, angre, norsk og engelsk. Rekkefølge M0–M8: `mvp/MVP-PLAN.md`; M9 «Windows-verifisering og ekte data» følger etter M8 (`mvp/PLAN-M9.md`), og MVP er ferdig først etter M9 |
| V1 (3–6 mnd) | Serier med beste-valg, lukkede øyne, personer, lagringsdashboard, regelmotor |
| V2 (6–12 mnd) | Culling for fotografer, RAW, sammenligning, iPhone og Android (SwiftUI/PhotoKit, Kotlin/MediaStore; UniFFI), Lightroom-eksport |
| V3 (12+ mnd) | Naturlig språk-søk, hendelsesforslag, familiedeling, valgfri sky-analyse, NAS |

## V1-status (kjerne og skall, 2026-11-14)
Detaljer og åpne spørsmål: `v1/V1-PLAN.md`.

| Del | Status |
|---|---|
| Kontrakt (typer, migrering 005, API-metoder) | Ferdig |
| Beskyttelse per bilde, mappe, album og person | Kjerne ferdig og koblet til `Fjell`; krever alltid egen bekreftelse (`decide_many` og `confirm_purge`) |
| Serier med beste valg | Kjerne ferdig og koblet til skanning; rangering på skarphet, øyesignal først når en øyemodell er valgt |
| Forslagsregler (`suggest_rules`) | Kjerne ferdig og koblet oppå keeper-reglene fra MVP (`docs/mvp/REGLER.md`, uendret): evalueres etter hver gruppering og ved endret beskyttelse; foreslår bare; keeper-reglenes beskyttede mapper får aldri forslag |
| Personer og ansiktsindeks | Kjerne ferdig og koblet; indeksen er av som standard; ingen ansiktsmodell valgt ennå |
| Dashboard | Kjerne ferdig og koblet |
| Innstillinger og terskler (sensitivitet per forslagstype) | Lagres og styrer serier, visuelle duplikater og uskarphet; verdiene er ikke kalibrert mot golden-korpus |
| CLI (`protect`, `series`, `dashboard`, `people`, `rules`) | Ferdig, ingen sletting av bilder |
| Tauri-skall (DTO-er, kommandoer, `bridge.ts`) | Ferdig; ingen skjermer (UI-fundamentet M6/M7 finnes ikke ennå) |
| Ende-til-ende-test (`api/fjell_v1_tests.rs`, `fjell-cli/tests/cli_v1.rs`) | Ferdig |
| Gjenstår | Øyemodell og ansiktsmodell (lisens, ≤ 30 MB, ONNX), kalibrering av terskler, V1-skjermer, UniFFI-speil av V1-flaten i `fjell-ffi`, golden-tester per forslagstype i `testdata`, sluttreview |

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
