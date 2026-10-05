#!/usr/bin/env python3
"""Run the installed wheel/CLI viewer composition suite at one pinned SHA.

This entrypoint is invoked only by the integration workflow.  It deliberately
uses the checked-in downloader and the existing pytest suite, so setup or a
missing pinned viewer asset is a failed cell rather than a local skip.
"""

from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
import subprocess
import sys


ROOT = Path(__file__).resolve().parents[4]
DOWNLOADER = ROOT / "scripts/ci/fixtures/otlp/desktop-viewer/download_pinned_release.py"
TESTS = ROOT / "tests/telemetry-e2e"
INSTALL_TIMEOUT_SECONDS = 180
SUITE_TIMEOUT_SECONDS = 35 * 60
TIMEOUT_EXIT_CODE = 124
VIEWER_STOP_TIMEOUT_SECONDS = 30


def _text(value: str | bytes | None) -> str:
    if isinstance(value, bytes):
        return value.decode("utf-8", errors="replace")
    return value or ""


def _write_log(log: Path, command: list[str], stdout: str | bytes | None, stderr: str | bytes | None,
               marker: str) -> None:
    with log.open("a", encoding="utf-8") as output:
        output.write("$ " + " ".join(command) + "\n")
        output.write(_text(stdout))
        output.write(_text(stderr))
        output.write(marker + "\n")


def run(command: list[str], *, timeout: int, env: dict[str, str], log: Path) -> subprocess.CompletedProcess[str]:
    try:
        completed = subprocess.run(
            command, cwd=ROOT, env=env, text=True, capture_output=True, timeout=timeout, check=False,
        )
    except subprocess.TimeoutExpired as error:
        _write_log(log, command, error.output, error.stderr, f"timeout={timeout}s\nexit={TIMEOUT_EXIT_CODE}")
        return subprocess.CompletedProcess(command, TIMEOUT_EXIT_CODE, _text(error.output), _text(error.stderr))
    _write_log(log, command, completed.stdout, completed.stderr, f"exit={completed.returncode}")
    return completed


def cleanup_timed_out_viewers(pytest_state: Path, *, env: dict[str, str], log: Path) -> None:
    """Stop only viewer processes whose state directories belong to this run."""
    for state in pytest_state.glob("**/viewer-state"):
        if state.is_dir():
            run(
                [sys.executable, str(DOWNLOADER.parent / "viewer_harness.py"), "stop", "--state-dir", str(state)],
                timeout=VIEWER_STOP_TIMEOUT_SECONDS, env=env, log=log,
            )


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source-sha", required=True)
    parser.add_argument("--output-dir", required=True)
    args = parser.parse_args(argv)
    output = Path(args.output_dir)
    if not output.is_absolute():
        parser.error("output-dir must be absolute")
    output.mkdir(parents=True, exist_ok=True)
    log = output / "wheel-cli-viewer.log"
    env = os.environ | {"CI": "1", "GITHUB_ACTIONS": "true"}

    installed = run(
        [sys.executable, "-m", "pip", "install", "pytest==8.4.2", "pytest-timeout==2.4.0"],
        timeout=INSTALL_TIMEOUT_SECONDS, env=env, log=log,
    )
    if installed.returncode:
        return installed.returncode
    downloaded = run(
        [sys.executable, str(DOWNLOADER), str(output / "otel-desktop-viewer")],
        timeout=INSTALL_TIMEOUT_SECONDS, env=env, log=log,
    )
    if downloaded.returncode:
        return downloaded.returncode
    try:
        metadata = json.loads(downloaded.stdout.strip().splitlines()[-1])
        binary = metadata["binary"]
    except (IndexError, json.JSONDecodeError, KeyError) as error:
        log.write_text(log.read_text(encoding="utf-8") + f"invalid downloader receipt: {error}\n", encoding="utf-8")
        return 1
    pytest_state = output / "pytest-state"
    tested = run(
        [sys.executable, "-m", "pytest", "-q", str(TESTS), f"--basetemp={pytest_state}",
         f"--junitxml={output / 'pytest.xml'}"],
        timeout=SUITE_TIMEOUT_SECONDS,
        env=env | {
            "TELEMETRY_E2E_VIEWER_BINARY": binary,
            "TELEMETRY_E2E_VIEWER_BINARY_SHA256": metadata["binary_sha256"],
            "TELEMETRY_E2E_VIEWER_PLATFORM": metadata["platform"],
        }, log=log,
    )
    if tested.returncode == TIMEOUT_EXIT_CODE:
        cleanup_timed_out_viewers(pytest_state, env=env, log=log)
    return tested.returncode


if __name__ == "__main__":
    raise SystemExit(main())
