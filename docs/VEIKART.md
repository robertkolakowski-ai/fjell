# Veikart

| Fase | Innhold |
|---|---|
| MVP (ca. 3 mnd; anslag 13 uker, se `mvp/MVP-PLAN.md`) | **Windows-PC-app** (Tauri 2; avklart 2026-11-11). Eksakte og visuelle duplikater, uskarpe og skjermbilder, kortmodus (swipe), papirkurv, angre, norsk og engelsk. Rekkefølge M0–M8: `mvp/MVP-PLAN.md` |
| V1 (3–6 mnd) | Serier med beste-valg, lukkede øyne, personer, lagringsdashboard, regelmotor |
| V2 (6–12 mnd) | Culling for fotografer, RAW, sammenligning, iPhone og Android (SwiftUI/PhotoKit, Kotlin/MediaStore; UniFFI), Lightroom-eksport |
| V3 (12+ mnd) | Naturlig språk-søk, hendelsesforslag, familiedeling, valgfri sky-analyse, NAS |

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
