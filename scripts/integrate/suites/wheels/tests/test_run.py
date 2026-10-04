"""Unit tests for the bounded wheels integration runner."""
from __future__ import annotations

import json
import os
from pathlib import Path
import tempfile
import unittest
from unittest import mock

from scripts.integrate.suites.wheels import run


class WheelsRunnerTests(unittest.TestCase):
    def test_shared_venv_python_uses_platform_native_layout(self) -> None:
        root = Path("/tmp/e5-venv")
        validator = run.python_binding_validator
        self.assertEqual(validator.venv_python(root, platform_name="posix"), root / "bin/python")
        self.assertEqual(validator.venv_python(root, platform_name="nt"), root / "Scripts/python.exe")

    def test_run_checked_retains_timeout_stdout_and_stderr(self) -> None:
        timeout = run.subprocess.TimeoutExpired(["probe"], 7, output="timed stdout", stderr="timed stderr")
        with mock.patch.object(run.subprocess, "run", side_effect=timeout):
            with self.assertRaises(run.SuiteError) as raised:
                run.run_checked(["probe"], cwd=Path("/tmp"), timeout=7)
        self.assertIn("timed out after 7s", str(raised.exception))
        self.assertIn("timed stdout", str(raised.exception))
        self.assertIn("timed stderr", str(raised.exception))

    def test_run_checked_retains_nonzero_stdout_and_stderr(self) -> None:
        failure = run.subprocess.CalledProcessError(23, ["probe"], output="failed stdout", stderr="failed stderr")
        with mock.patch.object(run.subprocess, "run", side_effect=failure):
            with self.assertRaises(run.SuiteError) as raised:
                run.run_checked(["probe"], cwd=Path("/tmp"))
        self.assertIn("failed (23)", str(raised.exception))
        self.assertIn("failed stdout", str(raised.exception))
        self.assertIn("failed stderr", str(raised.exception))

    @mock.patch.object(run.python_binding_validator.subprocess, "check_output")
    def test_windows_embedded_environment_exposes_selected_dll_directory(self, check_output: mock.Mock) -> None:
        check_output.return_value = "C:\\Python310\n"
        python_path = Path(r"C:\venv\Scripts\python.exe")
        with mock.patch.dict(os.environ, {"PATH": r"C:\\Windows"}, clear=True):
            environment = run.python_binding_validator.embedded_environment(python_path, platform_name="windows")
        self.assertEqual(environment["PYO3_PYTHON"], str(python_path))
        self.assertEqual(environment["PYTHONHOME"], "C:\\Python310")
        self.assertTrue(environment["PATH"].startswith("C:\\Python310"))

    @mock.patch.object(run.python_binding_validator.subprocess, "check_output")
    def test_linux_embedded_environment_exposes_selected_library_directory(self, check_output: mock.Mock) -> None:
        check_output.side_effect = ["/opt/python\n", "/opt/python/lib\n"]
        with mock.patch.dict(os.environ, {"LD_LIBRARY_PATH": "/existing"}, clear=True):
            environment = run.python_binding_validator.embedded_environment(Path("/venv/bin/python"), platform_name="linux")
        self.assertEqual(environment["PYTHONHOME"], "/opt/python")
        self.assertEqual(environment["LD_LIBRARY_PATH"], f"/opt/python/lib{os.pathsep}/existing")

    @mock.patch.object(run, "run_checked")
    def test_verify_source_sha_rejects_a_different_checkout(self, checked: mock.Mock) -> None:
        checked.return_value = "a" * 40 + "\n"
        with self.assertRaisesRegex(run.SuiteError, "does not match"):
            run.verify_source_sha("b" * 40)
        self.assertEqual(checked.call_args.args[0], ["git", "rev-parse", "HEAD"])

    @mock.patch.object(run, "run_checked")
    @mock.patch.object(run.python_binding_validator, "maturin_build_command")
    def test_build_wheel_uses_the_shared_declared_build_command(self, command: mock.Mock, checked: mock.Mock) -> None:
        command.return_value = ["/tools/uvx", "--from", "maturin==1.10.2", "maturin", "build"]
        with mock.patch.object(Path, "mkdir"), mock.patch.object(Path, "glob", return_value=[Path("/tmp/wheel.whl")]):
            wheel = run.build_wheel(Path("/tmp/output"))
        self.assertEqual(wheel, Path("/tmp/wheel.whl"))
        self.assertEqual(checked.call_args.args[0], command.return_value)
        command.assert_called_once_with(run.ROOT, Path("/tmp/output/wheel"))

    @mock.patch.object(run, "run_installed_typing_tests")
    @mock.patch.object(run, "run_installed_runtime_tests")
    @mock.patch.object(run, "stage_installed_runtime_suite", return_value=Path("/tmp/output/runtime/tests"))
    @mock.patch.object(run, "run_checked")
    def test_install_probe_uses_isolated_interpreter_and_installs_candidate(
            self, checked: mock.Mock, staged: mock.Mock, runtime_tests: mock.Mock,
            typing_tests: mock.Mock) -> None:
        # Avoid a platform filesystem dependency while preserving the command contract.
        checked.return_value = '{"prefix": "/tmp/venv", "native": "/tmp/venv/native.so", "package": "/tmp/venv/package.py"}\n'
        with mock.patch.object(Path, "mkdir"), mock.patch.object(run.python_binding_validator, "venv_python", return_value=Path("/tmp/venv/bin/python")):
            installed = run.install_and_probe(Path("/tmp/candidate.whl"), Path("/tmp/output"))
        commands = [call.args[0] for call in checked.call_args_list]
        self.assertEqual(commands[0][:3], [run.sys.executable, "-m", "venv"])
        self.assertIn("--no-input", commands[1])
        self.assertEqual(commands[2][1:3], ["-m", "pip"])
        self.assertEqual(commands[3][1:5], ["-I", "-X", "dev", "-W"])
        runtime_tests.assert_called_once_with(
            Path("/tmp/venv/bin/python"), Path("/tmp/output/runtime"), Path("/tmp/output/runtime/tests")
        )
        typing_tests.assert_called_once_with(
            Path("/tmp/venv/bin/python"), Path("/tmp/output/runtime"), Path("/tmp/output/runtime/tests")
        )
        staged.assert_called_once_with(Path("/tmp/output/runtime"))
        self.assertEqual(installed["python"], "/tmp/venv/bin/python")

    def test_installed_probe_exercises_the_enabled_telemetry_lifecycle(self) -> None:
        probe = run.IMPORT_AND_TELEMETRY_PROBE
        self.assertIn('getattr(native, "open", None)', probe)
        self.assertIn("Telemetry.open(", probe)
        self.assertIn("telemetry.emit(submission)", probe)
        self.assertIn("telemetry.flush(timeout_s=0.1)", probe)
        self.assertIn("telemetry.status()", probe)
        self.assertIn("telemetry.shutdown(timeout_s=0.1)", probe)
        self.assertIn("TelemetryErr", probe)

    @mock.patch.object(run, "run_checked")
    def test_installed_runtime_tests_use_the_real_owned_and_async_suites_under_strict_diagnostics(
            self, checked: mock.Mock) -> None:
        python = Path("/tmp/venv/bin/python")
        runtime = Path("/tmp/runtime")
        tests = runtime / "tests"
        with mock.patch.dict(os.environ, {"PATH": "/usr/bin"}, clear=True):
            run.run_installed_runtime_tests(python, runtime, tests)
        command = checked.call_args.args[0]
        environment = checked.call_args.kwargs["environment"]
        self.assertEqual(command[:8], [str(python), "-I", "-X", "dev", "-W", "error", "-m", "pytest"])
        self.assertEqual(command[8:10], [str(tests / "test_runtime.py"), str(tests / "test_async_runtime.py")])
        self.assertEqual(environment["SC_OBSERVABILITY_RUNTIME_TEST"], "1")
        self.assertEqual(environment["PYTHONDEVMODE"], "1")
        self.assertEqual(environment["PYTHONASYNCIODEBUG"], "1")
        self.assertEqual(environment["PYTHONWARNINGS"], "error")

    @mock.patch.object(run, "run_checked")
    def test_installed_typing_tests_cover_all_public_typed_result_suites(self, checked: mock.Mock) -> None:
        python = Path("/tmp/venv/bin/python")
        runtime = Path("/tmp/runtime")
        tests = runtime / "tests"
        run.run_installed_typing_tests(python, runtime, tests)
        command = checked.call_args.args[0]
        self.assertEqual(command[:7], [str(python), "-I", "-m", "mypy", "--strict", "--python-version", "3.10"])
        self.assertEqual(
            command[7:],
            [
                str(tests / "typing/test_result_narrowing.py"),
                str(tests / "typing/test_async_narrowing.py"),
                str(tests / "typing/test_telemetry_typing.py"),
            ],
        )

    def test_shared_installed_origin_rejects_a_source_tree(self) -> None:
        installed = {
            "python": "/tmp/e5-venv/bin/python",
            "prefix": "/tmp/e5-venv",
            "native": "/tmp/e5-venv/lib/python3.14/site-packages/sc_observability/_native.so",
            "package": "/workspace/bindings/python/sc-observability-py/python/sc_observability/__init__.py",
        }
        with self.assertRaisesRegex(run.python_binding_validator.BindingValidationError, "escaped the venv"):
            run.python_binding_validator.installed_origins(installed)

    @mock.patch.object(run, "run_checked")
    @mock.patch.object(run.python_binding_validator, "embedded_environment", return_value={"PYO3_PYTHON": "/tmp/e5-venv/bin/python"})
    def test_embedded_host_receives_the_installed_package_contract(
            self, environment: mock.Mock, checked: mock.Mock) -> None:
        package = Path("/tmp/e5-venv/lib/python3.14/site-packages/sc_observability/__init__.py")
        run.run_embedded_host(Path("/tmp/e5-venv/bin/python"), package)
        self.assertEqual(environment.call_args.args[0], Path("/tmp/e5-venv/bin/python"))
        self.assertEqual(environment.call_args.kwargs["timeout"], run.RUNTIME_TIMEOUT_SECONDS)
        self.assertEqual(
            checked.call_args.kwargs["environment"]["SC_OBSERVABILITY_ATTACHED_PACKAGE"],
            str(package),
        )
        self.assertEqual(checked.call_args.kwargs["environment"]["PYTHONDEVMODE"], "1")
        self.assertEqual(checked.call_args.kwargs["environment"]["PYTHONASYNCIODEBUG"], "1")
        self.assertEqual(checked.call_args.kwargs["environment"]["PYTHONWARNINGS"], "error")

    @mock.patch.object(run.python_binding_validator, "embedded_environment", side_effect=run.python_binding_validator.BindingValidationError("probe timed out"))
    def test_embedded_host_converts_a_bounded_interpreter_probe_failure_to_suite_error(
            self, environment: mock.Mock) -> None:
        with self.assertRaisesRegex(run.SuiteError, "probe timed out"):
            run.run_embedded_host(Path("/tmp/venv/bin/python"), Path("/tmp/package.py"))
        self.assertEqual(environment.call_args.kwargs["timeout"], run.RUNTIME_TIMEOUT_SECONDS)

    def test_run_composes_result_and_attaches_the_venv_interpreter(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            output = Path(temporary) / "output"
            wheel = Path(temporary) / "candidate.whl"
            wheel.write_bytes(b"candidate wheel")
            installed = {
                "python": "/tmp/e5-venv/bin/python",
                "prefix": "/tmp/e5-venv",
                "native": "/tmp/e5-venv/lib/python3.14/site-packages/sc_observability/_native.so",
                "package": "/tmp/e5-venv/lib/python3.14/site-packages/sc_observability/__init__.py",
            }
            package = Path(installed["package"]).resolve()
            with (
                mock.patch.object(run, "verify_source_sha"),
                mock.patch.object(run, "build_wheel", return_value=wheel),
                mock.patch.object(run, "install_and_probe", return_value=installed),
                mock.patch.object(run, "run_embedded_host") as embedded_host,
            ):
                result = run.run("a" * 40, output)
            report = json.loads((output / "result.json").read_text(encoding="utf-8"))
        embedded_host.assert_called_once_with(Path(installed["python"]), package)
        self.assertEqual(result["status"], "passed")
        self.assertEqual(result["wheel"]["path"], str(wheel))
        self.assertEqual(result["checks"]["installed_telemetry_lifecycle"], "passed")
        self.assertEqual(result["installed_artifacts"], installed)
        self.assertEqual(report, result)

    def test_main_writes_failure_report_and_returns_one_for_suite_error(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            output = Path(temporary) / "output"
            with mock.patch.object(run, "run", side_effect=run.SuiteError("complete failure context")):
                exit_code = run.main(["--source-sha", "a" * 40, "--output-dir", str(output)])
            report = (output / "failure-report.txt").read_text(encoding="utf-8")
        self.assertEqual(exit_code, 1)
        self.assertIn("SuiteError: complete failure context", report)


if __name__ == "__main__":
    unittest.main()
