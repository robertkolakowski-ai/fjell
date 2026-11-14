# desktop

Windows-appen: Tauri 2-skall med Svelte 5-frontend oppå Rust-kjernen i [`../core/`](../core/README.md). Beslutningene bak står i [`../docs/mvp/ARKITEKTUR.md`](../docs/mvp/ARKITEKTUR.md) (11, 17, 20–23), designet i [`../docs/design/V2.md`](../docs/design/V2.md).

## Struktur

| Sti | Innhold |
|---|---|
| `src-tauri/` | Eget Cargo-workspace utenfor `core/`, slik at `just check` ikke krever webkit2gtk eller WebView2. |
| `src-tauri/shell/` | `fjell-shell`: kommandologikk og DTO-er mot `fjell-core` og `fjell-fs`. Ren Rust, testes på alle plattformer. V1-flaten ligger i `v1.rs` og `dto_v1.rs`. |
| `src-tauri/src/` | `fjell-desktop`: tynne `#[tauri::command]`-omslag rundt `fjell-shell`. Kompilerer bare med WebView2 (Windows) eller webkit2gtk. |
| `src-tauri/capabilities/`, `tauri.conf.json` | Rettigheter og CSP. Frontend har ingen filsystem-, shell- eller nettverkstilgang. |
| `src-tauri/installer-hooks.nsh` | NSIS-hooker som avslutter gjenværende Fjell-prosesser før installasjon og avinstallering. |
| `src/bridge/` | `CoreBridge`: eneste vei fra UI til kjernen. `tauri-bridge.ts` bruker Tauri `invoke`, `mock-bridge.ts` brukes i nettleser og e2e. `v1.ts` og `types-v1.ts` dekker V1-kommandoene. |
| `src/screens/` | Velkomst, Hjem, gjennomgang, gruppe, kortmodus, Papirkurv, Innstillinger (med oppdatering), Hjelp. Hver skjerm har tilstand og flyt i egne `.ts`-filer med tester. |
| `src/lib/` | Komponenter, `tokens.css` (eneste kilde for farge, radius, skrift og avstand) og `i18n/`. |
| `e2e/` | Playwright mot vite dev med `MockBridge`. Kjører på alle plattformer. |
| `e2e-real/` | Playwright mot den bygde appen via WebView2 og CDP. Bare Windows. |
| `scripts/` | Kontrastsjekk, fixture-generator for ekte-app-e2e, hjelpeskript for oppdaterings-e2e. |

V1-kommandoene (beskyttelse, forslagsregler, personer, dashboard, serier, terskler) finnes i skallet og i `bridge/`, men har ingen skjermer ennå.

## Regler som gjelder her

- All logikk bor i kjernen. UI viser og sender beslutninger via `CoreBridge`; ingen direkte `invoke` fra skjermer.
- Ingen sletting fra frontend. Papirkurv flytter til karantene med angre; «Tøm» krever bekreftelse og går som standard til Windows Papirkurv.
- All brukertekst kommer fra `../i18n/*.json`. `src/lib/i18n/strings/` er generert og innsjekket: rediger aldri for hånd, kjør `just i18n-gen`.
- Farger bare fra `tokens.css`. Skrift er Inter, bundlet lokalt i `src/assets/fonts/`, med systemfonter som reserve. Ingen CDN, ingen eksterne fonter.
- Eneste nettverkskall er oppdateringssjekken, som går fra Rust (Tauri updater), aldri fra frontend. Se [`../docs/OPPDATERING.md`](../docs/OPPDATERING.md).

## Kommandoer

```sh
npm ci
npm run dev        # frontend med MockBridge på http://localhost:1420 (?gallery, ?mockDelay=0)
npm test           # Vitest
npm run check      # svelte-check
npm run contrast   # WCAG AA for alle fargepar i tokens.css
npx playwright test                    # e2e mot MockBridge; PW_CHROME=<sti> velger nettleser
SHOTS=1 npx playwright test e2e/herding.spec.ts   # skjermbilder til ../docs/screens/
npm run tauri dev                      # appvinduet (krever Windows eller webkit2gtk)
npx tauri build --bundles nsis         # usignert NSIS-installasjonsprogram (Windows)
```

Fra repo-roten kjører `just desktop-check` alt som går uten appvindu: fmt, clippy og test for `fjell-shell`, `npm ci`, kontrast, Vitest, svelte-check og `vite build`. `fjell-desktop` sjekkes bare når webkit2gtk-4.1 finnes eller på Windows.

Lisenser og sårbarheter for skallet: `cargo deny --manifest-path desktop/src-tauri/Cargo.toml --config desktop/src-tauri/deny.toml check`.

## Verifisering

- **Alle plattformer:** `fjell-shell`, Vitest, svelte-check, kontrast og `e2e/` (hurtigtaster, axe, høykontrast, redusert bevegelse, skalering 100/150/200 %, ARIA og at frontend ikke gjør nettverkskall).
- **Bare Windows (CI på `windows-latest`):** kompilering av `fjell-desktop`, NSIS-bygg og røyktest (`desktop.yml`), ekte-app-e2e (`e2e-windows.yml`, se [`../docs/mvp/E2E-WINDOWS.md`](../docs/mvp/E2E-WINDOWS.md)) og oppdaterings-e2e (`update-e2e.yml`, se [`../docs/mvp/OPPDATERING-E2E.md`](../docs/mvp/OPPDATERING-E2E.md)).
- **Bare manuelt:** Narrator og NVDA, se [`../docs/BETA.md`](../docs/BETA.md).

Installasjonsprogrammet er ikke kodesignert, så Windows SmartScreen viser en advarsel.
