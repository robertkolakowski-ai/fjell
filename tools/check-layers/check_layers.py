#!/usr/bin/env python3
"""check-layers: håndhever modulgrafen i fjell-core (ARKITEKTUR del 2 og 5).

Feiler (exit 1) på:
  - `crate::<modul>` / `use crate::{a, b}` mot modul i samme eller høyere lag
  - syklus i modulavhengighetene
  - modulfil som ikke finnes i layers.toml (og lag-modul uten fil)
  - kratnivå (`[crates]` i layers.toml): krate som avhenger av annen intern krate enn tillatt
    (f.eks. `fjell-core` mot `fjell-models`), eller `fjell-models` som bruker annet enn `fjell_core::model`

Kun standardbibliotek. Bruk:
  check_layers.py [--src core/crates/fjell-core/src] [--config tools/check-layers/layers.toml]
"""
import argparse
import re
import sys
import tomllib
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent.parent


def load_layers(config):
    """Returnerer {modul: lagnummer} (L0 = 0)."""
    with open(config, "rb") as f:
        raw = tomllib.load(f)["layers"]
    layers = {}
    for name, mods in raw.items():
        n = int(name.lstrip("Ll"))
        for m in mods:
            if m in layers:
                raise SystemExit(f"{config}: modul {m} står i flere lag")
            layers[m] = n
    return layers


def strip_comments_and_strings(text):
    text = re.sub(r"/\*.*?\*/", "", text, flags=re.S)
    text = re.sub(r"//[^\n]*", "", text)  # også //! og ///
    return re.sub(r'"(?:\\.|[^"\\])*"', '""', text)


def modules_in(src):
    """{modulnavn: [filer]} for src/<m>.rs og src/<m>/**.rs (lib.rs/main.rs hoppes over)."""
    mods = {}
    for p in sorted(src.iterdir()):
        if p.is_file() and p.suffix == ".rs" and p.stem not in ("lib", "main"):
            mods.setdefault(p.stem, []).append(p)
        elif p.is_dir():
            mods.setdefault(p.name, []).extend(sorted(p.rglob("*.rs")))
    return mods


USE_RE = re.compile(r"\buse\s+crate::(\{[^;]*\}|\w+)")
PATH_RE = re.compile(r"\bcrate::(\w+)")


def deps_of(files):
    """(avhengig modul, fil, linje) for hver referanse til crate::<modul>."""
    found = []
    for f in files:
        text = strip_comments_and_strings(f.read_text(encoding="utf-8"))
        for m in PATH_RE.finditer(text):
            found.append((m.group(1), f, text.count("\n", 0, m.start()) + 1))
        for m in USE_RE.finditer(text):  # use crate::{a::x, b}
            if m.group(1).startswith("{"):
                body = re.sub(r"\{[^{}]*\}", "", m.group(1)[1:-1])  # dropp nested {..}
                for item in body.split(","):
                    first = re.match(r"\s*(\w+)", item)
                    if first:
                        found.append((first.group(1), f, text.count("\n", 0, m.start()) + 1))
    return found


def find_cycle(graph):
    state, stack = {}, []

    def visit(n):
        state[n] = 1
        stack.append(n)
        for d in sorted(graph.get(n, ())):
            if state.get(d) == 1:
                return stack[stack.index(d):] + [d]
            if d not in state:
                c = visit(d)
                if c:
                    return c
        stack.pop()
        state[n] = 2

    for n in sorted(graph):
        if n not in state:
            c = visit(n)
            if c:
                return c
    return None


def check(src, config):
    layers = load_layers(config)
    mods = modules_in(src)
    errors = []
    for m in sorted(set(mods) - set(layers)):
        errors.append(f"modul `{m}` finnes i {src} men ikke i {config.name}")
    for m in sorted(set(layers) - set(mods)):
        errors.append(f"modul `{m}` står i {config.name} men mangler i {src}")
    graph = {}
    for m, files in sorted(mods.items()):
        if m not in layers:
            continue
        for dep, f, line in deps_of(files):
            if dep == m:
                continue
            if dep not in layers:
                errors.append(f"{f}:{line}: `crate::{dep}` er ikke en kjent modul")
                continue
            graph.setdefault(m, set()).add(dep)
            if layers[dep] >= layers[m]:
                errors.append(
                    f"{f}:{line}: lagbrudd: `{m}` (L{layers[m]}) bruker `{dep}` (L{layers[dep]})"
                )
    cycle = find_cycle(graph)
    if cycle:
        errors.append("syklus: " + " -> ".join(cycle))
    return errors


CRATE_DEP_RE = re.compile(r"^\s*(fjell-[\w-]+)\s*=", re.M)


def load_crates(config):
    """Returnerer ({krate: tillatte interne avhengigheter}, {krate: tillatte fjell_core-moduler})."""
    with open(config, "rb") as f:
        raw = tomllib.load(f)
    allowed = {k: set(v) for k, v in raw.get("crates", {}).items()}
    core_uses = {k: set(v) for k, v in raw.get("crate_core_modules", {}).items()}
    return allowed, core_uses


def check_crates(crates_dir, config):
    """Kratnivå: interne avhengigheter i Cargo.toml og `fjell_core::<modul>` i kildekoden."""
    allowed, core_uses = load_crates(config)
    errors = []
    if not allowed:
        return errors
    found = {p.name for p in crates_dir.iterdir() if (p / "Cargo.toml").is_file()}
    for c in sorted(found - set(allowed)):
        errors.append(f"krate `{c}` finnes i {crates_dir} men ikke i [crates] i {config.name}")
    for c in sorted(set(allowed) - found):
        errors.append(f"krate `{c}` står i [crates] i {config.name} men mangler i {crates_dir}")
    for c in sorted(found & set(allowed)):
        manifest = crates_dir / c / "Cargo.toml"
        text = manifest.read_text(encoding="utf-8")
        # bare [dependencies]/[dev-dependencies]/[build-dependencies]-seksjoner
        body = "\n".join(
            part.split("\n", 1)[1] if "\n" in part else ""
            for part in re.split(r"(?m)^(?=\[)", text)
            if re.match(r"\[(dev-|build-)?dependencies\]", part)
        )
        for dep in sorted(set(CRATE_DEP_RE.findall(body))):
            if dep not in allowed[c]:
                errors.append(f"{manifest}: kratbrudd: `{c}` avhenger av `{dep}` (tillatt: {sorted(allowed[c]) or 'ingen'})")
        if c in core_uses:
            for f in sorted((crates_dir / c).rglob("*.rs")):
                if "target" in f.parts:
                    continue
                code = strip_comments_and_strings(f.read_text(encoding="utf-8"))
                for m in re.finditer(r"\bfjell_core::(\w+)", code):
                    if m.group(1) not in core_uses[c]:
                        line = code.count("\n", 0, m.start()) + 1
                        errors.append(
                            f"{f}:{line}: kratbrudd: `{c}` bruker `fjell_core::{m.group(1)}` (tillatt: {sorted(core_uses[c])})"
                        )
        # kjernen kjenner ikke krater som avhenger av den: ingen kildereferanse heller
        if c == "fjell-core":
            for other in sorted(set(allowed) - {"fjell-core"}):
                ident = other.replace("-", "_")
                for f in sorted((crates_dir / c / "src").rglob("*.rs")):
                    code = strip_comments_and_strings(f.read_text(encoding="utf-8"))
                    for m in re.finditer(rf"\b{ident}\b", code):
                        line = code.count("\n", 0, m.start()) + 1
                        errors.append(f"{f}:{line}: kratbrudd: `fjell-core` refererer `{ident}`")
    return errors


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--src", type=Path, default=ROOT / "core/crates/fjell-core/src")
    ap.add_argument("--config", type=Path, default=HERE / "layers.toml")
    ap.add_argument("--crates", type=Path, default=ROOT / "core/crates",
                    help="katalog med krater (kratnivåsjekk hoppes over uten [crates] i konfig)")
    a = ap.parse_args(argv)
    errors = check(a.src, a.config)
    if a.crates.is_dir():
        errors += check_crates(a.crates, a.config)
    for e in errors:
        print(f"check-layers: {e}", file=sys.stderr)
    if errors:
        print(f"check-layers: FEIL ({len(errors)})", file=sys.stderr)
        return 1
    print(f"check-layers: OK ({a.src})")
    return 0


if __name__ == "__main__":
    sys.exit(main())
