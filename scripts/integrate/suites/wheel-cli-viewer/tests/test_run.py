"""Timeout regression coverage for the wheel/CLI viewer integration runner."""
from __future__ import annotations

import importlib.util
import json
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch


RUNNER = Path(__file__).resolve().parents[1] / "run.py"
SPEC = importlib.util.spec_from_file_location("wheel_cli_viewer_runner", RUNNER)
assert SPEC and SPEC.loader
runner = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(runner)


class RunnerTimeoutTests(unittest.TestCase):
    def test_timeout_preserves_command_output_and_failure_exit(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            log = Path(temporary) / "wheel-cli-viewer.log"
            failure = subprocess.TimeoutExpired(["pytest", "suite"], 7, output=b"partial stdout", stderr="partial stderr")
            with patch.object(runner.subprocess, "run", side_effect=failure):
                result = runner.run(["pytest", "suite"], timeout=7, env={}, log=log)
            self.assertEqual(runner.TIMEOUT_EXIT_CODE, result.returncode)
            evidence = log.read_text()
            self.assertIn("$ pytest suite", evidence)
            self.assertIn("partial stdout", evidence)
            self.assertIn("partial stderr", evidence)
            self.assertIn("timeout=7s", evidence)
            self.assertIn("exit=124", evidence)

    def test_timeout_cleanup_stops_only_this_runs_viewer_state(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            state_root = Path(temporary) / "pytest-state"
            owned = state_root / "test_case" / "viewer-state"
            owned.mkdir(parents=True)
            calls: list[list[str]] = []

            def stopped(command, *, timeout, env, log):
                calls.append(command)
                return subprocess.CompletedProcess(command, 0)

            with patch.object(runner, "run", side_effect=stopped):
                runner.cleanup_timed_out_viewers(state_root, env={}, log=state_root / "log")
            self.assertEqual(1, len(calls))
            self.assertEqual("stop", calls[0][2])
            self.assertEqual(str(owned), calls[0][-1])

    def test_pytest_timeout_runs_owned_viewer_cleanup(self) -> None:
        receipt = json.dumps({
            "binary": "viewer", "binary_sha256": "a" * 64, "platform": "linux_amd64",
        })
        with tempfile.TemporaryDirectory() as temporary:
            output = Path(temporary) / "evidence"
            results = [
                subprocess.CompletedProcess(["pip"], 0, "", ""),
                subprocess.CompletedProcess(["download"], 0, receipt, ""),
                subprocess.CompletedProcess(["pytest"], runner.TIMEOUT_EXIT_CODE, "", ""),
            ]
            with patch.object(runner, "run", side_effect=results), \
                 patch.object(runner, "cleanup_timed_out_viewers") as cleanup:
                self.assertEqual(runner.TIMEOUT_EXIT_CODE,
                                 runner.main(["--source-sha", "a" * 40, "--output-dir", str(output)]))
            cleanup.assert_called_once()
            self.assertEqual(output / "pytest-state", cleanup.call_args.args[0])


if __name__ == "__main__":
    unittest.main()
