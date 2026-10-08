"""Safety regressions for the isolated desktop viewer lifecycle."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import socket
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from types import SimpleNamespace
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


def _fake_binary(directory: Path, *, body: str) -> Path:
    """Write a headless fake executable for lifecycle tests."""
    if sys.platform == "win32":
        # Tests that need a live process use _owned_process, which invokes the
        # Python executable directly. Returning that executable here avoids a
        # cmd/ping process tree that can retain viewer.log after cleanup.
        return Path(sys.executable)
    binary = directory / "viewer"
    binary.write_text({"exit": "#!/bin/sh\nexit 0\n",
                       "sleep": "#!/usr/bin/env python3\nimport time\ntime.sleep(30)\n"}[body])
    binary.chmod(0o755)
    return binary


def _owned_process(database: Path, *, ignore_term: bool = False) -> subprocess.Popen[bytes]:
    if sys.platform == "win32":
        return subprocess.Popen([sys.executable, "-c", "import time; time.sleep(30)",
                                 "--db", str(database)])
    code = "trap - TERM; sleep 30"
    if ignore_term:
        code = "trap '' TERM; sleep 30"
    # Use a stable direct executable rather than macOS's Python launcher; the
    # test records the actual command identity that the safety guard verifies.
    return subprocess.Popen(["/bin/sh", "-c", code, "--db", str(database)])


class ViewerHarnessSafetyTests(unittest.TestCase):
    def test_start_occupied_port_writes_no_state_and_does_not_signal_listener(self) -> None:
        with tempfile.TemporaryDirectory() as temp, socket.socket() as listener:
            root = Path(temp)
            binary = _fake_binary(root, body="exit")
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

    @unittest.skipUnless(sys.platform == "win32",
                         "Windows fixtures must not spawn a cmd/ping process tree")
    def test_windows_fake_binary_uses_the_direct_python_executable(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            self.assertEqual(_fake_binary(Path(temp), body="sleep"), Path(sys.executable))

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

    def test_refuses_recorded_binary_that_differs_from_live_process(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            state = Path(temp)
            database = state / "viewer.duckdb"
            proc = _owned_process(database)
            try:
                command = harness._command_args(proc.pid)
                self.assertIsNotNone(command)
                assert command is not None
                foreign_binary = Path(command[0]).with_name("foreign-viewer")
                _record(state, proc, database, binary=str(foreign_binary))
                with self.assertRaisesRegex(harness.HarnessError, "process identity"):
                    harness._owned(state)
                self.assertIsNone(proc.poll(), "foreign-process refusal must not terminate it")
            finally:
                if proc.poll() is None:
                    proc.terminate()
                    proc.wait(timeout=5)

    def test_windows_identity_uses_recorded_executable_and_database(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            state = Path(temp)
            database = state / "viewer.duckdb"
            binary = Path(sys.executable).resolve()
            (state / "viewer.pid").write_text("123\n")
            (state / "viewer.json").write_text(json.dumps({
                "pid": 123, "binary": str(binary), "database": str(database)}))
            command = [str(binary), "-c", "import time", "--db", str(database)]
            with mock.patch.object(harness, "_is_windows", return_value=True), \
                    mock.patch.object(harness, "_command_args_win", return_value=command):
                pid, _ = harness._owned(state)
            self.assertEqual(pid, 123)

    @unittest.skipUnless(sys.platform == "win32",
                         "requires the native Windows command-line process API")
    def test_windows_live_identity_refuses_foreign_process(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            state = Path(temp)
            database = state / "viewer.duckdb"
            proc = _owned_process(database)
            try:
                _record(state, proc, database)
                self.assertEqual(harness._command_args_win(proc.pid),
                                 harness._command_args(proc.pid))
                metadata = json.loads((state / "viewer.json").read_text())
                metadata["binary"] = str(Path(sys.executable).with_name("foreign-viewer.exe"))
                (state / "viewer.json").write_text(json.dumps(metadata))
                with self.assertRaisesRegex(harness.HarnessError, "process identity"):
                    harness._owned(state)
                self.assertIsNone(proc.poll(), "foreign-process refusal must not terminate it")
            finally:
                if proc.poll() is None:
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
            binary = _fake_binary(root, body="exit")
            args = argparse.Namespace(binary=str(binary), state_dir=str(root / "state"),
                                      binary_sha256="0" * 64, version=None, host="127.0.0.1",
                                      http=44318, grpc=44317, ui=48000)
            with self.assertRaisesRegex(harness.HarnessError, "SHA-256 mismatch"):
                harness.start(args)
            self.assertFalse((root / "state" / "viewer.pid").exists())

    def test_start_cleans_up_process_and_files_after_keyboard_interrupt(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            binary = _fake_binary(root, body="exit")
            args = argparse.Namespace(binary=str(binary), state_dir=str(root / "state"),
                                      binary_sha256=hashlib.sha256(binary.read_bytes()).hexdigest(),
                                      version=None, host="127.0.0.1", http=44318,
                                      grpc=44317, ui=48000)
            process = mock.Mock(pid=999)
            process.poll.return_value = None
            with mock.patch.object(harness.subprocess, "Popen", return_value=process), \
                    mock.patch.object(harness, "_request", side_effect=KeyboardInterrupt):
                with self.assertRaises(KeyboardInterrupt):
                    harness.start(args)
            process.terminate.assert_called_once()
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
            binary = _fake_binary(root, body="exit")
            args = argparse.Namespace(binary=str(binary), state_dir=str(state),
                                      binary_sha256=hashlib.sha256(binary.read_bytes()).hexdigest(),
                                      version=None, host="127.0.0.1", http=44318,
                                      grpc=44317, ui=48000)
            process = mock.Mock(pid=999)
            process.poll.return_value = None
            with mock.patch.object(harness.subprocess, "Popen", return_value=process), \
                    mock.patch.object(harness, "_request", side_effect=KeyboardInterrupt):
                with self.assertRaises(KeyboardInterrupt):
                    harness.start(args)
            process.terminate.assert_called_once()
            self.assertTrue(state.is_dir())
            self.assertEqual(list(state.iterdir()), [])

    def test_failed_start_closes_log_before_removing_it(self) -> None:
        class Log:
            closed = False

            def close(self) -> None:
                self.closed = True

        class Process:
            pid = 999

            def poll(self) -> None:
                return None

            def terminate(self) -> None:
                return None

            def wait(self, *, timeout: float) -> None:
                return None

            def kill(self) -> None:
                return None

        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            binary = _fake_binary(root, body="exit")
            args = argparse.Namespace(binary=str(binary), state_dir=str(root / "state"),
                                      binary_sha256=hashlib.sha256(binary.read_bytes()).hexdigest(),
                                      version=None, host="127.0.0.1", http=44318,
                                      grpc=44317, ui=48000)
            log = Log()
            real_open = Path.open
            real_unlink = Path.unlink

            def open_path(path: Path, *args: object, **kwargs: object) -> object:
                return log if path.name == "viewer.log" else real_open(path, *args, **kwargs)

            def unlink_path(path: Path, *args: object, **kwargs: object) -> None:
                if path.name == "viewer.log":
                    self.assertTrue(log.closed, "log must close before Windows unlink")
                real_unlink(path, *args, **kwargs)

            with mock.patch.object(Path, "open", new=open_path), \
                    mock.patch.object(Path, "unlink", new=unlink_path), \
                    mock.patch.object(harness.subprocess, "Popen", return_value=Process()), \
                    mock.patch.object(harness, "_request", side_effect=KeyboardInterrupt):
                with self.assertRaises(KeyboardInterrupt):
                    harness.start(args)

    @unittest.skipIf(sys.platform == "win32",
                     "Windows terminates an owned process in one native stage")
    def test_stop_escalates_after_timeout_and_removes_owned_wal_on_posix(self) -> None:
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
                posix_signal = SimpleNamespace(SIGTERM=15, SIGKILL=9)
                with mock.patch.object(harness, "signal", posix_signal):
                    harness.stop(argparse.Namespace(state_dir=str(state), timeout=0.1,
                                                    remove_state=True))
                proc.wait(timeout=5)
                self.assertFalse(state.exists())
            finally:
                if proc.poll() is None:
                    os.kill(proc.pid, 9)
                    proc.wait(timeout=5)

    @unittest.skipUnless(sys.platform == "win32",
                         "Windows has one native owned-process termination stage")
    def test_windows_stop_uses_one_termination_stage_and_removes_owned_wal(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            state = Path(temp) / "state"
            state.mkdir()
            database = state / "viewer.duckdb"
            database.write_text("owned")
            Path(str(database) + ".wal").write_text("owned wal")
            (state / "viewer.log").write_text("owned")
            proc = _owned_process(database)
            try:
                _record(state, proc, database)
                with mock.patch.object(harness, "_terminate", wraps=harness._terminate) as terminate:
                    harness.stop(argparse.Namespace(state_dir=str(state), timeout=5,
                                                    remove_state=True))
                proc.wait(timeout=5)
                terminate.assert_called_once_with(proc.pid, force=False)
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
            marker = json.loads((state / harness.RESTART_MARKER).read_text())
            self.assertEqual(marker, metadata)
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
                    mock.patch.object(harness, "_command_args", return_value=["viewer"]), \
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

    @unittest.skipUnless(sys.platform == "win32",
                         "covers the Windows dead-PID PermissionError branch")
    def test_windows_dead_pid_permission_error_cleans_owned_state(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            state = Path(temp) / "state"
            state.mkdir()
            database = state / "viewer.duckdb"
            database.write_text("owned")
            Path(str(database) + ".wal").write_text("owned wal")
            (state / "viewer.log").write_text("owned")
            metadata = {"pid": 123, "database": str(database)}
            with mock.patch.object(harness, "_owned", return_value=(123, metadata)), \
                    mock.patch.object(harness, "_owned_database", return_value=database), \
                    mock.patch.object(harness, "_terminate", side_effect=PermissionError), \
                    mock.patch.object(harness, "_command_args", return_value=None):
                harness.stop(argparse.Namespace(state_dir=str(state), timeout=5,
                                                remove_state=True))
            self.assertFalse(state.exists())

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
