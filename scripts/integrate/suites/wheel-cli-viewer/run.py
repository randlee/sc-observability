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


def run(command: list[str], *, timeout: int, env: dict[str, str], log: Path) -> subprocess.CompletedProcess[str]:
    completed = subprocess.run(
        command, cwd=ROOT, env=env, text=True, capture_output=True, timeout=timeout, check=False,
    )
    with log.open("a", encoding="utf-8") as output:
        output.write("$ " + " ".join(command) + "\n")
        output.write(completed.stdout)
        output.write(completed.stderr)
        output.write(f"exit={completed.returncode}\n")
    return completed


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
    tested = run(
        [sys.executable, "-m", "pytest", "-q", str(TESTS), f"--junitxml={output / 'pytest.xml'}"],
        timeout=SUITE_TIMEOUT_SECONDS,
        env=env | {"TELEMETRY_E2E_VIEWER_BINARY": binary}, log=log,
    )
    return tested.returncode


if __name__ == "__main__":
    raise SystemExit(main())
