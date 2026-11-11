#!/usr/bin/env python3
"""Negative tester for gen-i18n: manglende nøkkel, ulik plassholder og ikke-støttet ICU skal feile."""
import json
import tempfile
import unittest
from pathlib import Path

import gen_i18n


def run(nb, en):
    with tempfile.TemporaryDirectory() as d:
        for loc, entries in (("nb", nb), ("en", en)):
            body = {"_meta": {"locale": loc, "version": 1}, **{k: {"t": v} for k, v in entries.items()}}
            Path(d, f"{loc}.json").write_text(json.dumps(body), encoding="utf-8")
        return gen_i18n.check(Path(d))[0]


class Parity(unittest.TestCase):
    def test_ok(self):
        self.assertEqual(run({"a.b": "{n} stk"}, {"a.b": "{n} pcs"}), [])

    def test_missing_key(self):
        self.assertTrue(any("mangler nøkkel" in e for e in run({"a.b": "x", "a.c": "y"}, {"a.b": "x"})))

    def test_placeholder_mismatch(self):
        self.assertTrue(any("plassholdere ulike" in e for e in run({"a.b": "{size}"}, {"a.b": "{count}"})))

    def test_plural_args_count(self):
        t = "{count, plural, one {# a} other {# b}} {size}"
        self.assertEqual(run({"a.b": t}, {"a.b": t}), [])
        self.assertTrue(run({"a.b": t}, {"a.b": "{count, plural, one {# a} other {# b}}"}))

    def test_unsupported_icu(self):
        self.assertTrue(any("ikke støttet" in e for e in run({"a.b": "{n, number}"}, {"a.b": "{n, number}"})))

    def test_plural_needs_other(self):
        self.assertTrue(run({"a.b": "{n, plural, one {x}}"}, {"a.b": "{n, plural, one {x}}"}))

    def test_short_too_long(self):
        def r(nb, en):
            with tempfile.TemporaryDirectory() as d:
                for loc, t in (("nb", nb), ("en", en)):
                    body = {"_meta": {"locale": loc, "version": 1}, "a.b": {"t": t, "len": "short"}}
                    Path(d, f"{loc}.json").write_text(json.dumps(body), encoding="utf-8")
                return gen_i18n.check(Path(d))[0]
        self.assertEqual(r("Lagre alle", "Save all"), [])
        self.assertTrue(any("for lang" in e for e in r("Lagre", "Save everything now")))

    def test_real_catalog(self):
        self.assertEqual(gen_i18n.check(gen_i18n.ROOT / "i18n")[0], [])


class Pseudo(unittest.TestCase):
    def test_structure_kept(self):
        out = gen_i18n.pseudo("{count, plural, one {# bilde} other {# bilder}} ({size})")
        self.assertEqual(gen_i18n.placeholders(out), {"count", "size"})
        self.assertTrue(out.startswith("[") and out.endswith("]"))
        self.assertIn("# bildæ", out)

    def test_length_and_accents(self):
        src = "Kom i gang med oppryddingen"
        out = gen_i18n.pseudo(src)
        self.assertGreaterEqual(len(out) - 2, int(len(src) * 1.3))
        self.assertIn("å", out)
        self.assertIn("·", out)

    def test_types(self):
        self.assertEqual(gen_i18n.placeholder_kinds("{n, plural, one {# a} other {# b}} {size} {n}"), {"n": "plural", "size": "arg"})


class Generated(unittest.TestCase):
    def test_checked_in_files_current(self):
        cat = gen_i18n.check(gen_i18n.ROOT / "i18n")[1]
        self.assertEqual(gen_i18n.stale(gen_i18n.render(cat), gen_i18n.OUT_DIR), [])

    def test_stale_detected(self):
        cat = gen_i18n.check(gen_i18n.ROOT / "i18n")[1]
        with tempfile.TemporaryDirectory() as d:
            self.assertEqual(len(gen_i18n.stale(gen_i18n.render(cat), Path(d))), 4)

    def test_missing_reason_key(self):
        with tempfile.TemporaryDirectory() as d:
            ts = Path(d, "types.ts")
            ts.write_text('export type ReasonCode =\n  | "ExactCopy" | "BrandNew";\n')
            cat = {"nb": {"reason.exact_copy": {"t": "x"}}}
            errs = gen_i18n.check_codes(cat, ts, Path(d, "nope.rs"))
            self.assertEqual(len(errs), 1)
            self.assertIn("reason.brand_new", errs[0])


if __name__ == "__main__":
    unittest.main()
