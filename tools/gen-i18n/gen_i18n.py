#!/usr/bin/env python3
"""gen-i18n (skall, fase UX-0 i docs/mvp/PLAN-UX-I18N.md del 5).

Leser i18n/nb.json (kilde) og i18n/en.json. `--check` (standard) sjekker:
  - nøkkelparitet nb/en
  - plassholderparitet (navn, inkl. plural/select-argumenter)
  - ICU-delmengde: bare enkle plassholdere, `plural` og `select` (plural krever `other`)
  - `_meta.locale` lik filnavn, og nøkkelnavn `<område>.<element>.<tilstand>` (små bokstaver)
Generering til .xcstrings og pseudo-lokale er stub (TODO UX-0).

Kun standardbibliotek. Bruk: gen_i18n.py [--check] [--i18n-dir i18n]
"""
import argparse
import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent.parent
LOCALES = ("nb", "en")  # nn kommer etter MVP
KEY_RE = re.compile(r"^[a-z0-9_]+(\.[a-z0-9_]+)+$")
NAME_RE = re.compile(r"^[A-Za-z_][A-Za-z0-9_]*$")


class IcuError(ValueError):
    pass


def _group_end(s, i):
    """Indeks til `}` som matcher `{` i s[i]."""
    depth = 0
    for j in range(i, len(s)):
        if s[j] == "{":
            depth += 1
        elif s[j] == "}":
            depth -= 1
            if depth == 0:
                return j
    raise IcuError("ubalansert {")


def placeholders(msg):
    """Mengde plassholdernavn i en melding (ICU-delmengde). Kaster IcuError på alt annet."""
    names, i = set(), 0
    while i < len(msg):
        c = msg[i]
        if c == "}":
            raise IcuError("ubalansert }")
        if c != "{":
            i += 1
            continue
        j = _group_end(msg, i)
        inner = msg[i + 1 : j]
        head, sep, rest = inner.partition(",")
        name = head.strip()
        if not NAME_RE.match(name):
            raise IcuError(f"ugyldig plassholdernavn «{name}»")
        names.add(name)
        if sep:
            kind, sep2, opts = rest.partition(",")
            kind = kind.strip()
            if kind not in ("plural", "select") or not sep2:
                raise IcuError(f"ikke støttet: {{{inner.strip()[:30]}…}} (kun plural og select)")
            seen, k = set(), 0
            while True:
                m = re.compile(r"\s*([^\s{]+)\s*\{").match(opts, k)
                if not m:
                    if opts[k:].strip():
                        raise IcuError(f"ugyldig {kind}-alternativ: «{opts[k:].strip()[:30]}»")
                    break
                gs = m.end() - 1
                ge = _group_end(opts, gs)
                seen.add(m.group(1))
                names |= placeholders(opts[gs + 1 : ge])
                k = ge + 1
            if "other" not in seen:
                raise IcuError(f"{kind} mangler `other`")
        i = j + 1
    return names


def load(i18n_dir, loc, errors):
    path = i18n_dir / f"{loc}.json"
    try:
        data = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as e:
        errors.append(f"{path}: kan ikke leses: {e}")
        return {}
    meta = data.pop("_meta", {})
    if meta.get("locale") != loc:
        errors.append(f"{path}: _meta.locale er {meta.get('locale')!r}, forventet {loc!r}")
    return data


def check(i18n_dir):
    errors = []
    cat = {loc: load(i18n_dir, loc, errors) for loc in LOCALES}
    src = LOCALES[0]
    for loc, entries in cat.items():
        for key, e in entries.items():
            if not KEY_RE.match(key):
                errors.append(f"{loc}: ugyldig nøkkelnavn «{key}»")
            if not isinstance(e, dict) or not isinstance(e.get("t"), str) or not e["t"]:
                errors.append(f"{loc}:{key}: mangler tekst `t`")
            elif e.get("len", "short") not in ("short",):
                errors.append(f"{loc}:{key}: `len` må være «short» hvis satt")
    for loc in LOCALES[1:]:
        for key in sorted(set(cat[src]) - set(cat[loc])):
            errors.append(f"{loc}: mangler nøkkel «{key}» (finnes i {src})")
        for key in sorted(set(cat[loc]) - set(cat[src])):
            errors.append(f"{loc}: nøkkel «{key}» finnes ikke i {src}")
    for key in sorted(set.intersection(*(set(c) for c in cat.values()))):
        found = {}
        for loc in LOCALES:
            t = cat[loc][key].get("t") if isinstance(cat[loc][key], dict) else None
            if not isinstance(t, str):
                continue
            try:
                found[loc] = placeholders(t)
            except IcuError as e:
                errors.append(f"{loc}:{key}: {e}")
        if len(found) == len(LOCALES) and len({frozenset(v) for v in found.values()}) > 1:
            errors.append(
                f"{key}: plassholdere ulike: "
                + ", ".join(f"{l}={sorted(v)}" for l, v in found.items())
            )
    return errors, cat


def generate(cat, i18n_dir):
    # TODO(UX-0): generer Localizable.xcstrings (nb/en) og pseudo-lokale (+30 %, [ ]-ramme,
    # aksenttegn) fra nb. Se docs/mvp/PLAN-UX-I18N.md 5.1 og 5.5. Ikke del av M0.
    print(f"gen-i18n: generering til xcstrings er ikke implementert ennå (TODO UX-0), {len(cat['nb'])} nøkler")


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--check", action="store_true", help="bare sjekk (standard i M0)")
    ap.add_argument("--i18n-dir", type=Path, default=ROOT / "i18n")
    a = ap.parse_args(argv)
    errors, cat = check(a.i18n_dir)
    for e in errors:
        print(f"gen-i18n: {e}", file=sys.stderr)
    if errors:
        print(f"gen-i18n: FEIL ({len(errors)})", file=sys.stderr)
        return 1
    print(f"gen-i18n: OK ({len(cat['nb'])} nøkler, {'/'.join(LOCALES)})")
    if not a.check:
        generate(cat, a.i18n_dir)
    return 0


if __name__ == "__main__":
    sys.exit(main())
