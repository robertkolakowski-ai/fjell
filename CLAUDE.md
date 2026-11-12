# Fjell – instrukser for Claude Code

## Faste regler
- Sletting går alltid via appintern papirkurv («Nylig slettet», 30 dager, konfigurerbar). Aldri hard sletting uten eksplisitt varsel.
- Alt som slettes kan angres. Hver gruppe og hver kortbeslutning.
- Originalfilers innhold endres aldri. De flyttes bare til appens karantenemappe (angrebar) og forlater den først ved «Tøm» med bekreftelse; «Tøm» går som standard til Windows Papirkurv. Metadata lagres i egen indeks og skrives tilbake bare når brukeren velger det.
- Appen «foreslår», den «dømmer» ikke. Hvert forslag har et begrunnelsesmerke. Ingen automatisk sletting.
- On-device først: ingen bilder forlater enheten i standardmodus. Ingen eksterne CDN-er eller fonter.
- Handlinger < 100 ms (p95), kald oppstart < 1,5 s.
- WCAG AA, Windows Narrator og NVDA, Windows-skalering 100–200 % og høykontrast. Alle hurtigtaster og mus-gester har knappealternativ.
- MVP: norsk bokmål og engelsk. Nynorsk kommer etter MVP (nn er stub med fallback til nb). Test med `nb` (15–30 % lengre tekst).
- MVP er en Windows-PC-app (Windows 10/11; Tauri 2 + Rust-kjerne). iPhone og Android kommer etter MVP (beslutning 2026-11-11); hold kjernen plattformnøytral, men bygg ingen iOS-/Android-kode i MVP. Linux/macOS-bygg er bonus, ikke krav.
- Én fase = én gren og én PR. Ikke rør filer utenfor fasens omfang.

## Spesifikasjon
Se `docs/`. Veikartet i `docs/VEIKART.md` styrer rekkefølgen.
