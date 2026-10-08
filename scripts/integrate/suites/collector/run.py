#!/usr/bin/env python3
"""Run the official-Collector end-to-end qualification for one candidate SHA.

The runner downloads the Collector pinned in ``tests/telemetry-e2e/collector-release.json``
(archive and binary SHA-256 verified), then runs the native, installed-frontend and
harness pytest files with ``TELEMETRY_E2E_COLLECTOR_BINARY`` set.  The tests start
the Collector, drive the real artifacts and read its exported files back.  Each
case keeps its own receipt and later cases run after an earlier one fails.
"""

from __future__ import annotations

import argparse
import os
from pathlib import Path
import signal
import subprocess
import sys
import traceback


ROOT = Path(__file__).resolve().parents[4]
TESTS = ROOT / "tests/telemetry-e2e"
DOWNLOADER = TESTS / "download_collector.py"
# One case may build the release wheel and install the CLI before it runs.
CASE_TIMEOUT_SECONDS = 30 * 60
SETUP_TIMEOUT_SECONDS = 5 * 60
PYTEST_REQUIREMENTS = ["pytest==8.4.2", "pytest-timeout==2.4.0"]
COLLECTOR_BINARY_NAME = "otelcol-contrib.exe" if os.name == "nt" else "otelcol-contrib"

SETUP_CASES = (
    ("install-pytest", [sys.executable, "-m", "pip", "install", *PYTEST_REQUIREMENTS]),
    ("download-collector", None),  # command needs the output directory
)
TEST_FILES = (
    ("native", "test_native.py"),
    ("frontends", "test_frontends.py"),
    ("harness-timeouts", "test_harness_timeouts.py"),
)


def setup_commands(output: Path) -> list[tuple[str, list[str]]]:
    """Commands that must succeed before any test case can run."""
    return [
        (name, command if command is not None
         else [sys.executable, str(DOWNLOADER), str(output / "otelcol-contrib")])
        for name, command in SETUP_CASES
    ]


def case_commands(output: Path) -> list[tuple[str, list[str]]]:
    """One pytest invocation per test file, each with its own JUnit report."""
    return [
        (name, [sys.executable, "-m", "pytest", "-q", str(TESTS / file),
                f"--junitxml={output / (name + '.xml')}"])
        for name, file in TEST_FILES
    ]


def timeout_output_text(output: str | bytes | None) -> str:
    """Normalize captured timeout output for the text receipt."""
    if output is None:
        return ""
    if isinstance(output, bytes):
        return output.decode(errors="replace")
    return output


def timeout_output_text_after_cleanup(partial: str | bytes | None, drained: str | bytes | None) -> str:
    """Keep timeout output without duplicating data returned by the final drain."""
    partial_text = timeout_output_text(partial)
    drained_text = timeout_output_text(drained)
    if not drained_text:
        return partial_text
    if drained_text.startswith(partial_text):
        return drained_text
    return partial_text + drained_text


def case_process_options() -> dict[str, object]:
    """Place each case in an owned process group or Windows job group."""
    if os.name == "nt":
        return {"creationflags": getattr(subprocess, "CREATE_NEW_PROCESS_GROUP", 0x00000200)}
    return {"start_new_session": True}


def terminate_process_tree(process: subprocess.Popen[str]) -> str:
    """Kill the entire case process tree after its watchdog expires."""
    if os.name == "nt":
        result = subprocess.run(
            ["taskkill", "/PID", str(process.pid), "/T", "/F"],
            capture_output=True,
            check=False,
            text=True,
        )
        return f"cleanup=windows-taskkill-exit={result.returncode}"
    try:
        os.killpg(process.pid, signal.SIGKILL)
    except ProcessLookupError:
        return "cleanup=posix-process-group-already-exited"
    return "cleanup=posix-process-group-killed"


def run_case(
    name: str, command: list[str], *, environment: dict[str, str], output: Path,
    timeout: int = CASE_TIMEOUT_SECONDS,
) -> bool:
    """Run one bounded case and retain its entire command/test receipt."""
    log = output / f"{name}.log"
    process = subprocess.Popen(
        command,
        cwd=ROOT,
        env=environment,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        **case_process_options(),
    )
    try:
        stdout, stderr = process.communicate(timeout=timeout)
    except subprocess.TimeoutExpired as error:
        cleanup = terminate_process_tree(process)
        drained_stdout, drained_stderr = process.communicate()
        with log.open("w", encoding="utf-8") as receipt:
            receipt.write("$ " + " ".join(command) + "\n")
            receipt.write(timeout_output_text_after_cleanup(error.stdout, drained_stdout))
            receipt.write(timeout_output_text_after_cleanup(error.stderr, drained_stderr))
            receipt.write(f"timeout={timeout}\n")
            receipt.write(cleanup + "\n")
        print(f"collector case {name}: timed out; inspect {log}", file=sys.stderr)
        return False

    with log.open("w", encoding="utf-8") as receipt:
        receipt.write("$ " + " ".join(command) + "\n")
        receipt.write(stdout)
        receipt.write(stderr)
        receipt.write(f"exit={process.returncode}\n")
        receipt.write("cleanup=process-exited\n")
    if process.returncode:
        print(f"collector case {name}: exit {process.returncode}; inspect {log}", file=sys.stderr)
        return False
    print(f"collector case {name}: passed; receipt={log}")
    return True


def run(source_sha: str, output: Path) -> bool:
    """Prepare the pinned Collector, then run every test case, retaining later results on failure."""
    output.mkdir(parents=True, exist_ok=True)
    environment = dict(os.environ)
    for name, command in setup_commands(output):
        if not run_case(name, command, environment=environment, output=output, timeout=SETUP_TIMEOUT_SECONDS):
            print(f"collector setup {name} failed; no test case was run", file=sys.stderr)
            return False
    case_environment = environment | {
        "TELEMETRY_E2E_COLLECTOR_BINARY": str(output / COLLECTOR_BINARY_NAME),
    }
    outcomes = [
        run_case(name, command, environment=case_environment, output=output)
        for name, command in case_commands(output)
    ]
    return all(outcomes)


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source-sha", required=True)
    parser.add_argument("--output-dir", required=True, type=Path)
    args = parser.parse_args(argv)
    output = args.output_dir.expanduser().resolve()
    try:
        return 0 if run(args.source_sha, output) else 1
    except (OSError, subprocess.CalledProcessError) as error:
        output.mkdir(parents=True, exist_ok=True)
        (output / "failure-report.txt").write_text(traceback.format_exc(), encoding="utf-8")
        print(f"collector integration: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
