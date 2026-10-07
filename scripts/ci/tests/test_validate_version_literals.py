"""Regression coverage for release inventory validation."""
import json
import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

import validate_version_literals  # noqa: E402


ROOT = Path(__file__).resolve().parents[3]


class VersionLiteralPolicyTests(unittest.TestCase):
    def test_version_validation_checks_the_release_inventory_roster(self):
        inventory = json.loads((ROOT / "release/release-inventory.json").read_text(encoding="utf-8"))
        self.assertIn("qualificationCandidate", inventory)
        validate_version_literals.validate(ROOT)


if __name__ == "__main__":
    unittest.main()
