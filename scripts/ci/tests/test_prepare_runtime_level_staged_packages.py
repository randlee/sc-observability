"""Regression coverage for baseline-version-derived staging rewrites (QA1 IMPORTANT-2)."""
import sys
import tomllib
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from prepare_runtime_level_staged_packages import candidate_workspace_manifest, normalized_lock


class BaselineDerivedRewriteTests(unittest.TestCase):
    def test_workspace_manifest_advances_plain_and_exact_pins_for_any_baseline(self):
        for baseline in ("1.2.0", "1.4.0", "2.7.9"):
            content = (
                f'[workspace.package]\nversion = "{baseline}"\n'
                "[workspace.dependencies]\n"
                f'sc-observability = {{ version = "{baseline}", path = "crates/sc-observability" }}\n'
                f'sc-observability-log-macros = {{ version = "={baseline}", path = "crates/sc-observability-log-macros" }}\n'
            )
            rendered = tomllib.loads(candidate_workspace_manifest(content, "9.9.9"))
            self.assertEqual(rendered["workspace"]["package"]["version"], "9.9.9")
            self.assertEqual(rendered["workspace"]["dependencies"]["sc-observability"]["version"], "9.9.9")
            self.assertEqual(rendered["workspace"]["dependencies"]["sc-observability-log-macros"]["version"], "=9.9.9")

    def test_normalized_lock_rewrites_any_baseline_not_only_1_2_0(self):
        import tempfile
        with tempfile.TemporaryDirectory() as directory:
            lock = Path(directory) / "Cargo.lock"
            lock.write_text(
                '[[package]]\nname = "sc-observability"\nversion = "2.7.9"\n\n'
                '[[package]]\nname = "third-party"\nversion = "2.7.9"\n'
            )
            normalized = tomllib.loads(normalized_lock(lock, "9.9.9").decode())["package"]
            self.assertEqual(normalized[0]["version"], "9.9.9")
            self.assertEqual(normalized[1]["version"], "2.7.9")


if __name__ == "__main__":
    unittest.main()
