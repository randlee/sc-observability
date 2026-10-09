"""Regression coverage for the e2e subprocess boundary."""
from __future__ import annotations

import json
from pathlib import Path
import subprocess
import sys

import pytest

import test_viewer_readback
from conftest import (
    PinnedViewer, ROOT, VIEWER_HARNESS, VIEWER_HARNESS_WORST_CASE_SECONDS, VIEWER_TIMEOUT_SECONDS,
    _harness_ready_seconds, _viewer_start_command, owned_viewer, run_process,
)


def test_subprocess_timeout_reports_the_command_and_captured_output(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    def stall(command: list[str], **_kwargs: object) -> subprocess.CompletedProcess[str]:
        raise subprocess.TimeoutExpired(command, 1, output="partial stdout", stderr="partial stderr")

    monkeypatch.setattr(subprocess, "run", stall)
    with pytest.raises(pytest.fail.Exception, match="subprocess timed out after 1s: \\['stalled'\\]") as failure:
        run_process(["stalled"], timeout=1, text=True, capture_output=True)
    assert "partial stdout" in str(failure.value)
    assert "partial stderr" in str(failure.value)


def test_real_release_manifest_builds_host_specific_harness_command(tmp_path: Path) -> None:
    manifest = json.loads((ROOT / "scripts/ci/fixtures/otlp/desktop-viewer/release.json").read_text())
    entry = manifest["platforms"]["linux_amd64"]
    command = _viewer_start_command(
        "installed-viewer", manifest["version"], entry["binary_sha256"], tmp_path / "state",
        4317, 4318, 4320, reuse_state=True,
    )
    assert command[command.index("--version") + 1] == manifest["version"]
    assert command[command.index("--binary-sha256") + 1] == entry["binary_sha256"]
    assert command[-1] == "--reuse-state"


def test_viewer_restart_uses_canonical_harness_stop_and_start_headlessly(
    monkeypatch: pytest.MonkeyPatch, tmp_path: Path,
) -> None:
    state = tmp_path / "viewer-state"
    calls: list[list[str]] = []
    ports = iter([(41000, 41001, 41002), (42000, 42001, 42002)])

    def record(command: list[str], **_kwargs: object) -> subprocess.CompletedProcess[str]:
        calls.append(command)
        if "start" not in command:
            return subprocess.CompletedProcess(command, 0, "", "")
        http, grpc, ui = next(ports)
        ready = {"status": "ready", "http": http, "grpc": grpc, "ui": ui}
        return subprocess.CompletedProcess(command, 0, json.dumps(ready) + "\n", "")

    monkeypatch.setattr("conftest.run_process", record)
    viewer = PinnedViewer("viewer", "0.5.0", "a" * 64, state)
    viewer.start()
    assert (viewer.http, viewer.grpc, viewer.ui) == (41000, 41001, 41002)
    assert viewer["otlp"] == "http://127.0.0.1:41000"
    viewer.restart()

    expected_harness = ROOT / "scripts/ci/fixtures/otlp/desktop-viewer/viewer_harness.py"
    assert VIEWER_HARNESS == expected_harness
    # Every start asks the viewer to bind ephemeral ports and publishes what it reports.
    assert calls == [
        _viewer_start_command("viewer", "0.5.0", "a" * 64, state, 0, 0, 0, reuse_state=False),
        [sys.executable, str(expected_harness), "stop", "--state-dir", str(state)],
        _viewer_start_command("viewer", "0.5.0", "a" * 64, state, 0, 0, 0, reuse_state=True),
    ]
    assert (viewer.http, viewer.grpc, viewer.ui) == (42000, 42001, 42002)
    assert viewer["otlp"] == "http://127.0.0.1:42000"
    assert viewer["rpc"] == "http://127.0.0.1:42002/rpc"


def test_outer_viewer_timeout_outlasts_the_harness_worst_case() -> None:
    ready = _harness_ready_seconds()
    # hash pass + --version (10 s) + readiness + terminate and kill waits (5 s each)
    assert VIEWER_HARNESS_WORST_CASE_SECONDS >= 30 + 10 + ready + 5 + 5
    assert VIEWER_TIMEOUT_SECONDS > VIEWER_HARNESS_WORST_CASE_SECONDS


def test_failed_viewer_start_is_still_stopped_by_state_directory(
    monkeypatch: pytest.MonkeyPatch, tmp_path: Path,
) -> None:
    state = tmp_path / "viewer-state"
    calls: list[list[str]] = []

    def record(command: list[str], **_kwargs: object) -> subprocess.CompletedProcess[str]:
        calls.append(command)
        if "start" in command:
            state.mkdir()  # the harness created its state before the start failed
            return subprocess.CompletedProcess(command, 1, "", "viewer did not become ready")
        return subprocess.CompletedProcess(command, 0, "", "")

    monkeypatch.setattr("conftest.run_process", record)
    viewer = PinnedViewer("viewer", "0.5.0", "a" * 64, state)
    with pytest.raises(AssertionError, match="viewer did not become ready"):
        with owned_viewer(viewer):
            pytest.fail("the body must not run after a failed start")
    assert "stop" in calls[-1] and str(state) in calls[-1]


def _scripted_rpc(monkeypatch: pytest.MonkeyPatch, responses: list[dict[str, object]]) -> list[str]:
    calls: list[str] = []

    def respond(_url: str, method: str, _params: list[object]) -> dict[str, object]:
        calls.append(method)
        return responses.pop(0)

    monkeypatch.setattr(test_viewer_readback, "rpc_response", respond)
    monkeypatch.setattr(test_viewer_readback.time, "sleep", lambda _seconds: None)
    return calls


def test_wait_for_keeps_polling_while_the_trace_is_not_ingested(monkeypatch: pytest.MonkeyPatch) -> None:
    not_found = {"id": "d32", "error": {"code": -32001, "message": "Trace not found"}}
    calls = _scripted_rpc(monkeypatch, [not_found, not_found, {"id": "d32", "result": {"spans": [1]}}])
    result = test_viewer_readback._wait_for(
        {"rpc": "http://viewer/rpc"}, "searchSpans", ["trace"], lambda value: bool(value["spans"]),
    )
    assert result == {"spans": [1]}
    assert calls == ["searchSpans"] * 3


def test_wait_for_fails_at_once_on_any_other_rpc_error(monkeypatch: pytest.MonkeyPatch) -> None:
    other = {"id": "d32", "error": {"code": -32602, "message": "Invalid params"}}
    calls = _scripted_rpc(monkeypatch, [other, {"id": "d32", "result": {}}])
    with pytest.raises(AssertionError, match="Invalid params"):
        test_viewer_readback._wait_for({"rpc": "http://viewer/rpc"}, "searchSpans", ["trace"], bool)
    assert calls == ["searchSpans"]
    # the same code from a different method is not an ingestion wait either
    not_found = {"id": "d32", "error": {"code": -32001, "message": "Trace not found"}}
    calls = _scripted_rpc(monkeypatch, [not_found, {"id": "d32", "result": {}}])
    with pytest.raises(AssertionError, match="Trace not found"):
        test_viewer_readback._wait_for({"rpc": "http://viewer/rpc"}, "getLog", ["id"], bool)
    assert calls == ["getLog"]
