# i18n (UX-0)

- Kilde: `i18n/nb.json` (kildespråk) og `i18n/en.json`. Format og regler: `docs/mvp/PLAN-UX-I18N.md` del 5.
- **Generert kode i `strings/` er innsjekket** (valgt framfor git-ignorert: bygg, vitest og `svelte-check` trenger ikke Python).
  Ikke rediger for hånd. Regenerer med `just i18n-gen`; `just i18n` (CI) feiler hvis filene er utdatert.
- `strings/keys.ts`: `MessageKey`, `MessageParams` (typede parametre: plural = number, select = string), `SHORT_KEYS`.
  `nb.ts`, `en.ts`, `pseudo.ts`: ferdige meldinger. `pseudo` = nb +30 %, `[ ]`, aksenttegn, ICU urørt.
- `t("nøkkel", { param })` / `{$tr("nøkkel")}` i Svelte (reaktiv ved språkbytte). Feil nøkkel eller parameter gir typefeil.
- Språk: `setLanguage("system" | "nb" | "en" | "pseudo")`. «system» følger `navigator.languages`; valget lagres i localStorage.
  Manglende oversettelse faller nøkkelvis tilbake til nb (klart for nn etter MVP).
- `fmt.bytes/percent/number/date` formaterer med Intl i valgt språk (hardt mellomrom i `1,2 GB`, `82 %`).
- Lint: ingen tekstnoder eller `aria-label`/`title`/`alt`/`placeholder` med litteral tekst i `.svelte`
  (`tools/gen-i18n/lint_hardcoded.py`). Unntak: `<!-- i18n-ignore: begrunnelse -->`.
