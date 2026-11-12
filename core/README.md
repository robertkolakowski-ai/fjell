# core

Rust-kjernen (Cargo-workspace, én `Cargo.lock`). Skjelett fra M0: ingen forretningslogikk ennå.
Modulgrafen og lagene står i `../docs/mvp/ARKITEKTUR.md` del 2 (konfig for håndhevelse: `../tools/check-layers/layers.toml`).

| Crate | Innhold |
|---|---|
| `crates/fjell-core` | Modulene `model`, `store`, `hash`, `quality`, `screenshot`, `cluster`, `suggest`, `actions`, `pipeline`, `api` |
| `crates/fjell-ffi` | Tomt omslag rundt `api` (UniFFI kommer i M4) |
| `crates/fjell-cli` | Testsele (skriver versjon foreløpig) |

## Kjøre sjekkene

Fra repo-roten (krever Rust, `just` og Python 3.11+):

```sh
cargo install just --locked   # én gang
just check    # fmt + clippy -D warnings + test + check-layers
just i18n     # nøkkel- og plassholderparitet nb/en
just layers   # bare lagsjekken (inkl. selvtest med bevisste brudd)
```

`just bench` og `just bindings` er stubber (M1+ og M4). `cargo-deny`: `cargo install cargo-deny --locked && cargo deny --manifest-path core/Cargo.toml check`.
