# Fjell – instrukser for Claude Code

## Faste regler
- Sletting går alltid via appintern papirkurv («Nylig slettet», 30 dager, konfigurerbar). Aldri hard sletting uten eksplisitt varsel.
- Alt som slettes kan angres. Hver gruppe og hver swipe.
- Originalfiler endres aldri. Metadata lagres i egen indeks og skrives tilbake bare når brukeren velger det.
- Appen «foreslår», den «dømmer» ikke. Hvert forslag har et begrunnelsesmerke. Ingen automatisk sletting.
- On-device først: ingen bilder forlater enheten i standardmodus. Ingen eksterne CDN-er eller fonter.
- Handlinger < 100 ms (p95), kald oppstart < 1,5 s.
- WCAG AA, VoiceOver/TalkBack, alle gester har knappealternativ.
- Norsk (bokmål, nynorsk) og engelsk fra dag én. Test med `nb` (15–30 % lengre tekst).
- Én fase = én gren og én PR. Ikke rør filer utenfor fasens omfang.

## Spesifikasjon
Se `docs/`. Veikartet i `docs/VEIKART.md` styrer rekkefølgen.
