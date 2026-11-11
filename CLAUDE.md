# Fjell – instrukser for Claude Code

## Faste regler
- Sletting går alltid via appintern papirkurv («Nylig slettet», 30 dager, konfigurerbar). Aldri hard sletting uten eksplisitt varsel.
- Alt som slettes kan angres. Hver gruppe og hver swipe.
- Originalfiler endres aldri. Metadata lagres i egen indeks og skrives tilbake bare når brukeren velger det.
- Appen «foreslår», den «dømmer» ikke. Hvert forslag har et begrunnelsesmerke. Ingen automatisk sletting.
- On-device først: ingen bilder forlater enheten i standardmodus. Ingen eksterne CDN-er eller fonter.
- Handlinger < 100 ms (p95), kald oppstart < 1,5 s.
- WCAG AA, VoiceOver (og TalkBack når Android kommer), alle gester har knappealternativ.
- MVP: norsk bokmål og engelsk. Nynorsk kommer etter MVP (nn er stub med fallback til nb). Test med `nb` (15–30 % lengre tekst).
- MVP er kun iPhone (iOS 17+). Android kommer etter MVP; hold kjernen plattformnøytral, men bygg ingen Android-kode i MVP.
- Én fase = én gren og én PR. Ikke rør filer utenfor fasens omfang.

## Spesifikasjon
Se `docs/`. Veikartet i `docs/VEIKART.md` styrer rekkefølgen.
