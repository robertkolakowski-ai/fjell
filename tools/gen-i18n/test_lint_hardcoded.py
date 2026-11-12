#!/usr/bin/env python3
"""Negative tester for lint-hardcoded: hardkodet tekst og attributter skal feile, t()-bruk skal passere."""
import unittest

from lint_hardcoded import lint_svelte


class Lint(unittest.TestCase):
    def test_ok(self):
        src = '<script>const x = "Hei";</script>\n<button aria-label={t("a.b")} title="{$tr(\'a.c\')}">{$tr("a.d")} &nbsp; 42 %</button>\n<style>.a::after{content:"x"}</style>\n<!-- Kommentar -->'
        self.assertEqual(lint_svelte(src), [])

    def test_text_node(self):
        self.assertEqual([n for n, _ in lint_svelte("<p>\n  Hei der\n</p>")], [2])

    def test_attributes(self):
        r = lint_svelte('<button aria-label="Lukk" title=\'Stor\' alt="" placeholder="Søk">{t("a.b")}</button>')
        self.assertEqual(len(r), 3)

    def test_mixed_attribute(self):
        self.assertEqual(len(lint_svelte('<img alt="Bilde {n}" />')), 1)

    def test_ignore_comment(self):
        self.assertEqual(lint_svelte("<!-- i18n-ignore: merkenavn -->\n<h1>Fjell</h1>"), [])

    def test_blocks_and_expr(self):
        self.assertEqual(lint_svelte('{#if a > b}<span>{t("x.y")}</span>{:else}<b>Nei</b>{/if}')[0][0], 1)


if __name__ == "__main__":
    unittest.main()
