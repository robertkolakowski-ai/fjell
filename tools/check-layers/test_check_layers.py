#!/usr/bin/env python3
"""Beviser at check-layers godtar gyldig kode og feiler på bevisste brudd (fixtures/)."""
import unittest
from pathlib import Path

import check_layers

FX = Path(__file__).resolve().parent / "fixtures"
REAL_SRC = check_layers.ROOT / "core/crates/fjell-core/src"
REAL_CFG = check_layers.HERE / "layers.toml"


def run(name):
    return check_layers.check(FX / name / "src", FX / name / "layers.toml")


class Fixtures(unittest.TestCase):
    def test_ok_passes(self):
        self.assertEqual(run("ok"), [])

    def test_upward_use_fails(self):
        errs = run("bad_layer")
        self.assertTrue(any("lagbrudd" in e and "`model` (L0)" in e for e in errs), errs)

    def test_cycle_fails(self):
        self.assertTrue(any(e.startswith("syklus:") for e in run("bad_cycle")))

    def test_cli_exit_codes(self):
        for name, code in (("ok", 0), ("bad_layer", 1), ("bad_cycle", 1)):
            argv = ["--src", str(FX / name / "src"), "--config", str(FX / name / "layers.toml")]
            self.assertEqual(check_layers.main(argv), code, name)

    def test_real_core_is_clean(self):
        self.assertEqual(check_layers.check(REAL_SRC, REAL_CFG), [])


if __name__ == "__main__":
    unittest.main()
