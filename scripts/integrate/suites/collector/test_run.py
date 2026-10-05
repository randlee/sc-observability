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

    def test_matrix_reuses_the_legacy_conformance_feature_coverage(self) -> None:
        names = [name for name, _ in runner.CASES]
        features = [command[command.index("--features") + 1] for _, command in runner.CASES]
        self.assertEqual(
            names,
            ["sync-http-full-stack", "sdk-full-stack", "combined-full-stack", "canonical-ingress"],
        )
        self.assertEqual(features, ["sync-http", "otlp-sdk", "otlp-sdk,sync-http", "otlp-sdk,sync-http"])
        self.assertTrue(all(command[-2:] == ["--", "--nocapture"] for _, command in runner.CASES))

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
                "$ cargo\nstdout\nstderr\nexit=7\ncleanup=cargo-process-exited\n",
            )

    def test_timeout_receipt_normalizes_bytes_and_runs_later_cases(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            timeout = subprocess.TimeoutExpired(
                ["cargo"], runner.CASE_TIMEOUT_SECONDS, output=b"partial\xff output\n", stderr=None
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
            with (
                mock.patch.object(runner, "os", posix),
                mock.patch.object(runner, "signal", posix_signal),
                mock.patch.object(runner.subprocess, "Popen", side_effect=[timed_out, completed, completed, completed]),
            ):
                self.assertFalse(runner.run("a" * 40, Path(temporary)))
            posix.killpg.assert_called_once_with(timed_out.pid, posix_signal.SIGKILL)
            output = Path(temporary)
            self.assertEqual(
                (output / "sync-http-full-stack.log").read_text(encoding="utf-8"),
                "$ cargo test --locked -p sc-observability-otlp --test full_stack_integration --features sync-http -- --nocapture\n"
                "partial\ufffd output\n"
                f"timeout={runner.CASE_TIMEOUT_SECONDS}\n"
                "cleanup=posix-process-group-killed\n",
            )
            self.assertIn(
                "later output\nexit=0\n",
                (output / "canonical-ingress.log").read_text(encoding="utf-8"),
            )

    def test_run_executes_later_cases_after_an_earlier_failure(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            calls: list[str] = []

            def record(name: str, _command: list[str], **_kwargs: object) -> bool:
                calls.append(name)
                return name != "sync-http-full-stack"

            with mock.patch.object(runner, "run_case", side_effect=record):
                self.assertFalse(runner.run("a" * 40, Path(temporary)))
            self.assertEqual(calls, [name for name, _ in runner.CASES])

    def test_run_preserves_the_caller_environment_without_ci_impersonation(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            environments: list[dict[str, str]] = []

            def record(_name: str, _command: list[str], **kwargs: object) -> bool:
                environments.append(kwargs["environment"])
                return True

            with (
                mock.patch.dict(runner.os.environ, {"COLLECTOR_TEST_ENV": "preserved"}, clear=True),
                mock.patch.object(runner, "run_case", side_effect=record),
            ):
                self.assertTrue(runner.run("a" * 40, Path(temporary)))
            self.assertEqual(environments, [{"COLLECTOR_TEST_ENV": "preserved"}] * len(runner.CASES))


if __name__ == "__main__":
    unittest.main()
