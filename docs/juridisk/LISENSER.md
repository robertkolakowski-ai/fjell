# Lisensfortegnelse og prosess

**Utkast for juridisk gjennomgang. Ikke juridisk rådgivning.** Status 2026-11-15.

## Hva som er kontrollert (kjørt her, kun Linux)

| Kontroll | Resultat |
|---|---|
| `python3 tools/licenses/gen_third_party.py --policy` | Grønn: alle 312 komponenter (Rust 291, npm 20, ressurser 1) har lisens i tillatt-listen i `deny.toml` (SPDX `OR`/`AND`/`WITH` tolkes av verktøyet) |
| `python3 tools/licenses/gen_third_party.py` (regenerert) | `THIRD_PARTY.md` var utdatert på utgangspunktet (nummerering og gruppering av lisenstekst-blokker hadde flyttet seg). Regenerert, deterministisk (to kjøringer gir identisk fil). Komponentantallet er uendret (312) |
| `gen_third_party.py --check` etter regenerering | Grønn |
| `python3 -m unittest -q test_gen_third_party` (tools/licenses) | 9 tester grønne |
| `cargo deny check` | **Ikke kjørt** (cargo-deny er ikke installert). Må kjøres i CI/lokalt for både `deny.toml` (kjernen) og `desktop/src-tauri/deny.toml` (skallet) |

## Dekning

- **Rust:** `deny.toml` (rot) gjelder kjernen; `desktop/src-tauri/deny.toml` gjelder skallet og tillater i tillegg `CDLA-Permissive-2.0` (Mozillas CA-data via `webpki-root-certs`, bare i oppdateringsstien) og tillater `tokio` (Tauri). Fortegnelsen genereres for Windows-målet (`x86_64-pc-windows-msvc`) fra `fjell-desktop`, bare normale avhengigheter. Verktøyet sjekker mot **rot-`deny.toml`**; `CDLA-Permissive-2.0` er ikke i den, og er heller ikke i fortegnelsen akkurat nå (ingen komponent bruker den i Windows-treet, bekreftet av `--policy`). Kommer en slik komponent, vil verktøyet feile og eier må avgjøre om rot-lista eller skall-lista skal utvides.
- **npm:** kjøretidsavhengigheter (`@tauri-apps/api`, plugin-dialog/-process/-updater, `svelte` med transitive avhengigheter) fra `desktop/package-lock.json`; utviklingsverktøy leveres ikke og er ikke med. Samme tillatt-liste som cargo.
- **Bundlede ressurser:** skriften Inter Variable 5.2.8 (OFL-1.1) med `desktop/src/assets/fonts/OFL.txt` er med i fortegnelsen. Ikoner i appen er egne inline-SVG (`Icon.svelte`, `fjell.svg`) og appikonene i `desktop/src-tauri/icons` er egne; ingen tredjepartsikoner bundles. Ingen modeller er bundlet.
- **Modeller:** ingen valgt, ingen i installasjonsprogrammet (`docs/v1/MODELLER.md`).

## Sjekkliste for ny avhengighet (cargo eller npm)

1. Er den nødvendig? (CLAUDE.md: få avhengigheter, ingen async-runtime, ingen UI-/CSS-bibliotek, ingen nettverkskode.)
2. Lisens i tillatt-listen (`deny.toml`; skall: `desktop/src-tauri/deny.toml`). Copyleft (GPL/AGPL/LGPL) er ikke tillatt; MPL-2.0 bare som ren avhengighet uten endring av filene.
3. `cargo deny check` for berørt workspace (kjernen og skallet) og `cargo test --locked`.
4. `python3 tools/licenses/gen_third_party.py` og sjekk inn `THIRD_PARTY.md` (aldri for hånd); `--check` må være grønn.
5. Bruker den nettverk, filsystem eller telemetri? Da kreves eierbeslutning og oppdatering av `docs/PERSONVERN.md`, `docs/juridisk/BEHANDLINGSOVERSIKT.md` og `nonet`-testene.
6. Dokumenter formål i PR-en. Er den bundlet ressurs (font, ikon, bilde), legg til i `BUNDLED_ASSETS` i `gen_third_party.py` og i `ASSET_ALLOWED` ved ny lisens, og legg lisenstekst ved ressursen.

## Prosess for å legge inn en modell (jf. `docs/v1/MODELLER.md`)

1. Eier velger modell og kjøremotor (ort eller tract). Hver gjelder egen lisensvurdering, også `cargo deny` og `--locked` for motoren.
2. Verifiser mot primærkilde, med dato: lisens for **vektene**, lisens/vilkår for **treningsdata** (egen vurdering; kan være mer restriktiv enn vektene), opphav og størrelse (≤ 30 MB per fil).
3. Lag `manifest.json` (navn, formål, versjon, BLAKE3, bytes, SPDX-lisens, `source_url` som metadata). Lasteporten avviser lisens utenfor tillatt-listen (Apache-2.0, MIT, BSD-2/3, 0BSD, CC0-1.0), feil størrelse eller feil hash.
4. Legg modellen i installasjonsprogrammet (aldri nedlasting fra appen). Legg den til som bundlet ressurs i `gen_third_party.py` (`BUNDLED_ASSETS`, med lisensfil og kilde) og regenerer `THIRD_PARTY.md`. Lisensteksten må hentes fra kilden og kontrolleres av eier; ikke skriv den fra hukommelsen.
5. Øk algoritmeversjonen når embedding-rommet endres, slik at gamle og nye embeddings ikke blandes.
6. Oppdater `docs/PERSONVERN.md` (hva modellen lagrer), `GDPR-ANSIKTSINDEKS.md` og gjør DPIA-vurderingen på nytt før utgivelse.
7. Ikke-verifiserbart her: Windows-ytelse, ekte ONNX-kjøring.

## Spørsmål til jurist

1. Er attribusjonsformatet (`THIRD_PARTY.md` i repoet) tilstrekkelig for installert app? Skal lisensfortegnelsen også vises i appen eller installasjonsprogrammet?
2. MPL-2.0 (uniffi) og OFL-1.1 (Inter): krever distribusjon av installasjonsprogram noe mer enn lisenstekst?
3. Treningsdata-lisenser for en fremtidig ansiktsmodell.
