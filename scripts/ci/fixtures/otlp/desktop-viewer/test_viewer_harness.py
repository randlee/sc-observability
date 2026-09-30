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
            *, binary: str = sys.executable) -> None:
    (state / "viewer.pid").write_text(f"{proc.pid}\n")
    (state / "viewer.json").write_text(json.dumps({
        "pid": proc.pid, "binary": binary, "database": str(database)}))


def _owned_process(database: Path, *, ignore_term: bool = False) -> subprocess.Popen[bytes]:
    code = "import time; time.sleep(30)"
    if ignore_term:
        code = "import signal,time; signal.signal(signal.SIGTERM, signal.SIG_IGN); time.sleep(30)"
    return subprocess.Popen([sys.executable, "-c", code, "--db", str(database)])


class ViewerHarnessSafetyTests(unittest.TestCase):
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
            self.assertEqual(list((root / "state").iterdir()), [])

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
