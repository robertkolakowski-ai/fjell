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
| [docs/BETA.md](docs/BETA.md) | Manuell testing: Narrator/NVDA, skalering, høykontrast, installasjon, brukbarhetstest |
| [docs/RELEASE.md](docs/RELEASE.md) | NSIS mot MSIX, WebView2, kodesignering, versjonering |
| [docs/PERSONVERN.md](docs/PERSONVERN.md) | Personvernerklæring: ingen opplasting, ingen telemetri, hva som lagres lokalt |
| [THIRD_PARTY.md](THIRD_PARTY.md) | Lisenser for tredjepartskomponenter (generert) |

## Struktur
- `core/` – indeksering, hashing, embeddings, scoring (delt logikk)
- `desktop/` – MVP: Windows-PC-app (Tauri 2) for filbaserte biblioteker, eksterne disker og nettverksstasjoner
- `app/` – iPhone og Android (etter MVP; tom i MVP)
- `docs/` – spesifikasjon

## Status
MVP under bygging (Windows-PC; beslutning 2026-11-11, iPhone og Android kommer etter MVP). Kjerne (`core/`) og skrivebordsapp (`desktop/`) finnes; M8 (herding) pågår. Hva som er verifisert og ikke, står i [docs/BETA.md](docs/BETA.md) og i PR-beskrivelsene. Windows- og Tauri-delene kan bare kjøres på Windows (CI: `windows-latest`).

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
just i18n            # nb/en-paritet, ICU, hardkodet tekst
just contrast        # WCAG AA-kontrast for tokens (lys, mørk, høykontrast)
just desktop-check   # skallet + npm ci, kontrast, Vitest, svelte-check, vite build
just third-party     # regenerer THIRD_PARTY.md (just third-party-check feiler hvis den er utdatert)

# Ende-til-ende (Playwright mot vite + MockBridge; på Linux: PW_CHROME=/usr/bin/google-chrome)
cd desktop && npx playwright install chromium && npx playwright test
SHOTS=1 npx playwright test e2e/herding.spec.ts   # skjermbilder til docs/screens/m8/

# Installasjonsprogram (Windows): usignert NSIS i desktop/src-tauri/target/release/bundle/nsis/
cd desktop && npx tauri build --bundles nsis
```

Testene dekker blant annet: hurtigtaster, axe (WCAG 2.2 AA, lys/mørk × bred/smal), ytelse (p95 < 100 ms), høykontrast (`forced-colors`), redusert bevegelse, Windows-skalering 100/150/200 % og ARIA/fokus (`desktop/e2e/herding.spec.ts`, `aria.spec.ts`), og at appen **ikke gjør nettverkskall** (`desktop/src/lib/nonet.test.ts`, `desktop/e2e/nonet.spec.ts`). Narrator og NVDA kan bare testes manuelt: se [docs/BETA.md](docs/BETA.md).

## Personvern
Bildene forlater aldri maskinen. Ingen opplasting, ingen telemetri, ingen nettverkskall, ingen eksterne fonter eller CDN. Se [docs/PERSONVERN.md](docs/PERSONVERN.md).
