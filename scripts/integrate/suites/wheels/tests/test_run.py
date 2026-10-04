"""Unit tests for the bounded wheels integration runner."""
from __future__ import annotations

import os
from pathlib import Path
import unittest
from unittest import mock

from scripts.integrate.suites.wheels import run


class WheelsRunnerTests(unittest.TestCase):
    def test_venv_python_uses_platform_native_layout(self) -> None:
        root = Path("/tmp/e5-venv")
        self.assertEqual(run.venv_python(root, platform_name="posix"), root / "bin/python")
        self.assertEqual(run.venv_python(root, platform_name="nt"), root / "Scripts/python.exe")

    def test_prepend_path_avoids_empty_loader_entries(self) -> None:
        self.assertEqual(run.prepend_path(None, "/python"), "/python")
        self.assertEqual(run.prepend_path("/existing", "/python"), f"/python{os.pathsep}/existing")

    @mock.patch.object(run.subprocess, "check_output")
    def test_windows_embedded_environment_exposes_selected_dll_directory(self, check_output: mock.Mock) -> None:
        check_output.return_value = "C:\\Python310\n"
        with mock.patch.dict(os.environ, {"PATH": r"C:\\Windows"}, clear=True):
            environment = run.embedded_environment(Path(r"C:\\venv\\Scripts\\python.exe"), platform_name="windows")
        self.assertEqual(environment["PYO3_PYTHON"], r"C:\\venv\\Scripts\\python.exe")
        self.assertEqual(environment["PYTHONHOME"], "C:\\Python310")
        self.assertTrue(environment["PATH"].startswith("C:\\Python310"))

    @mock.patch.object(run.subprocess, "check_output")
    def test_linux_embedded_environment_exposes_selected_library_directory(self, check_output: mock.Mock) -> None:
        check_output.side_effect = ["/opt/python\n", "/opt/python/lib\n"]
        with mock.patch.dict(os.environ, {"LD_LIBRARY_PATH": "/existing"}, clear=True):
            environment = run.embedded_environment(Path("/venv/bin/python"), platform_name="linux")
        self.assertEqual(environment["PYTHONHOME"], "/opt/python")
        self.assertEqual(environment["LD_LIBRARY_PATH"], f"/opt/python/lib{os.pathsep}/existing")

    @mock.patch.object(run.subprocess, "check_output")
    def test_verify_source_sha_rejects_a_different_checkout(self, check_output: mock.Mock) -> None:
        check_output.return_value = "a" * 40 + "\n"
        with self.assertRaisesRegex(run.SuiteError, "does not match"):
            run.verify_source_sha("b" * 40)

    @mock.patch.object(run, "run_checked")
    @mock.patch.object(run.shutil, "which")
    def test_build_wheel_uses_pinned_uvx_only_when_maturin_is_missing(self, which: mock.Mock, checked: mock.Mock) -> None:
        which.side_effect = [None, "/tools/uvx"]
        with mock.patch.object(Path, "mkdir"), mock.patch.object(Path, "glob", return_value=[Path("/tmp/wheel.whl")]):
            wheel = run.build_wheel(Path("/tmp/output"))
        self.assertEqual(wheel, Path("/tmp/wheel.whl"))
        command = checked.call_args.args[0]
        self.assertEqual(command[:4], ["/tools/uvx", "--from", "maturin==1.10.2", "maturin"])

    @mock.patch.object(run, "run_checked")
    def test_install_probe_uses_isolated_interpreter_and_installs_candidate(self, checked: mock.Mock) -> None:
        # Avoid a platform filesystem dependency while preserving the command contract.
        checked.return_value = '{"native": "/tmp/venv/native.so", "package": "/tmp/venv/package.py"}\n'
        with mock.patch.object(Path, "mkdir"), mock.patch.object(run, "venv_python", return_value=Path("/tmp/venv/bin/python")):
            installed = run.install_and_probe(Path("/tmp/candidate.whl"), Path("/tmp/output"))
        commands = [call.args[0] for call in checked.call_args_list]
        self.assertEqual(commands[0][:3], [run.sys.executable, "-m", "venv"])
        self.assertIn("--no-input", commands[1])
        self.assertEqual(commands[2][1:3], ["-I", "-c"])
        self.assertEqual(installed["python"], "/tmp/venv/bin/python")

    def test_embedded_package_origin_rejects_a_source_tree(self) -> None:
        installed = {
            "python": "/tmp/e5-venv/bin/python",
            "package": "/workspace/bindings/python/sc-observability-py/python/sc_observability/__init__.py",
        }
        with self.assertRaisesRegex(run.SuiteError, "escaped the installed venv"):
            run.installed_package_origin(installed)

    @mock.patch.object(run, "run_checked")
    @mock.patch.object(run, "embedded_environment", return_value={"PYO3_PYTHON": "/tmp/e5-venv/bin/python"})
    def test_embedded_host_receives_the_installed_package_contract(
            self, environment: mock.Mock, checked: mock.Mock) -> None:
        package = Path("/tmp/e5-venv/lib/python3.14/site-packages/sc_observability/__init__.py")
        run.run_embedded_host(Path("/tmp/e5-venv/bin/python"), package)
        self.assertEqual(environment.call_args.args[0], Path("/tmp/e5-venv/bin/python"))
        self.assertEqual(
            checked.call_args.kwargs["environment"]["SC_OBSERVABILITY_ATTACHED_PACKAGE"],
            str(package),
        )


if __name__ == "__main__":
    unittest.main()
