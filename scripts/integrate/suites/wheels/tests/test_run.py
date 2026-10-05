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
    @mock.patch.object(run.python_binding_validator, "maturin_build_command")
    @mock.patch.object(run.python_binding_validator, "maturin_version", return_value="1.10.2")
    @mock.patch.object(run.python_binding_validator, "venv_python", return_value=Path("/tmp/output/build-venv/bin/python"))
    def test_build_wheel_provisions_the_pinned_maturin_for_the_shared_declared_command(
            self, builder: mock.Mock, version: mock.Mock, command: mock.Mock, checked: mock.Mock) -> None:
        command.return_value = ["/tmp/output/build-venv/bin/python", "-m", "maturin", "build"]
        with mock.patch.object(run, "recreate_output_directory"), mock.patch.object(Path, "glob", return_value=[Path("/tmp/wheel.whl")]):
            wheel = run.build_wheel(Path("/tmp/output"))
        self.assertEqual(wheel, Path("/tmp/wheel.whl"))
        commands = [call.args[0] for call in checked.call_args_list]
        self.assertEqual(commands[0], [run.sys.executable, "-m", "venv", "/tmp/output/build-venv"])
        self.assertEqual(
            commands[1],
            ["/tmp/output/build-venv/bin/python", "-m", "pip", "install", "--disable-pip-version-check", "--no-input", "maturin==1.10.2"],
        )
        self.assertEqual(commands[2], command.return_value)
        version.assert_called_once_with(run.ROOT)
        builder.assert_called_once_with(Path("/tmp/output/build-venv"))
        command.assert_called_once_with(
            run.ROOT,
            Path("/tmp/output/wheel"),
            maturin_command=["/tmp/output/build-venv/bin/python", "-m", "maturin"],
        )

    @mock.patch.object(run, "stage_installed_runtime_suite", return_value=Path("/tmp/output/runtime/tests"))
    @mock.patch.object(run, "run_checked")
    def test_prepare_installed_runtime_uses_isolated_interpreter_and_installs_candidate(
            self, checked: mock.Mock, staged: mock.Mock) -> None:
        # Avoid a platform filesystem dependency while preserving the command contract.
        with mock.patch.object(run, "recreate_output_directory"), mock.patch.object(run.python_binding_validator, "venv_python", return_value=Path("/tmp/venv/bin/python")):
            python, runtime, tests = run.prepare_installed_runtime(Path("/tmp/candidate.whl"), Path("/tmp/output"))
        commands = [call.args[0] for call in checked.call_args_list]
        self.assertEqual(commands[0][:3], [run.sys.executable, "-m", "venv"])
        self.assertIn("--no-input", commands[1])
        self.assertEqual(commands[2][1:3], ["-m", "pip"])
        staged.assert_called_once_with(Path("/tmp/output/runtime"))
        self.assertEqual(python, Path("/tmp/venv/bin/python"))
        self.assertEqual(runtime, Path("/tmp/output/runtime"))
        self.assertEqual(tests, Path("/tmp/output/runtime/tests"))

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

    def test_recreate_output_directory_removes_retained_runner_artifacts(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary) / "runtime"
            directory.mkdir()
            stale = directory / "stale.txt"
            stale.write_text("old", encoding="utf-8")
            run.recreate_output_directory(directory)
            self.assertTrue(directory.is_dir())
            self.assertFalse(stale.exists())

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

    def test_run_records_each_successful_observation_and_attaches_the_venv_interpreter(self) -> None:
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
                mock.patch.object(run, "build_wheel", return_value=wheel),
                mock.patch.object(run, "prepare_installed_runtime", return_value=(Path(installed["python"]), output / "runtime", output / "runtime/tests")),
                mock.patch.object(run, "run_installed_runtime_tests"),
                mock.patch.object(run, "run_installed_typing_tests"),
                mock.patch.object(run, "probe_installed_runtime", return_value=installed),
                mock.patch.object(run, "run_embedded_host") as embedded_host,
            ):
                result = run.run("a" * 40, output)
            report = json.loads((output / "result.json").read_text(encoding="utf-8"))
        embedded_host.assert_called_once_with(Path(installed["python"]), package)
        self.assertEqual(result["status"], "passed")
        self.assertEqual(result["wheel"]["path"], str(wheel))
        self.assertEqual(result["checks"]["installed_telemetry_lifecycle"]["status"], "passed")
        self.assertEqual(result["installed_artifacts"], installed)
        self.assertEqual(report, result)

    def test_run_records_a_runtime_failure_and_runs_independent_checks(self) -> None:
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
            with (
                mock.patch.object(run, "build_wheel", return_value=wheel),
                mock.patch.object(run, "prepare_installed_runtime", return_value=(Path(installed["python"]), output / "runtime", output / "runtime/tests")),
                mock.patch.object(run, "run_installed_runtime_tests", side_effect=run.SuiteError("runtime assertion failed")),
                mock.patch.object(run, "run_installed_typing_tests") as typing_tests,
                mock.patch.object(run, "probe_installed_runtime", return_value=installed),
                mock.patch.object(run, "run_embedded_host") as embedded_host,
            ):
                result = run.run("a" * 40, output)
            report = json.loads((output / "result.json").read_text(encoding="utf-8"))
        self.assertEqual(result["status"], "failed")
        self.assertEqual(result["checks"]["owned_typed_runtime"], {"status": "failed", "detail": "runtime assertion failed"})
        self.assertEqual(result["checks"]["installed_typing"]["status"], "passed")
        self.assertEqual(result["checks"]["installed_telemetry_lifecycle"]["status"], "passed")
        self.assertEqual(result["checks"]["rust_host_attached_python"]["status"], "passed")
        typing_tests.assert_called_once()
        embedded_host.assert_called_once()
        self.assertEqual(report, result)

    def test_main_writes_failure_report_and_returns_one_for_suite_error(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            output = Path(temporary) / "output"
            with mock.patch.object(run, "run", side_effect=run.SuiteError("complete failure context")):
                exit_code = run.main(["--source-sha", "a" * 40, "--output-dir", str(output)])
            report = (output / "failure-report.txt").read_text(encoding="utf-8")
        self.assertEqual(exit_code, 1)
        self.assertIn("SuiteError: complete failure context", report)

    def test_main_writes_a_failure_report_when_builder_provisioning_fails(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            output = Path(temporary) / "output"
            with mock.patch.object(run, "run_checked", side_effect=run.SuiteError("maturin setup failed")):
                exit_code = run.main(["--source-sha", "a" * 40, "--output-dir", str(output)])
            report = (output / "failure-report.txt").read_text(encoding="utf-8")
        self.assertEqual(exit_code, 1)
        self.assertEqual(json.loads(report)["checks"]["wheel_build"], {"status": "failed", "detail": "maturin setup failed"})

    def test_main_writes_observed_setup_failure_results(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            output = Path(temporary) / "output"
            with mock.patch.object(run, "build_wheel", side_effect=run.SuiteError("builder setup failed")):
                exit_code = run.main(["--source-sha", "a" * 40, "--output-dir", str(output)])
            report = json.loads((output / "failure-report.txt").read_text(encoding="utf-8"))
        self.assertEqual(exit_code, 1)
        self.assertEqual(report["status"], "failed")
        self.assertEqual(report["checks"]["wheel_build"], {"status": "failed", "detail": "builder setup failed"})
        self.assertEqual(report["checks"]["clean_install"]["status"], "blocked")


if __name__ == "__main__":
    unittest.main()
