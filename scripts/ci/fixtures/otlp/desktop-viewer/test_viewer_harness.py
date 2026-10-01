"""Safety regressions for the isolated desktop viewer lifecycle."""

from __future__ import annotations

import argparse
import hashlib
import json
import socket
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from unittest import mock

import viewer_harness as harness


def _record(state: Path, proc: subprocess.Popen[bytes], database: Path,
            *, binary: str | None = None) -> None:
    command = harness._command_args(proc.pid)
    if binary is None and command:
        binary = command[0]
    if binary is None:
        raise AssertionError("test process has no recorded executable")
    (state / "viewer.pid").write_text(f"{proc.pid}\n")
    (state / "viewer.json").write_text(json.dumps({
        "pid": proc.pid, "binary": binary, "database": str(database)}))


def _owned_process(database: Path, *, ignore_term: bool = False) -> subprocess.Popen[bytes]:
    code = "trap - TERM; sleep 30"
    if ignore_term:
        code = "trap '' TERM; sleep 30"
    # Use a stable direct executable rather than macOS's Python launcher; the
    # test records the actual command identity that the safety guard verifies.
    return subprocess.Popen(["/bin/sh", "-c", code, "--db", str(database)])


def _sdk_viewer_responses(*, log_service: str = "test-service", span_status: str = "Error",
                          distractor_first: bool = False) -> tuple[object, object]:
    """Return the exact decoded fields used by the production-viewer oracle."""
    metrics = [
        {"id": "sum", "name": "agent.canonical.events_total", "metricType": "Sum"},
        {"id": "gauge", "name": "agent.canonical.queue_depth", "metricType": "Gauge"},
        {"id": "zero", "name": "agent.canonical.histogram.zero", "metricType": "Histogram"},
        {"id": "one", "name": "agent.canonical.histogram.one", "metricType": "Histogram"},
        {"id": "many", "name": "agent.canonical.histogram.many", "metricType": "Histogram"},
    ]
    span = {
        "spanID": "1234567890abcdef", "parentSpanID": "abcdef0123456789",
        "name": "agent.run", "kind": "Client", "flags": 1, "statusCode": span_status,
        "r": 1, "attributes": [{"key": "corpus.phase", "value": "decoded"}],
        "events": [{"name": "tool.call", "attributes": [
            {"key": "sc.observability.span_event.parent_span_id", "value": "abcdef0123456789"},
            {"key": "sc.observability.span_event.span_id", "value": "1234567890abcdef"},
            {"key": "sc.observability.span_event.trace_flags", "value": "1"},
            {"key": "sc.observability.span_event.trace_id", "value": "1234567890abcdef1234567890abcdef"},
        ]}],
        "links": [{"traceID": "fedcba9876543210fedcba9876543210", "spanID": "fedcba9876543210",
                   "flags": 3, "attributes": [{"key": "link.reason", "value": "follows"}]}],
    }
    rows = [{"spanData": span}]
    if distractor_first:
        rows.insert(0, {"spanData": {
            "spanID": "feedfacefeedface", "parentSpanID": "abcdef0123456789",
            "name": "agent.run", "kind": "Client", "flags": 1, "statusCode": "Error",
        }})

    def wait(_base: str, method: str, _params: list[object], _needle: str,
             _deadline: float) -> object:
        if method == "searchLogs":
            return [{"id": "log-1", "bodyPreview": "d9-viewer-sdk-factory"}]
        if method == "searchSpans":
            return {"traceID": "1234567890abcdef1234567890abcdef", "resources": {
                "1": {"attributes": [{"key": "service.name", "value": "test-service"}]},
            }, "spans": rows}
        if method != "searchMetricSummaries":
            raise AssertionError(f"unexpected viewer method: {method}")
        return metrics

    def query(_base: str, method: str, params: list[object]) -> object:
        if method == "getLog":
            return {"body": "d9-viewer-sdk-factory", "severityText": "INFO", "severityNumber": 9,
                    "resource": {"attributes": [{"key": "service.name", "value": log_service}]},
                    "attributes": [{"key": "corpus.phase", "value": "decoded"},
                                   {"key": "event.name", "value": "agent.observe"},
                                   {"key": "log.target", "value": "test.agent"}]}
        point = {"doubleValue": 7.0, "sum": 555.0, "count": 10,
                 "explicitBounds": [1.0, 10.0, 100.0], "bucketCounts": [1, 2, 3, 4]}
        if params[0] == "gauge":
            point = {"doubleValue": 3.0}
        elif params[0] == "zero":
            point = {"count": 3, "sum": 4.5, "explicitBounds": [], "bucketCounts": [3]}
        elif params[0] == "one":
            point = {"count": 3, "sum": 20.0, "explicitBounds": [5.0], "bucketCounts": [1, 2]}
        return {"timeseries": [{"datapoints": [point]}]}

    return wait, query


class ViewerHarnessSafetyTests(unittest.TestCase):
    def test_production_assertion_queries_sdk_records_without_submitting_a_probe(self) -> None:
        metadata = {"host": "127.0.0.1", "ui": 8000}
        wait, query = _sdk_viewer_responses(distractor_first=True)

        with mock.patch.object(harness, "_owned", return_value=(123, metadata)), \
                mock.patch.object(harness, "_wait_rpc", side_effect=wait) as wait_rpc, \
                mock.patch.object(harness, "rpc", side_effect=query) as rpc:
            harness.assert_production(argparse.Namespace(state_dir="/owned", backend="sdk"))

        self.assertEqual(wait_rpc.call_count, 3)
        self.assertEqual(rpc.call_count, 6)

    def test_production_assertion_rejects_corrupt_matched_span_field(self) -> None:
        wait, query = _sdk_viewer_responses(span_status="Ok", distractor_first=True)
        with mock.patch.object(harness, "_owned", return_value=(123, {"host": "127.0.0.1", "ui": 8000})), \
                mock.patch.object(harness, "_wait_rpc", side_effect=wait), \
                mock.patch.object(harness, "rpc", side_effect=query):
            with self.assertRaisesRegex(harness.HarnessError, "statusCode"):
                harness.assert_production(argparse.Namespace(state_dir="/owned", backend="sdk"))

    def test_production_assertion_rejects_matched_span_link_count_mismatch(self) -> None:
        wait, query = _sdk_viewer_responses()

        def mismatched_links(*args: object) -> object:
            response = wait(*args)
            if args[1] == "searchSpans":
                response["spans"][0]["spanData"]["links"] = []
            return response

        with mock.patch.object(harness, "_owned", return_value=(123, {"host": "127.0.0.1", "ui": 8000})), \
                mock.patch.object(harness, "_wait_rpc", side_effect=mismatched_links), \
                mock.patch.object(harness, "rpc", side_effect=query):
            with self.assertRaisesRegex(harness.HarnessError, "links did not match"):
                harness.assert_production(argparse.Namespace(state_dir="/owned", backend="sdk"))

    def test_production_assertion_rejects_corrupt_matched_log_resource(self) -> None:
        wait, query = _sdk_viewer_responses(log_service="wrong-service")
        with mock.patch.object(harness, "_owned", return_value=(123, {"host": "127.0.0.1", "ui": 8000})), \
                mock.patch.object(harness, "_wait_rpc", side_effect=wait), \
                mock.patch.object(harness, "rpc", side_effect=query):
            with self.assertRaisesRegex(harness.HarnessError, "production log resource"):
                harness.assert_production(argparse.Namespace(state_dir="/owned", backend="sdk"))

    def test_start_occupied_port_writes_no_state_and_does_not_signal_listener(self) -> None:
        with tempfile.TemporaryDirectory() as temp, socket.socket() as listener:
            root = Path(temp)
            binary = root / "viewer"
            binary.write_text("#!/bin/sh\nexit 0\n")
            binary.chmod(0o755)
            listener.bind(("127.0.0.1", 0))
            listener.listen()
            host, occupied_port = listener.getsockname()
            args = argparse.Namespace(binary=str(binary), state_dir=str(root / "state"),
                                      binary_sha256=hashlib.sha256(binary.read_bytes()).hexdigest(),
                                      version=None, host=host, http=occupied_port,
                                      grpc=44317, ui=48000)
            with mock.patch.object(harness.os, "kill") as kill:
                with self.assertRaisesRegex(harness.HarnessError, "ports are occupied"):
                    harness.start(args)
                kill.assert_not_called()
            self.assertFalse((root / "state").exists())

    def test_refuses_pid_whose_command_line_only_contains_database_substring(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            state = Path(temp)
            database = state / "viewer.duckdb"
            proc = subprocess.Popen([sys.executable, "-c",
                                     f"import time; marker={str(database)!r}; time.sleep(30)"])
            try:
                _record(state, proc, database)
                with self.assertRaisesRegex(harness.HarnessError, "process identity"):
                    harness._owned(state)
                self.assertIsNone(proc.poll())
            finally:
                proc.terminate()
                proc.wait(timeout=5)

    def test_refuses_pid_when_pid_file_and_metadata_disagree(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            state = Path(temp)
            database = state / "viewer.duckdb"
            proc = _owned_process(database)
            try:
                _record(state, proc, database)
                metadata = json.loads((state / "viewer.json").read_text())
                metadata["pid"] = proc.pid + 1
                (state / "viewer.json").write_text(json.dumps(metadata))
                with self.assertRaisesRegex(harness.HarnessError, "pid file and metadata disagree"):
                    harness._owned(state)
            finally:
                proc.terminate()
                proc.wait(timeout=5)

    def test_hash_mismatch_prevents_process_start(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            binary = root / "viewer"
            binary.write_text("#!/bin/sh\nexit 0\n")
            binary.chmod(0o755)
            args = argparse.Namespace(binary=str(binary), state_dir=str(root / "state"),
                                      binary_sha256="0" * 64, version=None, host="127.0.0.1",
                                      http=44318, grpc=44317, ui=48000)
            with self.assertRaisesRegex(harness.HarnessError, "SHA-256 mismatch"):
                harness.start(args)
            self.assertFalse((root / "state" / "viewer.pid").exists())

    def test_start_cleans_up_process_and_files_after_keyboard_interrupt(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            binary = root / "viewer"
            binary.write_text("#!/usr/bin/env python3\nimport time\ntime.sleep(30)\n")
            binary.chmod(0o755)
            args = argparse.Namespace(binary=str(binary), state_dir=str(root / "state"),
                                      binary_sha256=hashlib.sha256(binary.read_bytes()).hexdigest(),
                                      version=None, host="127.0.0.1", http=44318,
                                      grpc=44317, ui=48000)
            with mock.patch.object(harness, "_request", side_effect=KeyboardInterrupt):
                with self.assertRaises(KeyboardInterrupt):
                    harness.start(args)
            state = root / "state"
            self.assertFalse((state / "viewer.pid").exists())
            self.assertFalse((state / "viewer.json").exists())
            self.assertFalse((state / "viewer.log").exists())
            self.assertFalse(state.exists(), "failed start must remove its newly created state directory")

    def test_failed_start_preserves_preexisting_empty_state_directory(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            state = root / "state"
            state.mkdir()
            binary = root / "viewer"
            binary.write_text("#!/usr/bin/env python3\nimport time\ntime.sleep(30)\n")
            binary.chmod(0o755)
            args = argparse.Namespace(binary=str(binary), state_dir=str(state),
                                      binary_sha256=hashlib.sha256(binary.read_bytes()).hexdigest(),
                                      version=None, host="127.0.0.1", http=44318,
                                      grpc=44317, ui=48000)
            with mock.patch.object(harness, "_request", side_effect=KeyboardInterrupt):
                with self.assertRaises(KeyboardInterrupt):
                    harness.start(args)
            self.assertTrue(state.is_dir())
            self.assertEqual(list(state.iterdir()), [])

    def test_stop_escalates_after_timeout_and_removes_owned_wal(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            state = Path(temp) / "state"
            state.mkdir()
            database = state / "viewer.duckdb"
            database.write_text("owned")
            Path(str(database) + ".wal").write_text("owned wal")
            (state / "viewer.log").write_text("owned")
            proc = _owned_process(database, ignore_term=True)
            try:
                _record(state, proc, database)
                harness.stop(argparse.Namespace(state_dir=str(state), timeout=0.1,
                                                remove_state=True))
                proc.wait(timeout=5)
                self.assertFalse(state.exists())
            finally:
                if proc.poll() is None:
                    proc.kill()
                    proc.wait(timeout=5)

    def test_stop_cleans_only_owned_database_when_requested(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            state = Path(temp) / "state"
            state.mkdir()
            database = state / "viewer.duckdb"
            database.write_text("owned")
            (state / "viewer.log").write_text("owned")
            proc = _owned_process(database)
            try:
                _record(state, proc, database)
                harness.stop(argparse.Namespace(state_dir=str(state), timeout=5,
                                                remove_state=True))
                proc.wait(timeout=5)
                self.assertFalse(database.exists())
                self.assertFalse(state.exists())
            finally:
                if proc.poll() is None:
                    proc.terminate()
                    proc.wait(timeout=5)

    def test_stop_process_lookup_removes_all_owned_state_when_requested(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            state = Path(temp) / "state"
            state.mkdir()
            database = state / "viewer.duckdb"
            database.write_text("owned")
            Path(str(database) + ".wal").write_text("owned wal")
            (state / "viewer.log").write_text("owned")
            (state / "viewer.pid").write_text("123\n")
            (state / "viewer.json").write_text("{}")
            metadata = {"pid": 123, "database": str(database)}
            with mock.patch.object(harness, "_owned", return_value=(123, metadata)), \
                    mock.patch.object(harness, "_owned_database", return_value=database), \
                    mock.patch.object(harness, "_command_args") as command_args, \
                    mock.patch.object(harness.os, "kill", side_effect=ProcessLookupError):
                harness.stop(argparse.Namespace(state_dir=str(state), timeout=5,
                                                remove_state=True))
            command_args.assert_not_called()
            self.assertFalse(state.exists())

    def test_stop_process_lookup_preserves_state_without_remove_state(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            state = Path(temp) / "state"
            state.mkdir()
            database = state / "viewer.duckdb"
            database.write_text("owned")
            wal = Path(str(database) + ".wal")
            wal.write_text("owned wal")
            log = state / "viewer.log"
            log.write_text("owned")
            (state / "viewer.pid").write_text("123\n")
            (state / "viewer.json").write_text("{}")
            metadata = {"pid": 123, "database": str(database)}
            with mock.patch.object(harness, "_owned", return_value=(123, metadata)), \
                    mock.patch.object(harness, "_command_args") as command_args, \
                    mock.patch.object(harness.os, "kill", side_effect=ProcessLookupError):
                harness.stop(argparse.Namespace(state_dir=str(state), timeout=5,
                                                remove_state=False))
            command_args.assert_not_called()
            self.assertFalse((state / "viewer.pid").exists())
            self.assertFalse((state / "viewer.json").exists())
            self.assertTrue(database.exists())
            self.assertTrue(wal.exists())
            self.assertTrue(log.exists())
            self.assertTrue(state.is_dir())

    def test_stop_permission_error_preserves_all_owned_state(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            state = Path(temp) / "state"
            state.mkdir()
            database = state / "viewer.duckdb"
            database.write_text("owned")
            wal = Path(str(database) + ".wal")
            wal.write_text("owned wal")
            log = state / "viewer.log"
            log.write_text("owned")
            (state / "viewer.pid").write_text("123\n")
            (state / "viewer.json").write_text("{}")
            metadata = {"pid": 123, "database": str(database)}
            with mock.patch.object(harness, "_owned", return_value=(123, metadata)), \
                    mock.patch.object(harness, "_owned_database", return_value=database), \
                    mock.patch.object(harness.os, "kill", side_effect=PermissionError):
                with self.assertRaises(PermissionError):
                    harness.stop(argparse.Namespace(state_dir=str(state), timeout=5,
                                                    remove_state=True))
            self.assertTrue((state / "viewer.pid").exists())
            self.assertTrue((state / "viewer.json").exists())
            self.assertTrue(database.exists())
            self.assertTrue(wal.exists())
            self.assertTrue(log.exists())
            self.assertTrue(state.is_dir())

    def test_stop_refuses_database_recorded_outside_state_directory(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            state = root / "state"
            state.mkdir()
            outside_database = root / "user-data.duckdb"
            outside_database.write_text("keep")
            proc = _owned_process(outside_database)
            try:
                _record(state, proc, outside_database)
                with self.assertRaisesRegex(harness.HarnessError, "refusing cleanup"):
                    harness.stop(argparse.Namespace(state_dir=str(state), timeout=5,
                                                    remove_state=True))
                self.assertIsNone(proc.poll(), "refusal must happen before signalling the recorded PID")
                self.assertEqual(outside_database.read_text(), "keep")
                self.assertTrue((state / "viewer.pid").exists())
            finally:
                if proc.poll() is None:
                    proc.terminate()
                    proc.wait(timeout=5)

    def test_ci_always_stops_an_instance_after_probe_failure(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            state = Path(temp) / "state"
            args = argparse.Namespace(binary="/unused", binary_sha256="0" * 64,
                                      version="0.5.0", state_dir=str(state),
                                      host="127.0.0.1", http=4318, grpc=4317,
                                      ui=8000, run_id="test")
            with mock.patch.object(harness, "start") as start_mock, \
                    mock.patch.object(harness, "status") as status_mock, \
                    mock.patch.object(harness, "probe", side_effect=RuntimeError("probe failed")), \
                    mock.patch.object(harness, "stop") as stop_mock:
                def start_and_record(_args: argparse.Namespace) -> None:
                    state.mkdir()
                    (state / "viewer.pid").write_text("123\n")

                start_mock.side_effect = start_and_record
                with self.assertRaisesRegex(RuntimeError, "probe failed"):
                    harness.run_ci(args)
                status_mock.assert_called_once()
                stop_mock.assert_called_once()


if __name__ == "__main__":
    unittest.main()
