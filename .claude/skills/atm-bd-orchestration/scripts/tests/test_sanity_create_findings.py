from __future__ import annotations

import importlib.machinery
import importlib.util
from pathlib import Path
import sys
import unittest

SCRIPTS = Path(__file__).parents[1]
sys.path.insert(0, str(SCRIPTS))
LOADER = importlib.machinery.SourceFileLoader("sanity_create_findings", str(SCRIPTS / "sanity-create-findings"))
SPEC = importlib.util.spec_from_loader("sanity_create_findings", LOADER)
module = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(module)


class ParentLayerTests(unittest.TestCase):
    def test_dev_bead_layer(self):
        self.assertEqual(module.parent_layer({"layer": 3, "found_on_layer": 1}), 3)

    def test_finding_bead_falls_back_to_found_on_layer(self):
        self.assertEqual(module.parent_layer({"found_on_layer": "2"}), "2")

    def test_neither_is_a_handoff_error(self):
        with self.assertRaises(module.HandoffError):
            module.parent_layer({"phase": "d"})


if __name__ == "__main__":
    unittest.main()
