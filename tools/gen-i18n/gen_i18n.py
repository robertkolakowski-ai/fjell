#!/usr/bin/env python3
"""gen-i18n (fase UX-0 i docs/mvp/PLAN-UX-I18N.md del 5).

Leser i18n/nb.json (kilde) og i18n/en.json. Uten argument: sjekker OG skriver genererte
TypeScript-moduler til desktop/src/lib/i18n/strings/ (keys.ts, nb.ts, en.ts, pseudo.ts).
`--check` skriver ingenting; feiler også hvis de innsjekkede genererte filene er utdatert.

Sjekker:
  - nøkkelparitet nb/en, `_meta.locale` lik filnavn, nøkkelnavn `<område>.<element>.<tilstand>`
  - plassholderparitet (navn, inkl. plural/select-argumenter)
  - ICU-delmengde: bare enkle plassholdere, `plural` og `select` (begge krever `other`)
  - `en` <= 1,3 x lengden av nb for `len: short` (minst nb + 3 tegn, så korte ord ikke gir falske treff)
  - hver ReasonCode (desktop/src/bridge/types.ts) og ConsequenceCode (core .../plan.rs) har nøkkel
Pseudo-lokale (`pseudo`) lages fra nb: litterær tekst +30 % (utfylt med «·»), aksenttegn
(a->å, e->æ, o->ø, u->ü), ramme [ ]. ICU-struktur og plassholdere beholdes uendret.
Genererte filer sjekkes inn (build og vitest trenger ikke Python); `--check` i CI fanger avvik.

Kun standardbibliotek. Bruk: gen_i18n.py [--check] [--i18n-dir i18n] [--out-dir <mappe>]
"""
import argparse
import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent.parent
LOCALES = ("nb", "en")  # nn kommer etter MVP (nøkkelvis fallback til nb i runtime)
OUT_DIR = ROOT / "desktop" / "src" / "lib" / "i18n" / "strings"
TYPES_TS = ROOT / "desktop" / "src" / "bridge" / "types.ts"
PLAN_RS = ROOT / "core" / "crates" / "fjell-core" / "src" / "model" / "plan.rs"
SHORT_RATIO = 1.3
SHORT_SLACK = 3  # tegn: forhindrer falske treff på ord på 3-6 tegn der en er litt lengre enn nb
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


def placeholder_kinds(msg, kinds=None):
    """Navn -> «plural» | «select» | «arg» (mest spesifikke vinner: plural > select > arg)."""
    rank = {"arg": 0, "select": 1, "plural": 2}
    kinds = {} if kinds is None else kinds
    i = 0
    while i < len(msg):
        if msg[i] != "{":
            i += 1
            continue
        j = _group_end(msg, i)
        head, sep, rest = msg[i + 1 : j].partition(",")
        name = head.strip()
        kind = rest.partition(",")[0].strip() if sep else "arg"
        if rank[kind] >= rank.get(kinds.get(name, "arg"), 0):
            kinds[name] = kind
        if sep:
            opts, k = rest.partition(",")[2], 0
            while True:
                m = re.compile(r"\s*([^\s{]+)\s*\{").match(opts, k)
                if not m:
                    break
                ge = _group_end(opts, m.end() - 1)
                placeholder_kinds(opts[m.end() : ge], kinds)
                k = ge + 1
        i = j + 1
    return kinds


def visible_len(msg):
    """Omtrentlig synlig lengde: litteral tekst, lengste gren i plural/select, plassholdere = 0."""
    n, i = 0, 0
    while i < len(msg):
        if msg[i] != "{":
            n += 1
            i += 1
            continue
        j = _group_end(msg, i)
        _, sep, rest = msg[i + 1 : j].partition(",")
        if sep:
            opts, k, best = rest.partition(",")[2], 0, 0
            while True:
                m = re.compile(r"\s*([^\s{]+)\s*\{").match(opts, k)
                if not m:
                    break
                ge = _group_end(opts, m.end() - 1)
                best = max(best, visible_len(opts[m.end() : ge]))
                k = ge + 1
            n += best
        i = j + 1
    return n


ACCENTS = str.maketrans("aeouAEOU", "åæøüÅÆØÜ")


def pseudo(msg):
    """Pseudo-lokale av én nb-melding. Rører bare litteral tekst; ICU-struktur beholdes."""

    def conv(m):
        out, i = [], 0
        while i < len(m):
            if m[i] != "{":
                out.append(m[i].translate(ACCENTS) if m[i] != "#" else "#")
                i += 1
                continue
            j = _group_end(m, i)
            inner = m[i + 1 : j]
            head, sep, rest = inner.partition(",")
            if not sep:
                out.append("{" + inner + "}")
            else:
                kind, _, opts = rest.partition(",")
                parts, k = [], 0
                while True:
                    mm = re.compile(r"\s*([^\s{]+)\s*\{").match(opts, k)
                    if not mm:
                        break
                    ge = _group_end(opts, mm.end() - 1)
                    parts.append(f"{mm.group(1)} {{{conv(opts[mm.end() : ge])}}}")
                    k = ge + 1
                out.append("{" + f"{head.strip()}, {kind.strip()}, " + " ".join(parts) + "}")
            i = j + 1
        return "".join(out)

    pad = -(-visible_len(msg) * 3 // 10)  # ceil(30 %)
    return "[" + conv(msg) + "·" * pad + "]"


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
    for key in sorted(set.intersection(*(set(c) for c in cat.values()))):
        nb, en = cat[src][key], cat["en"][key]
        if isinstance(nb, dict) and isinstance(en, dict) and nb.get("len") == "short":
            a, b = nb.get("t", ""), en.get("t", "")
            if isinstance(a, str) and isinstance(b, str) and len(b) > max(SHORT_RATIO * len(a), len(a) + SHORT_SLACK):
                errors.append(f"en:{key}: for lang for `len: short` ({len(b)} tegn mot {len(a)} i nb)")
    return errors, cat


def snake(name):
    return re.sub(r"(?<!^)(?=[A-Z])", "_", name).lower()


def enum_variants(path, enum, kind):
    """Variantnavn fra TS-union (`kind='ts'`) eller Rust-enum (`kind='rs'`). None hvis filen mangler."""
    try:
        text = path.read_text(encoding="utf-8")
    except OSError:
        return None
    if kind == "ts":
        m = re.search(r"type\s+%s\s*=([^;]*);" % enum, text)
        return re.findall(r'"([A-Za-z]+)"', m.group(1)) if m else []
    m = re.search(r"enum\s+%s\s*\{(.*?)\n\}" % enum, text, re.S)
    if not m:
        return []
    body = re.sub(r"//[^\n]*", "", m.group(1))
    return re.findall(r"\b([A-Z][A-Za-z0-9]*)\s*,", body)


def check_codes(cat, types_ts=TYPES_TS, plan_rs=PLAN_RS):
    """Hver kjernekode (ReasonCode, ConsequenceCode) skal ha nøkkel `reason.*` / `consequence.*`."""
    errors, nb = [], cat["nb"]
    for path, enum, kind, prefix in ((types_ts, "ReasonCode", "ts", "reason"), (plan_rs, "ConsequenceCode", "rs", "consequence")):
        names = enum_variants(path, enum, kind)
        if names is None:
            print(f"gen-i18n: advarsel: {path} finnes ikke, hopper over {enum}", file=sys.stderr)
            continue
        if not names:
            errors.append(f"{enum}: fant ingen varianter i {path} (parser utdatert?)")
        for n in names:
            if f"{prefix}.{snake(n)}" not in nb:
                errors.append(f"{enum}::{n}: mangler nøkkel «{prefix}.{snake(n)}»")
    return errors


TS_HEADER = "// GENERERT av tools/gen-i18n fra i18n/*.json (just i18n-gen). Ikke rediger for hånd.\n"
ARG_TS = {"plural": "number", "select": "string", "arg": "string | number"}


def render(cat):
    """Mapper filnavn -> innhold for de genererte TS-modulene."""
    keys = list(cat["nb"])
    files = {}
    lines = [TS_HEADER, "/** Parametre per nøkkel (plural -> number, select -> string, ellers string | number). */",
             "export interface MessageParams {"]
    for key in keys:
        kinds = placeholder_kinds(cat["nb"][key]["t"])
        body = "; ".join(f"{n}: {ARG_TS[kinds[n]]}" for n in sorted(kinds))
        lines.append(f"  {json.dumps(key)}: " + ("{ " + body + " }" if body else "Record<never, never>") + ";")
    lines += ["}", "", "export type MessageKey = keyof MessageParams;", "",
              "/** Nøkler for knapper, faner og merker (`len: short`): kort plass i layout. */",
              "export const SHORT_KEYS: ReadonlySet<MessageKey> = new Set<MessageKey>(["]
    lines += [f"  {json.dumps(k)}," for k in keys if cat["nb"][k].get("len") == "short"]
    lines += ["]);", ""]
    files["keys.ts"] = "\n".join(lines)
    texts = {"nb": {k: cat["nb"][k]["t"] for k in keys}, "en": {k: cat["en"][k]["t"] for k in keys},
             "pseudo": {k: pseudo(cat["nb"][k]["t"]) for k in keys}}
    for loc, d in texts.items():
        out = [TS_HEADER, 'import type { MessageKey } from "./keys";', "",
               f"export const {loc}: Readonly<Record<MessageKey, string>> = {{"]
        out += [f"  {json.dumps(k)}: {json.dumps(v, ensure_ascii=False)}," for k, v in d.items()]
        out += ["};", ""]
        files[f"{loc}.ts"] = "\n".join(out)
    return files


def stale(files, out_dir):
    errors = []
    for name, body in files.items():
        p = out_dir / name
        try:
            ok = p.read_text(encoding="utf-8") == body
        except OSError:
            ok = False
        if not ok:
            errors.append(f"{p}: genererte filer er utdatert, kjør `just i18n-gen`")
    return errors


def generate(cat, out_dir):
    out_dir.mkdir(parents=True, exist_ok=True)
    files = render(cat)
    for name, body in files.items():
        (out_dir / name).write_text(body, encoding="utf-8")
    print(f"gen-i18n: skrev {len(files)} filer til {out_dir} ({len(cat['nb'])} nøkler)")


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--check", action="store_true", help="bare sjekk (skriver ikke, feiler på utdatert generert kode)")
    ap.add_argument("--i18n-dir", type=Path, default=ROOT / "i18n")
    ap.add_argument("--out-dir", type=Path, default=OUT_DIR)
    a = ap.parse_args(argv)
    errors, cat = check(a.i18n_dir)
    if not errors:
        errors += check_codes(cat)
        if a.check:
            errors += stale(render(cat), a.out_dir)
    for e in errors:
        print(f"gen-i18n: {e}", file=sys.stderr)
    if errors:
        print(f"gen-i18n: FEIL ({len(errors)})", file=sys.stderr)
        return 1
    print(f"gen-i18n: OK ({len(cat['nb'])} nøkler, {'/'.join(LOCALES)} + pseudo)")
    if not a.check:
        generate(cat, a.out_dir)
    return 0


if __name__ == "__main__":
    sys.exit(main())
