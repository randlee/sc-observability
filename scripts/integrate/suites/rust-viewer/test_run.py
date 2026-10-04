"""No-GUI unit tests for the Rust viewer suite runner."""

from __future__ import annotations

import importlib.util
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest import mock


RUNNER = Path(__file__).with_name("run.py")
SPEC = importlib.util.spec_from_file_location("rust_viewer_runner", RUNNER)
assert SPEC is not None and SPEC.loader is not None
runner = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(runner)


class RustViewerRunnerTests(unittest.TestCase):
    def test_reserved_ports_are_distinct_positive_tcp_ports(self) -> None:
        ports = runner.reserved_ports()
        self.assertEqual(len(ports), 3)
        self.assertEqual(len(set(ports)), 3)
        self.assertTrue(all(port > 0 for port in ports))

    def test_verify_source_sha_rejects_non_commit_input_without_git(self) -> None:
        with mock.patch.object(runner.subprocess, "check_output") as check_output:
            with self.assertRaisesRegex(runner.SuiteError, "40-hex"):
                runner.verify_source_sha("not-a-commit")
        check_output.assert_not_called()

    def test_factory_commands_select_each_ignored_public_viewer_case(self) -> None:
        for test_name in runner.TESTS.values():
            command = runner.factory_test_command(test_name)
            self.assertEqual(command[-2:], ["--ignored", "--exact"])
            self.assertIn(test_name, command)
            self.assertIn("sync-http,otlp-sdk", command)

    def test_run_checked_records_command_streams_and_exit_status(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            log = Path(temp) / "command.log"
            completed = subprocess.CompletedProcess(["command"], 0, "out\n", "err\n")
            with mock.patch.object(runner.subprocess, "run", return_value=completed):
                self.assertEqual(
                    runner.run_checked(["command"], environment={}, timeout=1, log=log),
                    "out\n",
                )
            self.assertEqual(log.read_text(), "$ command\nout\nerr\nexit=0\n")


if __name__ == "__main__":
    unittest.main()
