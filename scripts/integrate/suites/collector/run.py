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
            "--test", "full_stack_integration", "--features", "sync-http",
        ],
    ),
    (
        "sdk-full-stack",
        [
            "cargo", "test", "--locked", "-p", "sc-observability-otlp",
            "--test", "full_stack_integration", "--features", "otlp-sdk",
        ],
    ),
    (
        "combined-full-stack",
        [
            "cargo", "test", "--locked", "-p", "sc-observability-otlp",
            "--test", "full_stack_integration", "--features", "otlp-sdk,sync-http",
        ],
    ),
    (
        "canonical-ingress",
        [
            "cargo", "test", "--locked", "-p", "sc-observability-otlp",
            "--test", "canonical_ingress", "--features", "otlp-sdk,sync-http",
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


def run_case(name: str, command: list[str], *, environment: dict[str, str], output: Path) -> bool:
    """Run one bounded corpus entry and retain its entire cargo/test receipt."""
    log = output / f"{name}.log"
    try:
        completed = subprocess.run(
            command,
            cwd=ROOT,
            env=environment,
            text=True,
            capture_output=True,
            timeout=CASE_TIMEOUT_SECONDS,
            check=False,
        )
    except subprocess.TimeoutExpired as error:
        with log.open("w", encoding="utf-8") as receipt:
            receipt.write("$ " + " ".join(command) + "\n")
            receipt.write(timeout_output_text(error.stdout))
            receipt.write(timeout_output_text(error.stderr))
            receipt.write(f"timeout={CASE_TIMEOUT_SECONDS}\n")
        print(f"collector case {name}: timed out; inspect {log}", file=sys.stderr)
        return False

    with log.open("w", encoding="utf-8") as receipt:
        receipt.write("$ " + " ".join(command) + "\n")
        receipt.write(completed.stdout)
        receipt.write(completed.stderr)
        receipt.write(f"exit={completed.returncode}\n")
    if completed.returncode:
        print(f"collector case {name}: exit {completed.returncode}; inspect {log}", file=sys.stderr)
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
