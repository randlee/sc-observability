"""Headless validation of binding-bundle Sandbox proof serialization."""
from __future__ import annotations

import importlib.util
import sys
import tempfile
import unittest
from pathlib import Path
from types import SimpleNamespace


ROOT = Path(__file__).resolve().parents[3]
HELPER_DIR = ROOT / "scripts/ci"
HELPER = HELPER_DIR / "validate_binding_bundle.py"
sys.path.insert(0, str(HELPER_DIR))
SPEC = importlib.util.spec_from_file_location("validate_binding_bundle", HELPER)
assert SPEC and SPEC.loader
bundle = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(bundle)


class BundleEvidenceTests(unittest.TestCase):
    def test_windows_style_sandbox_evidence_keeps_empty_prefix_and_absent_policy(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            manifest_path = root / "manifest.json"
            manifest_path.write_text("{}\n")
            manifest = {
                "source_commit": "a" * 40,
                "packages": [{"name": "sc-observability", "archive_sha256": "archive", "reviewed_requirements": {}}],
                "registry_selection": [],
                "lock_sha256": "lock",
            }
            sandbox = SimpleNamespace(prefix=[], denied=set(), commands=[["cargo", "run"]])
            evidence = bundle.bundle_evidence(
                manifest,
                root,
                {"packages": [{"name": "sc-observability", "version": "1.5.0", "source": None}]},
                {"checkout": {"denied": True}},
                sandbox,
                root / "missing-policy",
                {"CARGO_HOME": "cargo", "CARGO_TARGET_DIR": "target"},
                {},
                "BINDING_CONSUMER_OK\n",
            )
        self.assertEqual([], evidence["sandbox_prefix"])
        self.assertIsNone(evidence["sandbox_policy"])
        self.assertEqual({"sc-observability": "archive"}, evidence["archives"])


if __name__ == "__main__":
    unittest.main()
