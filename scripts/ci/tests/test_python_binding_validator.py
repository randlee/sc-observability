"""Regression tests for the shared Python binding validator contract."""
from __future__ import annotations

from pathlib import Path
import tempfile
import unittest
from unittest import mock

from scripts.ci import python_binding_validator as validator


class PythonBindingValidatorTests(unittest.TestCase):
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

    def test_installed_origins_reject_any_import_outside_the_venv(self) -> None:
        installed = {
            "prefix": "/tmp/venv",
            "package": "/tmp/venv/lib/python/site-packages/sc_observability/__init__.py",
            "native": "/workspace/sc_observability/_native.so",
        }
        with self.assertRaisesRegex(validator.BindingValidationError, "escaped the venv"):
            validator.installed_origins(installed)


if __name__ == "__main__":
    unittest.main()
