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
    def test_start_timeout_exceeds_the_harness_readiness_window(self) -> None:
        self.assertEqual(90, runner.START_TIMEOUT_SECONDS)
        self.assertGreater(runner.START_TIMEOUT_SECONDS, 30)
        self.assertGreater(runner.START_TIMEOUT_SECONDS, runner.CLEANUP_TIMEOUT_SECONDS)

    def test_readback_timeout_exceeds_the_harness_query_window(self) -> None:
        self.assertEqual(180, runner.READBACK_TIMEOUT_SECONDS)
        self.assertGreater(runner.READBACK_TIMEOUT_SECONDS, 120)
        self.assertGreater(runner.READBACK_TIMEOUT_SECONDS, runner.CLEANUP_TIMEOUT_SECONDS)

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
            stops: list[list[str]] = []

            def checked(command: list[str], **_kwargs: object) -> str:
                commands.append(command)
                if command[0] == "cargo" and runner.TESTS["sync-http"] in command:
                    raise runner.SuiteError("command exited 17", exit_code=17)
                return json.dumps({"binary": "viewer", "binary_sha256": "digest", "version": "v1"})

            def harness(arguments: list[str], **_kwargs: object) -> str:
                if arguments[0] == "start":
                    state = output / "viewer-state"
                    state.mkdir()
                    (state / "viewer.pid").write_text("123\n")
                    return ""
                if arguments[0] == "stop":
                    stops.append(arguments)
                    return ""
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
            self.assertEqual(1, len(stops))

    def test_successful_run_writes_the_complete_result_schema_and_stops_viewer(self) -> None:
        source_sha = "C" * 40
        with tempfile.TemporaryDirectory() as temporary:
            output = Path(temporary)
            stops: list[list[str]] = []

            def checked(_command: list[str], **_kwargs: object) -> str:
                return json.dumps({"binary": "viewer", "binary_sha256": "digest", "version": "v1"})

            def harness(arguments: list[str], **_kwargs: object) -> str:
                if arguments[0] == "start":
                    state = output / "viewer-state"
                    state.mkdir()
                    (state / "viewer.pid").write_text("123\n")
                    return ""
                if arguments[0] == "stop":
                    stops.append(arguments)
                    return ""
                return json.dumps({"backend": arguments[-1], "records": 3})

            with mock.patch.object(runner, "verify_source_sha"), \
                    mock.patch.object(runner, "reserved_ports", return_value=(10001, 10002, 10003)), \
                    mock.patch.object(runner, "run_checked", side_effect=checked), \
                    mock.patch.object(runner, "invoke_harness", side_effect=harness):
                result = runner.run(source_sha, output)

            self.assertEqual({"schema_version", "status", "source_commit", "viewer", "backends", "cleanup"}, result.keys())
            self.assertEqual("passed", result["status"])
            self.assertEqual(source_sha.lower(), result["source_commit"])
            self.assertEqual(set(runner.TESTS), result["backends"].keys())
            self.assertTrue(all(outcome["status"] == "passed" for outcome in result["backends"].values()))
            self.assertEqual("passed", result["cleanup"]["status"])
            self.assertEqual(result, json.loads((output / "result.json").read_text()))
            self.assertEqual(1, len(stops))

    def test_start_failure_does_not_run_backends_and_retains_failed_result(self) -> None:
        source_sha = "d" * 40
        with tempfile.TemporaryDirectory() as temporary:
            output = Path(temporary)
            commands: list[list[str]] = []

            def checked(command: list[str], **_kwargs: object) -> str:
                commands.append(command)
                return json.dumps({"binary": "viewer", "binary_sha256": "digest", "version": "v1"})

            def harness(arguments: list[str], **_kwargs: object) -> str:
                if arguments[0] == "start":
                    raise runner.SuiteError("viewer start failed")
                self.fail(f"unexpected harness call: {arguments}")

            with mock.patch.object(runner, "verify_source_sha"), \
                    mock.patch.object(runner, "reserved_ports", return_value=(10001, 10002, 10003)), \
                    mock.patch.object(runner, "run_checked", side_effect=checked), \
                    mock.patch.object(runner, "invoke_harness", side_effect=harness):
                with self.assertRaisesRegex(runner.SuiteError, "start failed"):
                    runner.run(source_sha, output)

            self.assertFalse(any(command[0] == "cargo" for command in commands))
            result = json.loads((output / "result.json").read_text())
            self.assertEqual("failed", result["status"])
            self.assertEqual("not-needed", result["cleanup"]["status"])

    def test_partial_start_is_stopped_and_cleanup_is_retained(self) -> None:
        source_sha = "e" * 40
        with tempfile.TemporaryDirectory() as temporary:
            output = Path(temporary)
            stops: list[list[str]] = []

            def checked(_command: list[str], **_kwargs: object) -> str:
                return json.dumps({"binary": "viewer", "binary_sha256": "digest", "version": "v1"})

            def harness(arguments: list[str], **_kwargs: object) -> str:
                if arguments[0] == "start":
                    state = output / "viewer-state"
                    state.mkdir()
                    (state / "viewer.pid").write_text("123\n")
                    raise runner.SuiteError("start failed after spawn")
                if arguments[0] == "stop":
                    stops.append(arguments)
                    return ""
                self.fail(f"unexpected harness call: {arguments}")

            with mock.patch.object(runner, "verify_source_sha"), \
                    mock.patch.object(runner, "reserved_ports", return_value=(10001, 10002, 10003)), \
                    mock.patch.object(runner, "run_checked", side_effect=checked), \
                    mock.patch.object(runner, "invoke_harness", side_effect=harness):
                with self.assertRaisesRegex(runner.SuiteError, "start failed after spawn"):
                    runner.run(source_sha, output)

            result = json.loads((output / "result.json").read_text())
            self.assertEqual("passed", result["cleanup"]["status"])
            self.assertEqual(1, len(stops))

    def test_cleanup_failure_is_retained_without_masking_primary_backend_failure(self) -> None:
        source_sha = "f" * 40
        with tempfile.TemporaryDirectory() as temporary:
            output = Path(temporary)

            def checked(command: list[str], **_kwargs: object) -> str:
                if command[0] == "cargo" and runner.TESTS["sync-http"] in command:
                    raise runner.SuiteError("command exited 17", exit_code=17)
                return json.dumps({"binary": "viewer", "binary_sha256": "digest", "version": "v1"})

            def harness(arguments: list[str], **_kwargs: object) -> str:
                if arguments[0] == "start":
                    state = output / "viewer-state"
                    state.mkdir()
                    (state / "viewer.pid").write_text("123\n")
                    return ""
                if arguments[0] == "stop":
                    raise runner.SuiteError("cleanup failed")
                return json.dumps({"backend": arguments[-1]})

            with mock.patch.object(runner, "verify_source_sha"), \
                    mock.patch.object(runner, "reserved_ports", return_value=(10001, 10002, 10003)), \
                    mock.patch.object(runner, "run_checked", side_effect=checked), \
                    mock.patch.object(runner, "invoke_harness", side_effect=harness):
                with self.assertRaisesRegex(runner.SuiteError, "sync-http"):
                    runner.run(source_sha, output)

            result = json.loads((output / "result.json").read_text())
            self.assertEqual("failed", result["cleanup"]["status"])
            self.assertIn("cleanup failed", result["cleanup"]["error"])
            self.assertIn("cleanup_error=cleanup failed", (output / "cleanup.log").read_text())
            self.assertEqual("failed", result["backends"]["sync-http"]["status"])

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
