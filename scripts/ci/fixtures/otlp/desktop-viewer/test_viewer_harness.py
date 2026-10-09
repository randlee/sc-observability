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


LOG = (
    '2026-10-08T20:22:31.625-0700\tinfo\totlpreceiver@v0.159.0/otlp.go:120\tStarting GRPC server\t'
    '{"resource": {"service.name": "otel-desktop-viewer"}, "endpoint": "127.0.0.1:55188"}\n'
    '2026-10-08T20:22:31.625-0700\tinfo\totlpreceiver@v0.159.0/otlp.go:175\tStarting HTTP server\t'
    '{"resource": {"service.name": "otel-desktop-viewer"}, "endpoint": "127.0.0.1:55189"}\n'
)


class EphemeralPortHandshakeTests(unittest.TestCase):
    def test_logged_endpoints_name_the_grpc_and_http_ports(self) -> None:
        self.assertEqual(harness._logged_endpoints(LOG), {"grpc": 55188, "http": 55189})
        self.assertEqual(harness._logged_endpoints("no endpoints yet\n"), {})

    def test_socket_table_parsers_read_only_listening_rows(self) -> None:
        proc = ("  sl  local_address rem_address   st tx_queue rx_queue tr tm->when retrnsmt   uid  timeout inode\n"
                "   0: 0100007F:D993 00000000:0000 0A 00000000:00000000 00:00000000 00000000   501        0 111 1\n"
                "   1: 0100007F:D994 0100007F:0050 01 00000000:00000000 00:00000000 00000000   501        0 222 1\n")
        self.assertEqual(harness._parse_proc_net_tcp(proc), {"111": 0xD993})
        self.assertEqual(harness._parse_lsof_ports("p123\nf4\nn127.0.0.1:55187\nf5\nn[::1]:55188\nn*:55189\n"),
                         {55187, 55188, 55189})
        netstat = ("  Proto  Local Address          Foreign Address        State           PID\n"
                   "  TCP    127.0.0.1:55187        0.0.0.0:0              LISTENING       4242\n"
                   "  TCP    127.0.0.1:55188        0.0.0.0:0              LISTENING       999\n"
                   "  TCP    127.0.0.1:55189        127.0.0.1:50000        ESTABLISHED     4242\n")
        self.assertEqual(harness._parse_netstat_ports(netstat, 4242), {55187})

    def test_bound_ports_resolve_ephemeral_requests_from_log_and_socket_table(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            log = Path(temp) / "viewer.log"
            requested = {"http": 0, "grpc": 0, "ui": 0}
            self.assertIsNone(harness._bound_ports(1, log, requested), "no log yet")
            log.write_text("starting\n")
            self.assertIsNone(harness._bound_ports(1, log, requested), "endpoints not logged yet")
            log.write_text(LOG)
            with mock.patch.object(harness, "_listening_ports", return_value={55187, 55188, 55189}):
                self.assertEqual(harness._bound_ports(1, log, requested),
                                 {"http": 55189, "grpc": 55188, "ui": 55187})
            with mock.patch.object(harness, "_listening_ports", return_value={55188, 55189}):
                self.assertIsNone(harness._bound_ports(1, log, requested), "browser port not listening yet")
            with mock.patch.object(harness, "_listening_ports", return_value={55186, 55187, 55188, 55189}):
                self.assertIsNone(harness._bound_ports(1, log, requested), "ambiguous browser port")

    def test_explicit_ports_never_consult_the_process(self) -> None:
        requested = {"http": 4318, "grpc": 4317, "ui": 8000}
        with mock.patch.object(harness, "_listening_ports", side_effect=AssertionError("consulted")):
            self.assertEqual(harness._bound_ports(1, Path("missing.log"), requested), requested)

    def test_start_reports_the_ports_the_viewer_bound(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            binary = _fake_binary(root, body="exit")
            args = argparse.Namespace(binary=str(binary), state_dir=str(root / "state"),
                                      binary_sha256=hashlib.sha256(binary.read_bytes()).hexdigest(),
                                      version=None, host="127.0.0.1", http=0, grpc=0, ui=0)
            process = mock.Mock(pid=999)
            process.poll.return_value = None

            def launch(command: list[str], **kwargs: object) -> mock.Mock:
                kwargs["stdout"].write(LOG.encode())
                kwargs["stdout"].flush()
                self.assertEqual([command[i + 1] for i, v in enumerate(command)
                                  if v in ("--http", "--grpc", "--browser-port")], ["0", "0", "0"])
                return process

            with mock.patch.object(harness.subprocess, "Popen", side_effect=launch), \
                    mock.patch.object(harness, "_listening_ports", return_value={55187, 55188, 55189}), \
                    mock.patch.object(harness, "_request", return_value=(200, b"")) as request, \
                    mock.patch.object(harness, "rpc", return_value=[]) as rpc, \
                    mock.patch("builtins.print") as printed:
                harness.start(args)
            ready = json.loads(printed.call_args.args[0])
            self.assertEqual((ready["status"], ready["http"], ready["grpc"], ready["ui"]),
                             ("ready", 55189, 55188, 55187))
            request.assert_called_with("http://127.0.0.1:55187/", timeout=1)
            self.assertEqual(rpc.call_args.args[0], "http://127.0.0.1:55187/")
            recorded = json.loads((root / "state" / "viewer.json").read_text())
            self.assertEqual((recorded["http"], recorded["grpc"], recorded["ui"]), (55189, 55188, 55187))

    def test_logged_port_zero_is_not_a_bound_port(self) -> None:
        zero = LOG.replace("127.0.0.1:55188", "127.0.0.1:0")
        self.assertEqual(harness._logged_endpoints(zero), {"http": 55189})

    def test_bound_ports_ignore_endpoints_logged_before_the_offset(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            log = Path(temp) / "viewer.log"
            requested = {"http": 0, "grpc": 0, "ui": 0}
            log.write_text(LOG)  # the previous run, retained across stop
            offset = log.stat().st_size
            with mock.patch.object(harness, "_listening_ports", return_value={55187, 55188, 55189}):
                self.assertIsNone(harness._bound_ports(1, log, requested, offset), "new run not logged yet")
            with log.open("a") as handle:
                handle.write(LOG.replace("55188", "56188").replace("55189", "56189"))
            with mock.patch.object(harness, "_listening_ports", return_value={56187, 56188, 56189}):
                self.assertEqual(harness._bound_ports(1, log, requested, offset),
                                 {"http": 56189, "grpc": 56188, "ui": 56187})

    def _start_args(self, root: Path) -> argparse.Namespace:
        binary = _fake_binary(root, body="exit")
        return argparse.Namespace(binary=str(binary), state_dir=str(root / "state"),
                                  binary_sha256=hashlib.sha256(binary.read_bytes()).hexdigest(),
                                  version=None, host="127.0.0.1", http=0, grpc=0, ui=0)

    def test_start_ignores_a_retained_log_from_the_previous_run(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            args = self._start_args(root)
            state = root / "state"
            state.mkdir()
            (state / "viewer.log").write_text(LOG)
            (state / "viewer.duckdb").write_bytes(b"")
            (state / harness.RESTART_MARKER).write_text(json.dumps({
                "binary": str(Path(args.binary).resolve()), "sha256": args.binary_sha256,
                "version": None, "database": str((state / "viewer.duckdb").resolve())}))
            args.reuse_state = True
            process = mock.Mock(pid=999)
            fresh = LOG.replace("55188", "56188").replace("55189", "56189")
            polls: list[int] = []

            def poll() -> None:
                polls.append(1)
                if len(polls) == 2:  # the new process logs only after the first readiness pass
                    with (state / "viewer.log").open("a") as handle:
                        handle.write(fresh)

            def listening(_pid: int) -> set[int]:
                return {56187, 56188, 56189} if len(polls) >= 2 else {55187, 55188, 55189}

            process.poll.side_effect = poll
            with mock.patch.object(harness.subprocess, "Popen", return_value=process), \
                    mock.patch.object(harness, "_listening_ports", side_effect=listening), \
                    mock.patch.object(harness, "_request", return_value=(200, b"")), \
                    mock.patch.object(harness, "rpc", return_value=[]), \
                    mock.patch("builtins.print") as printed:
                harness.start(args)
            ready = json.loads(printed.call_args.args[0])
            self.assertEqual((ready["http"], ready["grpc"], ready["ui"]), (56189, 56188, 56187))

    def test_start_fails_at_once_on_an_rpc_error_from_a_listening_viewer(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            args = self._start_args(root)
            process = mock.Mock(pid=999)
            process.poll.return_value = None

            def launch(command: list[str], **kwargs: object) -> mock.Mock:
                kwargs["stdout"].write(LOG.encode())
                kwargs["stdout"].flush()
                return process

            with mock.patch.object(harness.subprocess, "Popen", side_effect=launch), \
                    mock.patch.object(harness, "_listening_ports", return_value={55187, 55188, 55189}), \
                    mock.patch.object(harness, "_request", return_value=(200, b"")), \
                    mock.patch.object(harness, "rpc",
                                      side_effect=harness.HarnessError("RPC searchLogs failed: boom")) as rpc:
                with self.assertRaisesRegex(harness.HarnessError, "RPC searchLogs failed: boom"):
                    harness.start(args)
            rpc.assert_called_once()


if __name__ == "__main__":
    unittest.main()
