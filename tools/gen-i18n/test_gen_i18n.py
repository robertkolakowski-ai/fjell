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

    def test_real_catalog(self):
        self.assertEqual(gen_i18n.check(gen_i18n.ROOT / "i18n")[0], [])


if __name__ == "__main__":
    unittest.main()
