"""Regression coverage for the e2e subprocess boundary."""
from __future__ import annotations

import subprocess

import pytest

from conftest import run_process


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
