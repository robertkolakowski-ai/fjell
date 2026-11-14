import unittest

import gen_third_party as g

ALLOWED = {"MIT", "Apache-2.0", "Unicode-3.0", "Apache-2.0 WITH LLVM-exception"}


class Spdx(unittest.TestCase):
    def test_enkel(self):
        self.assertTrue(g.spdx_ok("MIT", ALLOWED))
        self.assertFalse(g.spdx_ok("GPL-3.0-only", ALLOWED))

    def test_or_krever_en(self):
        self.assertTrue(g.spdx_ok("GPL-3.0-only OR MIT", ALLOWED))
        self.assertTrue(g.spdx_ok("MIT/Apache-2.0", ALLOWED))  # gammel cargo-skrivemåte
        self.assertFalse(g.spdx_ok("GPL-3.0-only OR AGPL-3.0", ALLOWED))

    def test_and_krever_alle(self):
        self.assertTrue(g.spdx_ok("MIT AND Unicode-3.0", ALLOWED))
        self.assertFalse(g.spdx_ok("MIT AND GPL-3.0-only", ALLOWED))

    def test_parentes_og_with(self):
        self.assertTrue(g.spdx_ok("(MIT OR GPL-3.0-only) AND Unicode-3.0", ALLOWED))
        self.assertFalse(g.spdx_ok("(GPL-3.0-only OR AGPL-3.0) AND MIT", ALLOWED))
        self.assertTrue(g.spdx_ok("Apache-2.0 WITH LLVM-exception", ALLOWED))

    def test_tom(self):
        self.assertFalse(g.spdx_ok("", ALLOWED))


class Render(unittest.TestCase):
    def comp(self, eco, name, lic, text):
        return {"ecosystem": eco, "name": name, "version": "1.0.0", "license": lic, "source": "https://example.test", "texts": [("LICENSE", text)] if text else []}

    def test_dedupliserer_like_tekster_og_lister_manglende(self):
        a = self.comp("cargo", "a", "MIT", "MIT License\n\nCopyright X")
        b = self.comp("cargo", "b", "MIT", "MIT   License\nCopyright X")  # kun whitespace-forskjell
        c = self.comp("npm", "c", "MIT", "")
        out = g.render([a, b], [c])
        self.assertEqual(out.count("### Tekst"), 1)
        self.assertIn("a 1.0.0, b 1.0.0", out)
        self.assertIn("Komponenter uten lisensfil", out)
        self.assertIn("- c 1.0.0 (MIT)", out)
        self.assertIn("**3**", out)

    def test_bundlede_ressurser_i_egen_seksjon(self):
        f = self.comp("asset", "Skrift", "OFL-1.1", "SIL OPEN FONT LICENSE")
        out = g.render([], [], [f])
        self.assertIn("## Bundlede ressurser", out)
        self.assertIn("ressurser 1", out)
        self.assertEqual(g.policy([f]), [])  # OFL-1.1 er tillatt for ressurser
        self.assertEqual(len(g.policy([dict(f, ecosystem="npm")])), 1)  # men ikke for npm/cargo

    def test_deterministisk(self):
        a = self.comp("cargo", "a", "MIT", "t")
        self.assertEqual(g.render([a], []), g.render([a], []))

    def test_deny_toml_tillatt_liste_leses(self):
        allowed = g.allowed_from_deny(g.ROOT / "deny.toml")
        self.assertIn("MIT", allowed)
        self.assertNotIn("GPL-3.0-only", allowed)


if __name__ == "__main__":
    unittest.main()
