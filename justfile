# Eneste inngang for bygg og kontroll (docs/mvp/ARKITEKTUR.md del 5). `just` uten argument viser oppskriftene.
set shell := ["bash", "-euo", "pipefail", "-c"]

default:
    @just --list

# fmt + clippy (-D warnings) + test + check-layers
check: fmt clippy test testdata layers

fmt:
    cargo fmt --manifest-path core/Cargo.toml --all --check

clippy:
    cargo clippy --manifest-path core/Cargo.toml --workspace --all-targets --locked -- -D warnings

test:
    cargo test --manifest-path core/Cargo.toml --workspace --locked

# Golden-korpus og generator (egen crate, ikke i workspace)
testdata:
    cargo test --manifest-path core/testdata/Cargo.toml --locked

# Lagregler og syklus i fjell-core, pluss selvtest med bevisste brudd
layers:
    python3 tools/check-layers/check_layers.py
    cd tools/check-layers && python3 -m unittest -q test_check_layers

# i18n: paritet, ICU, kjernekoder, utdatert generert kode, hardkodet tekst i desktop/src, selvtester
i18n:
    python3 tools/gen-i18n/gen_i18n.py --check
    python3 tools/gen-i18n/lint_hardcoded.py
    cd tools/gen-i18n && python3 -m unittest -q test_gen_i18n test_lint_hardcoded

# Generer desktop/src/lib/i18n/strings/*.ts (nb, en, pseudo) fra i18n/*.json. Filene sjekkes inn.
i18n-gen:
    python3 tools/gen-i18n/gen_i18n.py

# Stub: criterion/perf kommer i M1 (docs/mvp/ARKITEKTUR.md del 4)
bench:
    @echo "bench: ikke implementert ennå (M1+)"

# Swift-bindinger fra UniFFI (library mode) til core/target/bindings/swift. Sjekkes ikke inn.
# Krever bare cargo; selve Swift-kompileringen (xcframework) skjer på macOS-runner (CI-jobb `ios`).
bindings:
    cargo build --manifest-path core/Cargo.toml -p fjell-ffi --locked
    cd core && cargo run -p fjell-ffi --features bindgen --bin uniffi-bindgen --locked -- \
        generate --library target/debug/libfjell_ffi.so --language swift --out-dir target/bindings/swift
    @test -s core/target/bindings/swift/fjell_ffi.swift && test -s core/target/bindings/swift/fjell_ffiFFI.h
    @echo "bindings: Swift generert i core/target/bindings/swift"

# WCAG AA-kontrast for alle fargepar i desktop/src/lib/tokens.css (lys, mørk, høykontrast)
contrast:
    cd desktop && npm run contrast

# Desktop-skall (Tauri, eget workspace i desktop/src-tauri; IKKE del av `check`, krever npm og webkit/WebView2 for full bygg)
desktop-check:
    cargo fmt --manifest-path desktop/src-tauri/Cargo.toml -p fjell-shell -p fjell-desktop --check
    cargo clippy --manifest-path desktop/src-tauri/Cargo.toml -p fjell-shell --all-targets --locked -- -D warnings
    cargo test --manifest-path desktop/src-tauri/Cargo.toml -p fjell-shell --locked
    cd desktop && npm ci --no-audit --no-fund && npm run contrast && npm test && npm run check && npm run build
    @if pkg-config --exists webkit2gtk-4.1 || [ "${OS:-}" = "Windows_NT" ]; then cargo clippy --manifest-path desktop/src-tauri/Cargo.toml -p fjell-desktop --locked -- -D warnings; else echo "desktop-check: hopper over fjell-desktop (mangler webkit2gtk-4.1); kjøres på windows-latest"; fi
