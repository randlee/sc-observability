"""Headless unit tests for the collector integration suite runner."""

from __future__ import annotations

import importlib.util
from pathlib import Path
import subprocess
import tempfile
from types import SimpleNamespace
import unittest
from unittest import mock


RUNNER = Path(__file__).with_name("run.py")
SPEC = importlib.util.spec_from_file_location("collector_runner", RUNNER)
assert SPEC is not None and SPEC.loader is not None
runner = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(runner)


class CollectorRunnerTests(unittest.TestCase):
    @staticmethod
    def process(returncode: int, stdout: str | bytes, stderr: str | bytes) -> mock.Mock:
        process = mock.Mock()
        process.pid = 1234
        process.returncode = returncode
        process.communicate.return_value = (stdout, stderr)
        return process

    def test_setup_and_cases_run_the_real_collector_suite_commands(self) -> None:
        output = Path("/out")
        tests = runner.ROOT / "tests/telemetry-e2e"
        self.assertEqual(
            runner.setup_commands(output),
            [
                ("install-pytest", [runner.sys.executable, "-m", "pip", "install",
                                    "pytest==8.4.2", "pytest-timeout==2.4.0"]),
                ("download-collector", [runner.sys.executable, str(tests / "download_collector.py"),
                                        str(output / "otelcol-contrib")]),
            ],
        )
        self.assertEqual(
            runner.case_commands(output),
            [
                ("native", [runner.sys.executable, "-m", "pytest", "-q", str(tests / "test_native.py"),
                            f"--junitxml={output / 'native.xml'}"]),
                ("frontends", [runner.sys.executable, "-m", "pytest", "-q", str(tests / "test_frontends.py"),
                               f"--junitxml={output / 'frontends.xml'}"]),
                ("harness-timeouts", [runner.sys.executable, "-m", "pytest", "-q",
                                      str(tests / "test_harness_timeouts.py"),
                                      f"--junitxml={output / 'harness-timeouts.xml'}"]),
            ],
        )
        for _, command in runner.case_commands(output):
            self.assertTrue(Path(command[4]).is_file(), command[4])
        self.assertTrue((tests / "download_collector.py").is_file())
        self.assertTrue((tests / "collector-release.json").is_file())

    def test_windows_cases_request_a_new_process_group(self) -> None:
        windows = SimpleNamespace(name="nt")
        with mock.patch.object(runner, "os", windows):
            self.assertEqual(runner.case_process_options(), {"creationflags": 0x00000200})

    def test_run_case_retains_a_nonzero_result_without_raising(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            process = self.process(7, "stdout\n", "stderr\n")
            with mock.patch.object(runner.subprocess, "Popen", return_value=process):
                self.assertFalse(
                    runner.run_case("failed", ["cargo"], environment={}, output=Path(temporary))
                )
            self.assertEqual(
                (Path(temporary) / "failed.log").read_text(encoding="utf-8"),
                "$ cargo\nstdout\nstderr\nexit=7\ncleanup=process-exited\n",
            )

    def test_timeout_receipt_normalizes_bytes_and_runs_later_cases(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            timeout = subprocess.TimeoutExpired(
                ["pytest"], runner.CASE_TIMEOUT_SECONDS, output=b"partial\xff output\n", stderr=None
            )
            timed_out = self.process(1, b"", b"")
            timed_out.communicate.side_effect = [timeout, (b"", None)]
            completed = self.process(0, "later output\n", "")
            posix = SimpleNamespace(
                environ=runner.os.environ,
                name="posix",
                killpg=mock.Mock(),
            )
            posix_signal = SimpleNamespace(SIGKILL=object())
            # Two setup processes, then the timed-out native case and two later cases.
            with (
                mock.patch.object(runner, "os", posix),
                mock.patch.object(runner, "signal", posix_signal),
                mock.patch.object(
                    runner.subprocess, "Popen",
                    side_effect=[completed, completed, timed_out, completed, completed],
                ),
            ):
                self.assertFalse(runner.run("a" * 40, Path(temporary)))
            posix.killpg.assert_called_once_with(timed_out.pid, posix_signal.SIGKILL)
            output = Path(temporary)
            native_command = " ".join(runner.case_commands(output)[0][1])
            self.assertEqual(
                (output / "native.log").read_text(encoding="utf-8"),
                f"$ {native_command}\n"
                "partial\ufffd output\n"
                f"timeout={runner.CASE_TIMEOUT_SECONDS}\n"
                "cleanup=posix-process-group-killed\n",
            )
            self.assertIn("later output\nexit=0\n", (output / "frontends.log").read_text(encoding="utf-8"))
            self.assertIn("later output\nexit=0\n", (output / "harness-timeouts.log").read_text(encoding="utf-8"))

    def test_run_executes_later_cases_after_an_earlier_failure(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            calls: list[str] = []

            def record(name: str, _command: list[str], **_kwargs: object) -> bool:
                calls.append(name)
                return name != "native"

            with mock.patch.object(runner, "run_case", side_effect=record):
                self.assertFalse(runner.run("a" * 40, Path(temporary)))
            self.assertEqual(
                calls,
                ["install-pytest", "download-collector", "native", "frontends", "harness-timeouts"],
            )

    def test_failed_setup_runs_no_test_case_and_fails(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            calls: list[str] = []

            def record(name: str, _command: list[str], **_kwargs: object) -> bool:
                calls.append(name)
                return name != "download-collector"

            with mock.patch.object(runner, "run_case", side_effect=record):
                self.assertFalse(runner.run("a" * 40, Path(temporary)))
            self.assertEqual(calls, ["install-pytest", "download-collector"])

    def test_failing_case_propagates_a_nonzero_exit_from_main(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            failed = self.process(1, "FAILED\n", "")
            passed = self.process(0, "ok\n", "")
            with mock.patch.object(runner.subprocess, "Popen", side_effect=[passed, passed, passed, failed, passed]):
                self.assertEqual(
                    runner.main(["--source-sha", "a" * 40, "--output-dir", temporary]), 1)
            self.assertIn("exit=1", (Path(temporary) / "frontends.log").read_text(encoding="utf-8"))
        with tempfile.TemporaryDirectory() as temporary:
            passed = self.process(0, "ok\n", "")
            with mock.patch.object(runner.subprocess, "Popen", return_value=passed):
                self.assertEqual(
                    runner.main(["--source-sha", "a" * 40, "--output-dir", temporary]), 0)

    def test_cases_receive_the_pinned_collector_binary_without_ci_impersonation(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            environments: dict[str, dict[str, str]] = {}

            def record(name: str, _command: list[str], **kwargs: object) -> bool:
                environments[name] = kwargs["environment"]
                return True

            with (
                mock.patch.dict(runner.os.environ, {"COLLECTOR_TEST_ENV": "preserved"}, clear=True),
                mock.patch.object(runner, "run_case", side_effect=record),
            ):
                self.assertTrue(runner.run("a" * 40, Path(temporary)))
            self.assertEqual(environments["install-pytest"], {"COLLECTOR_TEST_ENV": "preserved"})
            binary = str(Path(temporary) / runner.COLLECTOR_BINARY_NAME)
            for name in ("native", "frontends", "harness-timeouts"):
                self.assertEqual(environments[name], {
                    "COLLECTOR_TEST_ENV": "preserved", "TELEMETRY_E2E_COLLECTOR_BINARY": binary})


if __name__ == "__main__":
    unittest.main()
