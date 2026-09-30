"""Safety regressions for the isolated desktop viewer lifecycle."""

from __future__ import annotations

import argparse
import json
import os
import socket
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from unittest import mock

import viewer_harness as harness


class ViewerHarnessSafetyTests(unittest.TestCase):
    def test_occupied_port_is_reported_without_signalling_listener(self) -> None:
        with socket.socket() as listener:
            listener.bind(("127.0.0.1", 0))
            listener.listen()
            host, port = listener.getsockname()
            self.assertFalse(harness._port_available(host, port))
            self.assertEqual(listener.fileno() >= 0, True)

    def test_refuses_pid_whose_command_line_does_not_own_recorded_database(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            state = Path(temp)
            proc = subprocess.Popen([sys.executable, "-c", "import time; time.sleep(30)"])
            try:
                (state / "viewer.pid").write_text(f"{proc.pid}\n")
                (state / "viewer.json").write_text(json.dumps({"database": str(state / "db.duckdb")}))
                with self.assertRaisesRegex(harness.HarnessError, "refusing to signal"):
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

    def test_stop_cleans_only_owned_database_when_requested(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            state = Path(temp) / "state"
            state.mkdir()
            database = state / "viewer.duckdb"
            database.write_text("owned")
            log = state / "viewer.log"
            log.write_text("owned")
            proc = subprocess.Popen([sys.executable, "-c", "import time; time.sleep(30)",
                                     str(database)])
            try:
                (state / "viewer.pid").write_text(f"{proc.pid}\n")
                (state / "viewer.json").write_text(json.dumps({"database": str(database)}))
                harness.stop(argparse.Namespace(state_dir=str(state), timeout=5,
                                                remove_state=True))
                proc.wait(timeout=5)
                self.assertFalse(database.exists())
                self.assertFalse(log.exists())
                self.assertFalse(state.exists())
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
