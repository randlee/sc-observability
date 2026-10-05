"""Regression coverage for the e2e subprocess boundary."""
from __future__ import annotations

import json
from pathlib import Path
import subprocess

import pytest

from conftest import ROOT, _viewer_start_command, run_process


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
