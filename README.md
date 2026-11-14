# Fjell

> Arbeidsnavn. En AI-assistert bildeoppryddingsapp.

Fjell gjør tusenvis av ukurerte bilder om til et bibliotek du faktisk vil åpne. Appen finner duplikater, serier og dårlige bilder, foreslår hva som kan slettes, og lar deg godkjenne alt på sekunder. **Den sletter aldri noe uten at du har sett det og kan angre.**

**Kjerneløftet:** «Frigjør 30 GB og 4 000 bilder på 10 minutter, uten å miste ett eneste bilde du bryr deg om.»

## Målgrupper
- **Hverdagsbrukeren:** 20 000+ bilder, full lagring, ingen tid.
- **Familiefotografen:** flere enheter, delt bibliotek, mange nesten like bilder.
- **Hobby- og proffotografen:** culling av 2 000 RAW-filer etter en jobb.

## Dokumentasjon
| Fil | Innhold |
|---|---|
| [docs/PRODUKT.md](docs/PRODUKT.md) | Kjernefunksjoner og skjermer |
| [docs/TRYGGHET.md](docs/TRYGGHET.md) | Trygghetsmodell for sletting |
| [docs/TEKNIKK.md](docs/TEKNIKK.md) | Personvern, arkitektur, ytelse |
| [docs/INSPIRASJON.md](docs/INSPIRASJON.md) | Hva vi låner fra andre apper |
| [docs/VEIKART.md](docs/VEIKART.md) | Faser, forretningsmodell, suksessmål, risikoer |
| [docs/mvp/ARKITEKTUR.md](docs/mvp/ARKITEKTUR.md) | Beslutninger, modulgraf, datamodell, teststrategi, CI |
| [docs/mvp/MVP-PLAN.md](docs/mvp/MVP-PLAN.md) | Milepæler, faktisk status per milepæl, åpne spørsmål |
| [docs/BETA.md](docs/BETA.md) | Manuell testing: Narrator/NVDA, skalering, høykontrast, installasjon, brukbarhetstest |
| [docs/RELEASE.md](docs/RELEASE.md) | NSIS mot MSIX, WebView2, kodesignering, versjonering |
| [docs/RELEASE-PROSESS.md](docs/RELEASE-PROSESS.md) | Slik lages en utgivelse |
| [docs/OPPDATERING.md](docs/OPPDATERING.md) | Innebygd oppdatering fra beta-kanalen, og hva du gjør når den feiler |
| [docs/PERSONVERN.md](docs/PERSONVERN.md) | Personvernerklæring: ingen opplasting, ingen telemetri, hva som lagres lokalt |
| [docs/design/](docs/design/) | Visuelt design (V2: palett, typografi, sidemeny, merke) |
| [docs/produkt/](docs/produkt/) | Nettside, pris og lisens, signering, hjelp og støtte |
| [docs/brukbarhetstest/](docs/brukbarhetstest/) | Manus, oppgaver og skjema for brukbarhetstest |
| [docs/redteam/](docs/redteam/) | Red team-funn og beslutninger |
| [CHANGELOG.md](CHANGELOG.md) | Endringslogg for brukere og testere |
| [CLAUDE.md](CLAUDE.md) | Faste regler og arbeidsmåte for Claude Code |
| [THIRD_PARTY.md](THIRD_PARTY.md) | Lisenser for tredjepartskomponenter (generert) |

## Struktur
- `core/` – Rust-kjernen (delt logikk): `fjell-core` (indeksering, hashing, gruppering, forslag, papirkurv og angre), `fjell-fs` (filsystem- og Windows-adapter, karantene), `fjell-cli` (testsele og ytelsesmåler)
- `desktop/` – MVP: Windows-PC-app (Tauri 2, Svelte 5 + TypeScript) for filbaserte biblioteker, eksterne disker og nettverksstasjoner
- `i18n/` – tekstkilder (`nb.json`, `en.json`)
- `tools/` – generatorer og kontroller (i18n, lagregler, lisenser, versjon)
- `experiments/` – utforskning utenfor produktet (ML-spike for ansikts- og kvalitetsdeteksjon)
- `app/` – iPhone og Android (etter MVP; tom i MVP)
- `docs/` – spesifikasjon

## Status
MVP under bygging (Windows-PC; beslutning 2026-11-11, iPhone og Android kommer etter MVP). Milepælene M0–M8 er slått sammen. Arbeidet nå er M9, «Windows-verifisering og ekte data» ([docs/mvp/PLAN-M9.md](docs/mvp/PLAN-M9.md)); MVP er ferdig først etter M9.

På `main` finnes i tillegg: mappevelger, miniatyrer, flere mapper og formater, regelmotor (foreslå-regler), lagringsoversikt, nytt visuelt design (V2) og innebygd oppdatering fra beta-kanalen.

Det meste er utviklet og målt på Linux. Windows- og Tauri-delene kan bare kjøres på Windows (CI: `windows-latest`). Hva som er verifisert og ikke, står i [docs/mvp/MVP-PLAN.md](docs/mvp/MVP-PLAN.md) del 4b, [docs/BETA.md](docs/BETA.md) og PR-beskrivelsene. Installasjonsprogrammet er ikke kodesignert, så Windows SmartScreen viser en advarsel.

## Prøv beta
Siste bygg fra `main` ligger som NSIS-installasjonsprogram på [beta-utgivelsen](https://github.com/robertkolakowski-ai/fjell/releases/tag/beta) (Windows 10/11, per bruker, uten administrator). Appen kan deretter oppdatere seg selv når du ber om det: se [docs/OPPDATERING.md](docs/OPPDATERING.md).

## Slik kjører du
Forutsetninger: Rust (stable), Node 22, `just` (valgfritt, men eneste inngang i CI), Python 3. For selve appvinduet på Windows: Visual Studio Build Tools (C++) og WebView2 (ligger i Windows 11).

```bash
# Frontend med MockBridge i nettleser (fungerer på alle OS, ingen Tauri):
cd desktop && npm ci && npm run dev          # http://localhost:1420, ?mockDelay=0 for uten ventetid, ?gallery for komponentgalleriet

# Appen som Windows-vindu (Tauri 2, krever Windows eller webkit2gtk):
cd desktop && npm run tauri dev

# Kjernen fra kommandolinjen (testsele og ytelsesmåler):
cargo run --manifest-path core/Cargo.toml -p fjell-cli -- --help
```

## Slik bygger og tester du
```bash
just                 # viser alle oppskrifter
just check           # Rust-kjernen: fmt, clippy -D warnings, test, golden-korpus, lagregler
just i18n            # nb/en-paritet, ICU, hardkodet tekst, utdatert generert kode
just i18n-gen        # regenerer desktop/src/lib/i18n/strings/ fra i18n/*.json
just bench           # ytelsesbudsjett i release (10 000 bilder, handlinger p95)
just contrast        # WCAG AA-kontrast for tokens (lys, mørk, høykontrast)
just desktop-check   # skallet + npm ci, kontrast, Vitest, svelte-check, vite build
just third-party     # regenerer THIRD_PARTY.md (just third-party-check feiler hvis den er utdatert)

# Ende-til-ende (Playwright mot vite + MockBridge; på Linux: PW_CHROME=/usr/bin/google-chrome)
cd desktop && npx playwright install chromium && npx playwright test
SHOTS=1 npx playwright test e2e/herding.spec.ts   # skjermbilder til docs/screens/m8/

# Ekte app på Windows (WebView2 via CDP): desktop/e2e-real/, kjøres av e2e-windows.yml og update-e2e.yml
# (nattlig og for hånd, ikke på PR). Se docs/mvp/E2E-WINDOWS.md og docs/mvp/OPPDATERING-E2E.md.

# Installasjonsprogram (Windows): usignert NSIS i desktop/src-tauri/target/release/bundle/nsis/
cd desktop && npx tauri build --bundles nsis
```

Testene dekker blant annet: hurtigtaster, axe (WCAG 2.2 AA, lys/mørk × bred/smal), ytelse (p95 < 100 ms), høykontrast (`forced-colors`), redusert bevegelse, Windows-skalering 100/150/200 % og ARIA/fokus (`desktop/e2e/herding.spec.ts`, `aria.spec.ts`), og at frontend **ikke gjør nettverkskall** (`desktop/src/lib/nonet.test.ts`, `desktop/e2e/nonet.spec.ts`). Narrator og NVDA kan bare testes manuelt: se [docs/BETA.md](docs/BETA.md).

## Personvern
Bildene forlater aldri maskinen. Ingen opplasting, ingen telemetri, ingen eksterne fonter eller CDN. Eneste nettverkskall er oppdateringssjekken i beta-kanalen: den kjører bare når du ber om det (automatisk sjekk ved oppstart er av som standard) og sender aldri bilder, stier eller filnavn. Se [docs/PERSONVERN.md](docs/PERSONVERN.md).
