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

# Nøkkelparitet og plassholderparitet nb/en (generering til xcstrings: TODO UX-0)
i18n:
    python3 tools/gen-i18n/gen_i18n.py --check
    cd tools/gen-i18n && python3 -m unittest -q test_gen_i18n

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
