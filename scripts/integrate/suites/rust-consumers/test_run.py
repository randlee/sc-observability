"""Headless unit tests for the Rust-consumer integration suite runner."""
from __future__ import annotations

import importlib.util
import json
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch


RUNNER = Path(__file__).with_name("run.py")
SPEC = importlib.util.spec_from_file_location("rust_consumers_run", RUNNER)
assert SPEC and SPEC.loader
run = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(run)


class RunnerTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.output = Path(self.temporary.name)

    def test_four_named_independent_cases_are_recorded(self) -> None:
        calls = []

        def execute(command, **_kwargs):
            calls.append(command)
            return subprocess.CompletedProcess(command, 0, "ok\n", "")

        with patch.object(run, "verify_source"), patch.object(run, "candidate_version", return_value="1.4.1"), patch.object(run.subprocess, "run", side_effect=execute):
            self.assertEqual(0, run.run("a" * 40, self.output))
        self.assertEqual(5, len(calls))
        summary = json.loads((self.output / "summary.json").read_text())
        self.assertEqual({"core", "binding-bridge", "runtime-level", "log-bridge"}, set(summary["outcomes"]))
        self.assertTrue(all(summary["outcomes"].values()))

    def test_later_cases_run_when_earlier_case_fails(self) -> None:
        calls = []

        def execute(command, **_kwargs):
            calls.append(command)
            exit_code = 1 if len(calls) == 1 else 0
            return subprocess.CompletedProcess(command, exit_code, "", "failure\n" if exit_code else "")

        with patch.object(run, "verify_source"), patch.object(run, "candidate_version", return_value="1.4.1"), patch.object(run.subprocess, "run", side_effect=execute):
            self.assertEqual(1, run.run("b" * 40, self.output))
        self.assertEqual(5, len(calls))
        summary = json.loads((self.output / "summary.json").read_text())
        self.assertFalse(summary["outcomes"]["core"])
        self.assertTrue(summary["outcomes"]["log-bridge"])

    def test_source_mismatch_is_rejected_before_cases(self) -> None:
        with patch.object(run.subprocess, "check_output", return_value="b" * 40 + "\n"), self.assertRaisesRegex(ValueError, "does not match"):
            run.verify_source("a" * 40)


if __name__ == "__main__":
    unittest.main()
