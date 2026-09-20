"""Regression coverage for baseline-version-derived staging rewrites (QA1 IMPORTANT-2)."""
import sys
import fnmatch
import re
import tomllib
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from prepare_runtime_level_staged_packages import candidate_workspace_manifest, normalized_lock, normalized_manifest, PACKAGES


class BaselineDerivedRewriteTests(unittest.TestCase):
    def test_every_staged_manifest_resolves_all_inherited_package_metadata(self):
        root = Path(__file__).resolve().parents[3]
        workspace = tomllib.loads((root / "Cargo.toml").read_text())["workspace"]["package"]
        for package in PACKAGES:
            with self.subTest(package=package):
                path = root / "crates" / package / "Cargo.toml"
                original = tomllib.loads(path.read_text())["package"]
                normalized = tomllib.loads(normalized_manifest(path, "9.9.9").decode())["package"]
                for field, value in original.items():
                    if isinstance(value, dict) and value.get("workspace") is True:
                        self.assertEqual(normalized[field], "9.9.9" if field == "version" else workspace[field], field)

    def test_workflow_triggers_cover_authoritative_stage_inputs(self):
        from _runtime_level_common import ROOT, QUALIFICATION, PUBLISH_ARTIFACTS
        workflow = (ROOT / ".github/workflows/bp2-staged-consumer.yml").read_text()
        paths_section = workflow.split("    paths:\n", 1)[1].split("  workflow_dispatch:", 1)[0]
        patterns = re.findall(r"^      - '([^']+)'$", paths_section, re.MULTILINE)
        inputs = ["Cargo.toml", "Cargo.lock", QUALIFICATION.relative_to(ROOT).as_posix(),
                  PUBLISH_ARTIFACTS.relative_to(ROOT).as_posix(),
                  "scripts/ci/_runtime_level_common.py",
                  "scripts/ci/prepare_runtime_level_staged_packages.py",
                  "scripts/ci/tests/test_prepare_runtime_level_staged_packages.py"]
        inputs.extend(path.relative_to(ROOT).as_posix() for path in (ROOT / "scripts/ci").glob("validate_runtime_level_*.py"))
        for path in inputs:
            with self.subTest(path=path):
                self.assertTrue(any(fnmatch.fnmatchcase(path, pattern) for pattern in patterns), path)
        self.assertIn("python3 scripts/ci/tests/test_prepare_runtime_level_staged_packages.py", workflow)

    def test_candidate_workspace_excludes_later_members(self):
        content = '[workspace]\nmembers = ["crates/sc-observability-types", "bindings/python/sc-observability-py"]\n[workspace.package]\nversion = "1.2.0"\n'
        candidate = tomllib.loads(candidate_workspace_manifest(content, "1.3.0"))
        self.assertEqual(candidate["workspace"]["members"], [
            "crates/sc-observability-types", "crates/sc-observability", "crates/sc-observe",
            "crates/sc-observability-otlp",
        ])

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
