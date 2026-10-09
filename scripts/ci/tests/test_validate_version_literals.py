"""Regression coverage for release inventory validation."""
import json
import sys
import tempfile
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

    def test_roster_excludes_binary_only_published_crates(self):
        inventory = {
            "qualificationCandidate": {
                "packages": ["lib-crate"],
                "deferredStandalonePackages": validate_version_literals.APPROVED_DEFERRED_STANDALONE_PACKAGES,
            }
        }
        manifest = {
            "crates": [
                {"package": name, "cargo_toml": f"{name}/Cargo.toml"}
                for name in ("lib-crate", "bin-crate", "sc-observability-tauri")
            ]
        }
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            for name, source in (("lib-crate", "lib.rs"), ("bin-crate", "main.rs"), ("sc-observability-tauri", "lib.rs")):
                (root / name / "src").mkdir(parents=True)
                (root / name / "src" / source).write_text("", encoding="utf-8")
                (root / name / "Cargo.toml").write_text(f'[package]\nname = "{name}"\n', encoding="utf-8")
            validate_version_literals.validate_api_package_roster(inventory, manifest, root)
            (root / "bin-crate/src/lib.rs").write_text("", encoding="utf-8")
            with self.assertRaisesRegex(ValueError, r"omitted=\['bin-crate'\]"):
                validate_version_literals.validate_api_package_roster(inventory, manifest, root)


if __name__ == "__main__":
    unittest.main()
