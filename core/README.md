# core

Rust-kjernen: all logikk i Fjell, delt mellom appen, kommandolinjen og senere mobil. Cargo-workspace med én `Cargo.lock`, `unsafe_code = "forbid"`.

Beslutninger, modulgraf og datamodell står i [`../docs/mvp/ARKITEKTUR.md`](../docs/mvp/ARKITEKTUR.md). V1-flaten (serier, beskyttelse, personer, forslagsregler, dashboard) er beskrevet i [`../docs/v1/V1-PLAN.md`](../docs/v1/V1-PLAN.md).

## Crates

| Crate | Innhold |
|---|---|
| `crates/fjell-core` | Kjernen. Ingen plattformkode, ingen bildeformat-avhengighet, ingen async-runtime. `api` er eneste offentlige flate. |
| `crates/fjell-fs` | Plattformadapter utenfor lagreglene: `FsSource` (`AssetSource` over mapper, eksterne disker og UNC), karantene med manifest og avstemming, «Tøm» via Papirkurv eller permanent med `PermanentAck`, flere røtter, miniatyrer, skymappe-gjenkjenning. `windows` og WIC ligger bak `cfg(windows)`. |
| `crates/fjell-cli` | Testsele og ytelsesmåler mot ekte kjerne. |
| `crates/fjell-ffi` | UniFFI-omslag rundt `api` for mobilfasen. Brukes ikke av Windows-appen; V1-flaten er ikke speilet ennå. |
| `testdata/` | Golden-korpus og deterministisk generator. Egen crate utenfor workspacet; generert data ligger i `testdata/generated/` (ikke innsjekket). |

## Lag i `fjell-core`

En modul kan bare bruke moduler i strengt lavere lag. Regelen håndheves av `just layers` (konfig: [`../tools/check-layers/layers.toml`](../tools/check-layers/layers.toml)).

```
L4  api
L3  pipeline
L2  cluster  suggest  actions  rules  storage  series  people  suggest_rules  protect  dashboard
L1  store  hash  quality  screenshot
L0  model
```

Endres modulgrafen: oppdater `ARKITEKTUR.md` først, så `layers.toml`, så koden.

Trygg sletting håndheves med typer: `Confirmed` og `PurgeTicket` kan bare lages via `api` etter brukerbekreftelse, og `fjell-core` har ingen skrivende plattformkall. Selve flyttingen og slettingen skjer i `fjell-fs`.

## Kommandoer

Fra repo-roten (krever Rust stable, `just` og Python 3.11+):

```sh
just check    # fmt, clippy -D warnings, test, golden-korpus, lagregler
just layers   # bare lagsjekken, med selvtest mot bevisste brudd
just bench    # ytelsesbudsjett i release: 10 000 bilder, handlinger p95 på 50 000 rader
just bindings # Swift-bindinger fra fjell-ffi til core/target/bindings/swift (sjekkes ikke inn)
cargo deny --manifest-path core/Cargo.toml check   # lisenser og sårbarheter (konfig: ../deny.toml)
```

Mangler `just`, står de underliggende kommandoene i [`../justfile`](../justfile).

## Kommandolinjen

```sh
cargo run --manifest-path core/Cargo.toml -p fjell-cli -- <kommando>
```

| Kommando | Gjør |
|---|---|
| `skann <mappe>`, `liste <mappe>` | Skann og list mot mock |
| `index`, `trash`, `undo`, `purge` `<mappe> --db <fil>` | Full flyt mot ekte kjerne. `purge` sletter bare i en midlertidig kopi (`--fs-temp`); originalfiler endres aldri |
| `protect`, `series`, `dashboard`, `people`, `rules` `<mappe> --db <fil>` | V1-flaten. Sletter ingen bilder |
| `perf` | Ytelsesmåling (det `just bench` kjører) |
| `calibrate --dir <mappe> --labels <csv>` | Kalibrering av terskler mot merkede bilder |

Kjør uten argumenter for full bruksanvisning.

## Verifisering

Kjernen og den portable delen av `fjell-fs` testes på Linux og Windows i CI (`core.yml`). Windows-spesifikk kode (Papirkurv via `IFileOperation`, WIC-dekoding av HEIC og RAW, lange stier, stasjoner) kan bare kjøres på Windows. Hva som faktisk er kjørt der, står i [`../docs/mvp/MVP-PLAN.md`](../docs/mvp/MVP-PLAN.md) del 4b og [`../docs/BETA.md`](../docs/BETA.md).
