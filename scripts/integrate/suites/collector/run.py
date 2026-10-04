#!/usr/bin/env python3
"""Run the hermetic OTLP collector conformance corpus for one candidate SHA.

The existing Rust fixtures own their loopback receivers, readiness signals,
decoded-payload assertions, exporter lifecycle, and receiver cleanup.  This
runner keeps those real network assertions isolated by feature matrix entry,
retains one log per entry, and continues after an entry fails.
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
CASE_TIMEOUT_SECONDS = 15 * 60

CASES = (
    (
        "sync-http-full-stack",
        [
            "cargo", "test", "--locked", "-p", "sc-observability-otlp",
            "--test", "full_stack_integration", "--features", "sync-http", "--", "--nocapture",
        ],
    ),
    (
        "sdk-full-stack",
        [
            "cargo", "test", "--locked", "-p", "sc-observability-otlp",
            "--test", "full_stack_integration", "--features", "otlp-sdk", "--", "--nocapture",
        ],
    ),
    (
        "combined-full-stack",
        [
            "cargo", "test", "--locked", "-p", "sc-observability-otlp",
            "--test", "full_stack_integration", "--features", "otlp-sdk,sync-http", "--", "--nocapture",
        ],
    ),
    (
        "canonical-ingress",
        [
            "cargo", "test", "--locked", "-p", "sc-observability-otlp",
            "--test", "canonical_ingress", "--features", "otlp-sdk,sync-http", "--", "--nocapture",
        ],
    ),
)


class SuiteError(RuntimeError):
    """Reports an invalid candidate or unavailable integration command."""


def verify_source_sha(source_sha: str) -> None:
    """Bind execution to exactly the checked-out immutable candidate."""
    if len(source_sha) != 40 or any(character not in "0123456789abcdef" for character in source_sha.lower()):
        raise SuiteError("source-sha must be a full 40-hex commit")
    head = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip()
    if head != source_sha.lower():
        raise SuiteError(f"checkout HEAD {head} does not match source-sha {source_sha}")


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
    """Place each cargo case in an owned process group or Windows job group."""
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


def run_case(name: str, command: list[str], *, environment: dict[str, str], output: Path) -> bool:
    """Run one bounded corpus entry and retain its entire cargo/test receipt."""
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
        stdout, stderr = process.communicate(timeout=CASE_TIMEOUT_SECONDS)
    except subprocess.TimeoutExpired as error:
        cleanup = terminate_process_tree(process)
        drained_stdout, drained_stderr = process.communicate()
        with log.open("w", encoding="utf-8") as receipt:
            receipt.write("$ " + " ".join(command) + "\n")
            receipt.write(timeout_output_text_after_cleanup(error.stdout, drained_stdout))
            receipt.write(timeout_output_text_after_cleanup(error.stderr, drained_stderr))
            receipt.write(f"timeout={CASE_TIMEOUT_SECONDS}\n")
            receipt.write(cleanup + "\n")
        print(f"collector case {name}: timed out; inspect {log}", file=sys.stderr)
        return False

    with log.open("w", encoding="utf-8") as receipt:
        receipt.write("$ " + " ".join(command) + "\n")
        receipt.write(stdout)
        receipt.write(stderr)
        receipt.write(f"exit={process.returncode}\n")
        receipt.write("cleanup=cargo-process-exited\n")
    if process.returncode:
        print(f"collector case {name}: exit {process.returncode}; inspect {log}", file=sys.stderr)
        return False
    print(f"collector case {name}: passed; receipt={log}")
    return True


def run(source_sha: str, output: Path) -> bool:
    """Run every real collector corpus entry, retaining later results on failure."""
    verify_source_sha(source_sha)
    output.mkdir(parents=True, exist_ok=True)
    environment = os.environ | {"CI": "1", "GITHUB_ACTIONS": "true"}
    outcomes = [
        run_case(name, command, environment=environment, output=output)
        for name, command in CASES
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
    except (OSError, SuiteError, subprocess.CalledProcessError) as error:
        output.mkdir(parents=True, exist_ok=True)
        (output / "failure-report.txt").write_text(traceback.format_exc(), encoding="utf-8")
        print(f"collector integration: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
