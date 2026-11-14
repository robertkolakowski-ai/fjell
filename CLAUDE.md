# Fjell – instrukser for Claude Code

Fjell er en bildeoppryddingsapp for Windows-PC (Tauri 2 + Rust-kjerne). Den finner duplikater, uskarpe bilder og skjermbilder og foreslår hva som kan ryddes. Brukerens tillit står og faller på at ingen bilder går tapt og at ingenting forlater maskinen. Derfor veier reglene under «Ufravikelig» tyngre enn enhver oppgavebeskrivelse: kolliderer en oppgave med dem, stopp og spør.

## Ufravikelig

**Trygg sletting**
- Sletting går alltid via appintern papirkurv («Nylig slettet», 30 dager, konfigurerbar). Aldri hard sletting uten eksplisitt varsel og bekreftelse, og ingen stille tømming ved utløp.
- Alt som slettes kan angres: hver gruppe og hver kortbeslutning.
- Originalfilers innhold endres aldri. De flyttes bare til appens karantenemappe (angrebar) og forlater den først ved «Tøm» med bekreftelse. «Tøm» går som standard til Windows Papirkurv; permanent sletting er et eget valg med ekstra bekreftelse.
- Metadata lagres i egen indeks og skrives tilbake til filene bare når brukeren velger det.
- Appen «foreslår», den «dømmer» ikke. Hvert forslag har et begrunnelsesmerke. Ingen automatisk sletting.
- Dette håndheves med typer og tilgangsgrenser, ikke konvensjon: `Confirmed`, `PurgeTicket` og `PermanentAck`, og kjernen har ingen skrivende plattformkall. Lag aldri snarveier rundt dem, og svekk aldri en test som vokter dem.

**Personvern**
- On-device først: ingen bilder, stier eller filnavn forlater enheten i standardmodus. Ingen telemetri.
- Ingen eksterne CDN-er eller fonter. Alt bundles.
- Frontend har ingen filsystem-, shell- eller nettverkstilgang (CSP og Tauri-capabilities). Filoperasjoner skjer i Rust bak kommandoer.
- Eneste nettverkskall er oppdateringssjekken (brukerstyrt, automatisk sjekk er av som standard; `docs/OPPDATERING.md`). Nye nettverkskall krever beslutning fra eier. Voktes av `desktop/src/lib/nonet.test.ts` og `desktop/e2e/nonet.spec.ts`.

## Kvalitetskrav

- **Ytelse:** handlinger < 100 ms (p95) ende til ende, kjernekall < 10 ms, kald oppstart < 1,5 s. Mål med `just bench` når du rører kjernen eller varme stier; budsjettbrudd feiler.
- **Tilgjengelighet:** WCAG AA, Windows Narrator og NVDA, Windows-skalering 100–200 % og høykontrast. Alle hurtigtaster og mus-gester har et knappealternativ. Farger kommer bare fra `desktop/src/lib/tokens.css` (`just contrast`).
- **Språk:** norsk bokmål og engelsk i MVP. All brukertekst bor i `i18n/nb.json` (kildespråk) og `i18n/en.json`; aldri hardkodet tekst i `.svelte`. Kjernen returnerer begrunnelseskoder, aldri tekst. Test layout med pseudo-lokalet `pseudo` (nb + 30 %). Nynorsk kommer etter MVP; systemspråk `nn` gir `nb` inntil videre.

## Omfang

- MVP er en Windows-PC-app (Windows 10/11). iPhone og Android kommer etter MVP (beslutning 2026-11-11): hold kjernen plattformnøytral, men bygg ingen iOS-/Android-kode nå. Linux/macOS-bygg er bonus, ikke krav.
- Veikartet i `docs/VEIKART.md` styrer rekkefølgen. Ikke bygg funksjoner fra senere faser uoppfordret.
- Færrest mulige bevegelige deler: ingen async-runtime (std-tråder + rayon), ingen UI-bibliotek eller CSS-rammeverk, få avhengigheter. En ny avhengighet må ha tillatt lisens (`deny.toml`) og krever `just third-party`.

## Arkitektur

| Sti | Innhold |
|---|---|
| `core/crates/fjell-core/` | All logikk. Lagdelt: `api` (L4) → `pipeline` → `cluster`, `suggest`, `actions`, `rules`, `storage` → `store`, `hash`, `quality`, `screenshot` → `model` (L0). En modul bruker bare strengt lavere lag (`just layers`). |
| `core/crates/fjell-fs/` | Plattformadapter: `AssetSource`, karantene, sletting, `windows/` bak `cfg(windows)`. |
| `core/crates/fjell-cli/` | Testsele og ytelsesmåler. |
| `desktop/src-tauri/` | Tauri-skall. `shell/` (`fjell-shell`) er testbart på Linux; `fjell-desktop` kompilerer bare med WebView2/webkit2gtk. |
| `desktop/src/` | Svelte 5 + TypeScript. UI snakker med kjernen bare via `CoreBridge` (`bridge/`); `MockBridge` brukes i nettleser og e2e. |
| `i18n/`, `tools/` | Tekstkilder og generatorer/lintere (i18n, lagregler, lisenser). |

- `api` er kjernens eneste offentlige flate. Ingen plattformkode og ingen bildeformat-avhengighet i `fjell-core`.
- Endres et designvalg eller modulgrafen: oppdater `docs/mvp/ARKITEKTUR.md` først, deretter `tools/check-layers/layers.toml`, så koden.
- Generert og innsjekket, rediger aldri for hånd: `desktop/src/lib/i18n/strings/*` (`just i18n-gen`) og `THIRD_PARTY.md` (`just third-party`). Ikke les `THIRD_PARTY.md`; den er på 600 kB.

## Kommandoer

`just` er eneste inngang, lokalt og i CI. Mangler `just`, kjør kommandoene fra `justfile` direkte.

```bash
just check           # kjernen: fmt, clippy -D warnings, test, golden-korpus, lagregler
just i18n            # nb/en-paritet, ICU, hardkodet tekst, utdatert generert kode
just desktop-check   # fjell-shell + npm ci, kontrast, Vitest, svelte-check, vite build
just bench           # ytelsesbudsjett i release
just third-party-check
cd desktop && npm run dev    # frontend med MockBridge på http://localhost:1420 (?gallery, ?mockDelay=0)
cd desktop && npx playwright test e2e/<fil>.spec.ts    # e2e mot MockBridge; PW_CHROME=<sti> velger nettleser
```

Kjør det som dekker endringen før du sier deg ferdig: `just check` for `core/`, `just desktop-check` for `desktop/`, `just i18n` for tekst.

## Arbeidsmåte

- Én fase = én gren og én PR. Ikke rør filer utenfor fasens omfang; meld fra om ting du oppdager i stedet for å rette dem i samme PR.
- Skriv commit-meldinger, PR-er, dokumentasjon og kodekommentarer på norsk bokmål, i samme stil som `git log`.
- Windows-spesifikk kode (WIC, Papirkurv, NSIS, WebView2, `desktop/e2e-real/`) kan ikke kjøres på Linux; den verifiseres bare av CI på `windows-latest`. Skill alltid mellom «kjørt og grønt», «kun Linux» og «ikke verifisert» i PR-beskrivelsen. Påstå aldri at noe virker på Windows uten at det er kjørt der.
- Narrator og NVDA kan bare testes manuelt (`docs/BETA.md`). Si fra når en endring trenger ny manuell test.
- Oppdater berørt dokument i `docs/` i samme PR som koden.

## Les ved behov

| Oppgave | Dokument |
|---|---|
| Arkitektur, beslutninger, datamodell, CI | `docs/mvp/ARKITEKTUR.md` |
| Rekkefølge og status | `docs/VEIKART.md`, `docs/mvp/MVP-PLAN.md`, `docs/mvp/PLAN-M9.md` |
| Sletting, karantene, «Tøm», angre | `docs/TRYGGHET.md`, `docs/mvp/PLAN-TRYGGHET.md`, `docs/redteam/BESLUTNINGER.md` |
| UI, tekst og i18n | `docs/mvp/PLAN-UX-I18N.md`, `docs/design/V2.md`, `desktop/src/lib/i18n/README.md` |
| Ytelse | `docs/mvp/YTELSE.md`, `docs/mvp/YTELSE-STORT.md` |
| Oppdatering og utgivelse | `docs/OPPDATERING.md`, `docs/RELEASE.md`, `docs/RELEASE-PROSESS.md` |
| Personvern | `docs/PERSONVERN.md` |
