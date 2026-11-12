#!/usr/bin/env python3
"""Genererer THIRD_PARTY.md fra cargo- og npm-lisenser for det som faktisk leveres i Windows-appen.

Cargo: `cargo metadata --filter-platform x86_64-pc-windows-msvc` for desktop/src-tauri, og bare «normale» avhengigheter
fra rotpakken `fjell-desktop` (byggeverktøy og dev-avhengigheter lenkes ikke inn i programmet).
Npm: kjøretidsavhengigheter fra desktop/package-lock.json (+ `svelte`, som ligger som devDependency men bundles i appen).

Bruk:
  python3 tools/licenses/gen_third_party.py            # skriv THIRD_PARTY.md
  python3 tools/licenses/gen_third_party.py --check    # feil hvis fila er utdatert eller en lisens ikke er tillatt
  python3 tools/licenses/gen_third_party.py --policy   # bare lisenssjekk mot deny.toml

cargo kjøres i en kopi av core/ og desktop/src-tauri (uten target/), så Cargo.lock i repoet røres aldri.
Trenger nettverk for `cargo metadata` hvis crate-cachen er tom (bare i verktøyet, aldri i appen).
Ingen avhengigheter utover Python 3 og cargo.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import re
import shutil
import subprocess
import sys
import tempfile
from collections import defaultdict
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
TARGET_TRIPLE = "x86_64-pc-windows-msvc"
ROOT_CRATE = "fjell-desktop"
NPM_RUNTIME_ROOTS = ["@tauri-apps/api", "svelte"]  # svelte er devDependency, men kompileres inn i bundelen
LICENSE_FILE_RE = re.compile(r"^(LICEN[CS]E|COPYING|UNLICENSE|NOTICE|COPYRIGHT)([-_.].*)?$", re.I)
OUT = ROOT / "THIRD_PARTY.md"


# ---------- lisensuttrykk (SPDX) ----------
def spdx_tokens(expr: str) -> list[str]:
    return re.findall(r"\(|\)|[A-Za-z0-9.+\-]+(?:\s+WITH\s+[A-Za-z0-9.\-]+)?", expr.replace("/", " OR "))


def spdx_ok(expr: str, allowed: set[str]) -> bool:
    """AND krever alle, OR krever minst én. Parentes støttes."""
    toks = spdx_tokens(expr)
    pos = 0

    def parse_or() -> bool:
        nonlocal pos
        v = parse_and()
        while pos < len(toks) and toks[pos].upper() == "OR":
            pos += 1
            r = parse_and()
            v = v or r
        return v

    def parse_and() -> bool:
        nonlocal pos
        v = parse_atom()
        while pos < len(toks) and toks[pos].upper() == "AND":
            pos += 1
            r = parse_atom()
            v = v and r
        return v

    def parse_atom() -> bool:
        nonlocal pos
        t = toks[pos]
        pos += 1
        if t == "(":
            v = parse_or()
            pos += 1  # ")"
            return v
        return re.sub(r"\s+", " ", t) in allowed

    return parse_or() if toks else False


def allowed_from_deny(path: Path) -> set[str]:
    txt = path.read_text(encoding="utf-8")
    m = re.search(r"\[licenses\].*?allow\s*=\s*\[(.*?)\]", txt, re.S)
    if not m:
        raise SystemExit(f"fant ikke [licenses].allow i {path}")
    body = re.sub(r"#.*", "", m.group(1))
    return set(re.findall(r'"([^"]+)"', body))


# ---------- cargo ----------
def cargo_metadata() -> dict:
    with tempfile.TemporaryDirectory(prefix="fjell-lic-") as tmp:
        t = Path(tmp)
        ignore = shutil.ignore_patterns("target", "node_modules", "__pycache__")
        shutil.copytree(ROOT / "core", t / "core", ignore=ignore)
        shutil.copytree(ROOT / "desktop" / "src-tauri", t / "desktop" / "src-tauri", ignore=ignore)
        p = subprocess.run(
            ["cargo", "metadata", "--format-version", "1", "--filter-platform", TARGET_TRIPLE],
            cwd=t / "desktop" / "src-tauri", capture_output=True, text=True,
        )
        if p.returncode != 0:
            raise SystemExit("cargo metadata feilet:\n" + p.stderr[-2000:])
        lock_a = (ROOT / "desktop/src-tauri/Cargo.lock").read_text(encoding="utf-8")
        lock_b = (t / "desktop/src-tauri/Cargo.lock").read_text(encoding="utf-8")
        if lock_a != lock_b:
            print("ADVARSEL: desktop/src-tauri/Cargo.lock er utdatert i forhold til Cargo.toml (cargo ville oppdatert den). Lisenslisten er generert fra oppdatert oppløsning.", file=sys.stderr)
        meta = json.loads(p.stdout)
        # Les lisensfiler mens kopien og registry-cachen finnes
        for pkg in meta["packages"]:
            pkg["_texts"] = read_license_files(Path(pkg["manifest_path"]).parent, t)
        return meta


def read_license_files(d: Path, tmp_root: Path) -> list[tuple[str, str]]:
    out = []
    if not d.is_dir():
        return out
    for f in sorted(d.iterdir()):
        if f.is_file() and LICENSE_FILE_RE.match(f.name):
            try:
                out.append((f.name, f.read_text(encoding="utf-8", errors="replace")))
            except OSError:
                pass
    return out


def cargo_components(meta: dict) -> list[dict]:
    by_id = {p["id"]: p for p in meta["packages"]}
    members = set(meta["workspace_members"])
    root = next((p for p in meta["packages"] if p["name"] == ROOT_CRATE and p["id"] in members), None)
    if root is None:
        raise SystemExit(f"fant ikke rotpakken {ROOT_CRATE}")
    nodes = {n["id"]: n for n in meta["resolve"]["nodes"]}
    seen: set[str] = set()
    stack = [root["id"]]
    while stack:
        i = stack.pop()
        if i in seen:
            continue
        seen.add(i)
        for dep in nodes[i]["deps"]:
            if any(k["kind"] is None for k in dep["dep_kinds"]):  # None = normal (lenkes inn)
                stack.append(dep["pkg"])
    comps = []
    for i in seen:
        p = by_id[i]
        if i in members or p.get("source") is None:
            continue  # egne crates (workspace-medlemmer og path-avhengigheter som fjell-core/fjell-fs)
        comps.append({
            "ecosystem": "cargo", "name": p["name"], "version": p["version"],
            "license": (p.get("license") or "").replace("/", " OR ") or ("(se lisensfil)" if p["_texts"] else "UKJENT"),
            "source": p.get("repository") or p.get("homepage") or "",
            "texts": p["_texts"],
        })
    return sorted(comps, key=lambda c: (c["name"].lower(), c["version"]))


# ---------- npm ----------
def npm_components() -> list[dict]:
    lock = json.loads((ROOT / "desktop" / "package-lock.json").read_text(encoding="utf-8"))
    pk = lock["packages"]

    def entry(name: str, frm: str) -> tuple[str, dict] | None:
        # node resolution i flat lock: søk nærmest oppover
        base = frm
        while True:
            cand = f"{base}/node_modules/{name}" if base else f"node_modules/{name}"
            if cand in pk:
                return cand, pk[cand]
            if not base:
                return None
            base = base.rsplit("/node_modules/", 1)[0] if "/node_modules/" in base else ""

    seen: dict[str, dict] = {}
    stack = [(n, "") for n in NPM_RUNTIME_ROOTS]
    while stack:
        name, frm = stack.pop()
        e = entry(name, frm)
        if not e or e[0] in seen:
            continue
        seen[e[0]] = e[1]
        for dep in list((e[1].get("dependencies") or {}).keys()) + list((e[1].get("optionalDependencies") or {}).keys()):
            stack.append((dep, e[0]))
        for dep in (e[1].get("peerDependencies") or {}):
            if not (e[1].get("peerDependenciesMeta") or {}).get(dep, {}).get("optional"):
                stack.append((dep, e[0]))
    comps = []
    for path, meta in seen.items():
        name = path.split("node_modules/")[-1]
        d = ROOT / "desktop" / path
        lic = meta.get("license") or "UKJENT"
        if isinstance(lic, dict):
            lic = lic.get("type", "UKJENT")
        comps.append({
            "ecosystem": "npm", "name": name, "version": meta.get("version", "?"), "license": lic,
            "source": (meta.get("resolved") or "").split("/-/")[0].replace("registry.npmjs.org", "www.npmjs.com/package") if meta.get("resolved") else "",
            "texts": read_license_files(d, ROOT),
        })
    return sorted(comps, key=lambda c: (c["name"].lower(), c["version"]))


# ---------- utdata ----------
def norm(text: str) -> str:
    return re.sub(r"\s+", " ", text).strip().lower()


def render(cargo: list[dict], npm: list[dict]) -> str:
    comps = cargo + npm
    by_lic: dict[str, int] = defaultdict(int)
    for c in comps:
        by_lic[c["license"]] += 1
    L: list[str] = []
    a = L.append
    a("# Tredjeparts lisenser")
    a("")
    a("> Generert av `tools/licenses/gen_third_party.py`. Ikke rediger for hånd; kjør `just third-party` og sjekk inn resultatet.")
    a("")
    a("Fjell er bygget med åpne komponenter. Her står alt som leveres i Windows-appen, med lisens og lisenstekst. "
      "Fjell sender ingenting over nettverket (se [docs/PERSONVERN.md](docs/PERSONVERN.md)); komponentene under er derfor bare kode som kjører lokalt.")
    a("")
    a("Omfang:")
    a(f"- **Rust:** normale (innlenkede) avhengigheter av `{ROOT_CRATE}` for `{TARGET_TRIPLE}`; byggeverktøy og testbiblioteker er ikke med. Egne crates (`fjell-*`) er ikke listet.")
    a("- **npm:** kjøretidsavhengigheter som bundles i frontend (`@tauri-apps/api`, `svelte` og deres avhengigheter). Utviklingsverktøy (vite, vitest, playwright, axe-core, typescript) leveres ikke.")
    a("- **Ikke med:** systemkomponenter fra Windows (WebView2, Segoe UI). Appen bundler ingen fonter, ingen bilder fra tredjepart og ingen modeller. Appikonet er et eget plassholderikon.")
    a("")
    a(f"Antall komponenter: **{len(comps)}** (Rust {len(cargo)}, npm {len(npm)}).")
    a("")
    a("## Lisenser i bruk")
    a("")
    a("| Lisens (SPDX-uttrykk) | Komponenter |")
    a("|---|---:|")
    for lic, n in sorted(by_lic.items(), key=lambda kv: (-kv[1], kv[0])):
        a(f"| {lic} | {n} |")
    a("")
    for title, group in (("Rust-komponenter", cargo), ("npm-komponenter", npm)):
        a(f"## {title}")
        a("")
        a("| Navn | Versjon | Lisens | Kilde |")
        a("|---|---|---|---|")
        for c in group:
            src = c["source"]
            a(f"| {c['name']} | {c['version']} | {c['license']} | {src} |")
        a("")
    # Lisenstekster, deduplisert på normalisert tekst
    groups: dict[str, dict] = {}
    missing = []
    for c in comps:
        if not c["texts"]:
            missing.append(c)
        for fname, text in c["texts"]:
            h = hashlib.sha256(norm(text).encode()).hexdigest()[:12]
            g = groups.setdefault(h, {"text": text.strip("\n"), "users": []})
            g["users"].append(f"{c['name']} {c['version']}")
    a("## Lisenstekster")
    a("")
    a("Like tekster er slått sammen. Hver tekst står én gang, med liste over komponentene den gjelder (opphavsrettslinjene i teksten er komponentens egne).")
    a("")
    for i, (h, g) in enumerate(sorted(groups.items(), key=lambda kv: (-len(kv[1]["users"]), kv[0])), 1):
        users = sorted(set(g["users"]), key=str.lower)
        a(f"### Tekst {i} ({len(users)} komponenter)")
        a("")
        a("Gjelder: " + ", ".join(users))
        a("")
        a("```text")
        a(g["text"].replace("```", "'''"))
        a("```")
        a("")
    if missing:
        a("## Komponenter uten lisensfil i pakken")
        a("")
        a("Disse har kun SPDX-uttrykk i metadata (ingen egen lisensfil i pakken). Lisensteksten er standardteksten for uttrykket i kolonnen «Lisens» over.")
        a("")
        for c in missing:
            a(f"- {c['name']} {c['version']} ({c['license']})")
        a("")
    return "\n".join(L).rstrip() + "\n"


def collect() -> tuple[list[dict], list[dict]]:
    return cargo_components(cargo_metadata()), npm_components()


def policy(comps: list[dict]) -> list[str]:
    allowed = allowed_from_deny(ROOT / "deny.toml")
    bad = []
    for c in comps:
        if c["license"] in ("UKJENT", "(se lisensfil)") or not spdx_ok(c["license"], allowed):
            bad.append(f"{c['ecosystem']}: {c['name']} {c['version']}: {c['license']}")
    return bad


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--check", action="store_true", help="feil hvis THIRD_PARTY.md er utdatert eller lisens ikke er tillatt")
    ap.add_argument("--policy", action="store_true", help="bare lisenssjekk")
    args = ap.parse_args()
    cargo, npm = collect()
    bad = policy(cargo + npm)
    if bad:
        print("Lisenser utenfor tillatt liste (deny.toml; npm-pakker bruker samme liste):", *bad, sep="\n  ", file=sys.stderr)
    if args.policy:
        return 1 if bad else 0
    text = render(cargo, npm)
    if args.check:
        if bad:
            return 1
        if not OUT.exists() or OUT.read_text(encoding="utf-8") != text:
            print("THIRD_PARTY.md er utdatert: kjør `just third-party`", file=sys.stderr)
            return 1
        print(f"THIRD_PARTY.md er oppdatert ({len(cargo) + len(npm)} komponenter)")
        return 0
    OUT.write_text(text, encoding="utf-8")
    print(f"skrev {OUT.relative_to(ROOT)}: {len(cargo)} Rust + {len(npm)} npm, {len(text) // 1024} kB")
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main())
