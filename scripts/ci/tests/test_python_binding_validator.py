"""Regression tests for the shared Python binding validator contract."""
from __future__ import annotations

import os
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest import mock

from scripts.ci import python_binding_validator as validator


ROOT = Path(__file__).resolve().parents[3]


class PythonBindingValidatorTests(unittest.TestCase):
    def python_version(self, version: str) -> str:
        return subprocess.check_output(["uv", "python", "find", version], text=True).strip()

    def test_clean_python310_runs_embedded_helper_without_tomli(self) -> None:
        python310 = self.python_version("3.10")
        completed = subprocess.run(
            [python310, "-S", "scripts/ci/python_binding_validator.py", "embedded-environment", python310],
            cwd=ROOT,
            text=True,
            capture_output=True,
            check=False,
        )
        self.assertEqual(completed.returncode, 0, completed.stderr)
        self.assertIn(f"PYO3_PYTHON={python310}", completed.stdout)

    def test_binding_validator_stops_when_embedded_helper_fails(self) -> None:
        python310 = self.python_version("3.10")
        with tempfile.TemporaryDirectory() as temporary:
            temporary_path = Path(temporary)
            fake_uv = temporary_path / "uv"
            fake_uv.write_text("#!/usr/bin/env bash\necho embedded-helper-failed >&2\nexit 23\n", encoding="utf-8")
            fake_uv.chmod(0o755)
            fake_generator = temporary_path / "python312-generator"
            fake_generator.write_text("#!/usr/bin/env bash\nexit 0\n", encoding="utf-8")
            fake_generator.chmod(0o755)
            environment = os.environ | {
                "B4_PYTHON": python310,
                "B4_GENERATOR_PYTHON": str(fake_generator),
                "PATH": f"{temporary}{os.pathsep}{os.environ['PATH']}",
            }
            completed = subprocess.run(
                ["bash", "scripts/ci/validate_python_bindings.sh"],
                cwd=ROOT,
                env=environment,
                text=True,
                capture_output=True,
                check=False,
            )
        self.assertEqual(completed.returncode, 23, completed.stderr)
        self.assertIn("embedded-helper-failed", completed.stderr)

    def test_build_command_reads_the_declared_pin_and_production_features(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            requirements = root / "scripts/ci/python-packaging-requirements.txt"
            requirements.parent.mkdir(parents=True)
            requirements.write_text("maturin==9.8.7\n", encoding="utf-8")
            pyproject = root / "bindings/python/sc-observability-py/pyproject.toml"
            pyproject.parent.mkdir(parents=True)
            pyproject.write_text("[tool.maturin]\nfeatures = [\"production-a\", \"production-b\"]\n", encoding="utf-8")
            with mock.patch.object(validator.shutil, "which", side_effect=[None, "/tools/uvx"]):
                command = validator.maturin_build_command(root, root / "dist")
        self.assertEqual(command[:4], ["/tools/uvx", "--from", "maturin==9.8.7", "maturin"])
        self.assertEqual(command[command.index("--features") + 1], "production-a,production-b")

    def test_build_command_uses_a_provisioned_maturin_command_when_supplied(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            requirements = root / "scripts/ci/python-packaging-requirements.txt"
            requirements.parent.mkdir(parents=True)
            requirements.write_text("maturin==9.8.7\n", encoding="utf-8")
            pyproject = root / "bindings/python/sc-observability-py/pyproject.toml"
            pyproject.parent.mkdir(parents=True)
            pyproject.write_text("[tool.maturin]\nfeatures = [\"production-a\"]\n", encoding="utf-8")
            command = validator.maturin_build_command(
                root,
                root / "dist",
                maturin_command=["/tmp/build-venv/bin/python", "-m", "maturin"],
            )
        self.assertEqual(command[:3], ["/tmp/build-venv/bin/python", "-m", "maturin"])
        self.assertEqual(command[command.index("--features") + 1], "production-a")

    def test_installed_origins_reject_any_import_outside_the_venv(self) -> None:
        installed = {
            "prefix": "/tmp/venv",
            "package": "/tmp/venv/lib/python/site-packages/sc_observability/__init__.py",
            "native": "/workspace/sc_observability/_native.so",
        }
        with self.assertRaisesRegex(validator.BindingValidationError, "escaped the venv"):
            validator.installed_origins(installed)

    @mock.patch.object(validator.subprocess, "check_output")
    def test_interpreter_probe_has_a_deadline_and_reports_timeout(self, check_output: mock.Mock) -> None:
        check_output.side_effect = validator.subprocess.TimeoutExpired(["python"], 17)
        with self.assertRaisesRegex(validator.BindingValidationError, "timed out after 17s"):
            validator.checked_output(["python"], timeout=17)
        self.assertEqual(check_output.call_args.kwargs["timeout"], 17)


if __name__ == "__main__":
    unittest.main()
