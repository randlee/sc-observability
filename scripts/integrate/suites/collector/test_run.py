"""Headless unit tests for the collector integration suite runner."""

from __future__ import annotations

import importlib.util
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest import mock


RUNNER = Path(__file__).with_name("run.py")
SPEC = importlib.util.spec_from_file_location("collector_runner", RUNNER)
assert SPEC is not None and SPEC.loader is not None
runner = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(runner)


class CollectorRunnerTests(unittest.TestCase):
    def test_matrix_reuses_the_legacy_conformance_feature_coverage(self) -> None:
        names = [name for name, _ in runner.CASES]
        features = [command[-1] for _, command in runner.CASES]
        self.assertEqual(
            names,
            ["sync-http-full-stack", "sdk-full-stack", "combined-full-stack", "canonical-ingress"],
        )
        self.assertEqual(features, ["sync-http", "otlp-sdk", "otlp-sdk,sync-http", "otlp-sdk,sync-http"])

    def test_verify_source_sha_rejects_non_commit_input_without_git(self) -> None:
        with mock.patch.object(runner.subprocess, "check_output") as check_output:
            with self.assertRaisesRegex(runner.SuiteError, "40-hex"):
                runner.verify_source_sha("not-a-commit")
        check_output.assert_not_called()

    def test_run_case_retains_a_nonzero_result_without_raising(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            completed = subprocess.CompletedProcess(["cargo"], 7, "stdout\n", "stderr\n")
            with mock.patch.object(runner.subprocess, "run", return_value=completed):
                self.assertFalse(
                    runner.run_case("failed", ["cargo"], environment={}, output=Path(temporary))
                )
            self.assertEqual(
                (Path(temporary) / "failed.log").read_text(),
                "$ cargo\nstdout\nstderr\nexit=7\n",
            )

    def test_run_executes_later_cases_after_an_earlier_failure(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            calls: list[str] = []

            def record(name: str, _command: list[str], **_kwargs: object) -> bool:
                calls.append(name)
                return name != "sync-http-full-stack"

            with mock.patch.object(runner, "verify_source_sha"), mock.patch.object(
                runner, "run_case", side_effect=record
            ):
                self.assertFalse(runner.run("a" * 40, Path(temporary)))
            self.assertEqual(calls, [name for name, _ in runner.CASES])


if __name__ == "__main__":
    unittest.main()
