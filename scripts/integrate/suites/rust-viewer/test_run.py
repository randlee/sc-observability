"""No-GUI unit tests for the Rust viewer suite runner."""

from __future__ import annotations

import importlib.util
import json
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

    def test_backend_failure_records_both_backend_outcomes_before_reraising(self) -> None:
        source_sha = "a" * 40
        with tempfile.TemporaryDirectory() as temporary:
            output = Path(temporary)
            commands: list[list[str]] = []

            def checked(command: list[str], **_kwargs: object) -> str:
                commands.append(command)
                if command[0] == "cargo" and runner.TESTS["sync-http"] in command:
                    raise runner.SuiteError("command exited 17", exit_code=17)
                return json.dumps({"binary": "viewer", "binary_sha256": "digest", "version": "v1"})

            def harness(arguments: list[str], **_kwargs: object) -> str:
                if arguments[0] == "assert-production":
                    return json.dumps({"backend": arguments[-1], "records": 3})
                return ""

            with mock.patch.object(runner, "verify_source_sha"), \
                    mock.patch.object(runner, "reserved_ports", return_value=(10001, 10002, 10003)), \
                    mock.patch.object(runner, "run_checked", side_effect=checked), \
                    mock.patch.object(runner, "invoke_harness", side_effect=harness):
                with self.assertRaisesRegex(runner.SuiteError, "sync-http"):
                    runner.run(source_sha, output)

            result = json.loads((output / "result.json").read_text())
            self.assertEqual("failed", result["status"])
            self.assertEqual("failed", result["backends"]["sync-http"]["status"])
            self.assertEqual(17, result["backends"]["sync-http"]["test_exit"])
            self.assertIn("sync-http.log", result["backends"]["sync-http"]["log"])
            self.assertEqual("passed", result["backends"]["sdk"]["status"])
            self.assertEqual(0, result["backends"]["sdk"]["test_exit"])
            self.assertEqual("sdk", result["backends"]["sdk"]["readback"]["backend"])
            self.assertTrue(any(runner.TESTS["sdk"] in command for command in commands))

    def test_malformed_readback_is_retained_as_one_backend_failure(self) -> None:
        source_sha = "b" * 40
        with tempfile.TemporaryDirectory() as temporary:
            output = Path(temporary)

            def checked(_command: list[str], **_kwargs: object) -> str:
                return json.dumps({"binary": "viewer", "binary_sha256": "digest", "version": "v1"})

            def harness(arguments: list[str], **_kwargs: object) -> str:
                if arguments[0] != "assert-production":
                    return ""
                return "not-json" if arguments[-1] == "sync-http" else json.dumps({"backend": "sdk"})

            with mock.patch.object(runner, "verify_source_sha"), \
                    mock.patch.object(runner, "reserved_ports", return_value=(10001, 10002, 10003)), \
                    mock.patch.object(runner, "run_checked", side_effect=checked), \
                    mock.patch.object(runner, "invoke_harness", side_effect=harness):
                with self.assertRaisesRegex(runner.SuiteError, "sync-http"):
                    runner.run(source_sha, output)

            result = json.loads((output / "result.json").read_text())
            self.assertEqual("failed", result["backends"]["sync-http"]["status"])
            self.assertEqual(0, result["backends"]["sync-http"]["test_exit"])
            self.assertIn("invalid assert-production receipt", result["backends"]["sync-http"]["error"])
            self.assertEqual("passed", result["backends"]["sdk"]["status"])


if __name__ == "__main__":
    unittest.main()
