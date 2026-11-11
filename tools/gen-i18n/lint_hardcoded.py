#!/usr/bin/env python3
"""Lint mot hardkodet tekst i desktop/src (PLAN-UX-I18N 5.5, jobb `i18n`).

I `.svelte`-filer feiler:
  - tekstnoder i markup som inneholder bokstaver (utenfor {uttrykk}, <script>, <style>, kommentarer)
  - `aria-label`, `title`, `alt`, `placeholder`, `aria-description` med strenglitteral som inneholder bokstaver
Bruk `{t("nøkkel")}` / `{$tr("nøkkel")}`. Unntak: kommentar `<!-- i18n-ignore: begrunnelse -->` på samme eller forrige linje.
Kun standardbibliotek. Bruk: lint_hardcoded.py [mappe ...]  (standard: desktop/src)
"""
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent.parent
ATTRS = ("aria-label", "aria-description", "title", "alt", "placeholder")
# Dev-galleri for skjermbildetester (fixtures, ikke produkt-UI). Eneste unntak; alt annet bruker t().
EXEMPT = {"desktop/src/Gallery.svelte"}
LETTER = re.compile(r"[^\W\d_]")


def blank(text, a, b):
    """Erstatt text[a:b] med mellomrom, men behold linjeskift (stabile linjenumre)."""
    return text[:a] + re.sub(r"[^\n]", " ", text[a:b]) + text[b:]


def strip_blocks(src):
    for pat in (r"<script\b.*?</script>", r"<style\b.*?</style>", r"<!--.*?-->"):
        while (m := re.search(pat, src, re.S | re.I)):
            src = blank(src, m.start(), m.end())
    return src


def blank_exprs(src):
    """Blank ut balanserte {…} (uttrykk, {#if}, {@html} osv.)."""
    out, i = list(src), 0
    while i < len(src):
        if src[i] != "{":
            i += 1
            continue
        depth, j = 0, i
        while j < len(src):
            depth += (src[j] == "{") - (src[j] == "}")
            if depth == 0:
                break
            j += 1
        for k in range(i, min(j + 1, len(src))):
            if out[k] != "\n":
                out[k] = " "
        i = j + 1
    return "".join(out)


def lint_svelte(text):
    """Liste (linje, melding) for én .svelte-fil."""
    ignore = {n for n, l in enumerate(text.split("\n"), 1) if "i18n-ignore" in l}
    markup = strip_blocks(text)
    found = []
    line_of = lambda pos, s: s.count("\n", 0, pos) + 1
    for m in re.finditer(r"\b(%s)\s*=\s*(\"([^\"]*)\"|'([^']*)')" % "|".join(ATTRS), markup):
        value = m.group(3) if m.group(3) is not None else m.group(4)
        if LETTER.search(blank_exprs(value)):
            found.append((line_of(m.start(), markup), f"hardkodet {m.group(1)}=\"{value.strip()[:40]}\""))
    plain = blank_exprs(markup)
    for m in re.finditer(r"<[^>]*>", plain):
        plain = blank(plain, m.start(), m.end())
    plain = re.sub(r"&#?\w+;", lambda m: " " * len(m.group(0)), plain)
    for m in re.finditer(r"[^\n]+", plain):
        if LETTER.search(m.group(0)):
            found.append((line_of(m.start(), plain), f"hardkodet tekst «{m.group(0).strip()[:40]}»"))
    return sorted((n, msg) for n, msg in found if n not in ignore and n - 1 not in ignore)


def lint_paths(paths):
    errors = []
    for base in paths:
        for f in sorted(Path(base).rglob("*.svelte")):
            if "node_modules" in f.parts or f.resolve().relative_to(ROOT).as_posix() in EXEMPT:
                continue
            for n, msg in lint_svelte(f.read_text(encoding="utf-8")):
                errors.append(f"{f}:{n}: {msg}")
    return errors


def main(argv=None):
    args = (argv if argv is not None else sys.argv[1:]) or [str(ROOT / "desktop" / "src")]
    errors = lint_paths(args)
    for e in errors:
        print(f"lint-hardcoded: {e}", file=sys.stderr)
    if errors:
        print(f"lint-hardcoded: FEIL ({len(errors)})", file=sys.stderr)
        return 1
    print("lint-hardcoded: OK")
    return 0


if __name__ == "__main__":
    sys.exit(main())
