#!/usr/bin/env python3
"""Run the Rust public-factory viewer tests against one pinned viewer instance.

The runner is CI-only: it owns the viewer it starts in its output directory,
runs the SDK and synchronous HTTP public-factory tests independently, queries
each backend's actual records, and removes only that owned state on exit.
"""

from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
import socket
import subprocess
import sys
import traceback


ROOT = Path(__file__).resolve().parents[4]
DOWNLOADER = ROOT / "scripts/ci/fixtures/otlp/desktop-viewer/download_pinned_release.py"
HARNESS = ROOT / "scripts/ci/fixtures/otlp/desktop-viewer/viewer_harness.py"
TEST = "full_stack_integration"
FEATURES = "sync-http,otlp-sdk"
DOWNLOAD_TIMEOUT_SECONDS = 180
TEST_TIMEOUT_SECONDS = 15 * 60
CLEANUP_TIMEOUT_SECONDS = 30
# The shared harness permits 30 seconds for readiness; leave another minute
# for its probe and termination allowances before the outer runner intervenes.
START_TIMEOUT_SECONDS = 90
# The shared harness permits 120 seconds for viewer readback; leave another
# minute for its requests and receipt serialization before the outer runner
# intervenes.
READBACK_TIMEOUT_SECONDS = 180
HOST = "127.0.0.1"

TESTS = {
    "sync-http": "public_sync_http_factory_exports_three_signals_to_the_pinned_desktop_viewer",
    "sdk": "public_sdk_factory_exports_three_signals_to_the_pinned_desktop_viewer",
}


class SuiteError(RuntimeError):
    """Reports a bounded CI command failure with its retained evidence path."""

    def __init__(self, message: str, *, exit_code: int | None = None) -> None:
        super().__init__(message)
        self.exit_code = exit_code


def verify_source_sha(source_sha: str) -> None:
    """Bind the suite execution to exactly the checked-out candidate commit."""
    if len(source_sha) != 40 or any(char not in "0123456789abcdef" for char in source_sha.lower()):
        raise SuiteError("source-sha must be a full 40-hex commit")
    head = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip()
    if head != source_sha.lower():
        raise SuiteError(f"checkout HEAD {head} does not match source-sha {source_sha}")


def reserved_ports() -> tuple[int, int, int]:
    """Choose three distinct local ports; the harness rechecks them before use."""
    listeners: list[socket.socket] = []
    try:
        for _ in range(3):
            listener = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
            listener.bind((HOST, 0))
            listeners.append(listener)
        ports = [listener.getsockname()[1] for listener in listeners]
        return ports[0], ports[1], ports[2]
    finally:
        for listener in listeners:
            listener.close()


def run_checked(command: list[str], *, environment: dict[str, str], timeout: int, log: Path) -> str:
    """Run one bounded command and write its command, streams, and exit code."""
    try:
        completed = subprocess.run(
            command,
            cwd=ROOT,
            env=environment,
            text=True,
            capture_output=True,
            timeout=timeout,
            check=False,
        )
    except subprocess.TimeoutExpired as error:
        with log.open("a", encoding="utf-8") as output:
            output.write("$ " + " ".join(command) + "\n")
            output.write(error.stdout or "")
            output.write(error.stderr or "")
            output.write(f"timeout={timeout}\n")
        raise SuiteError(f"timed out after {timeout}s; inspect {log}") from error
    with log.open("a", encoding="utf-8") as output:
        output.write("$ " + " ".join(command) + "\n")
        output.write(completed.stdout)
        output.write(completed.stderr)
        output.write(f"exit={completed.returncode}\n")
    if completed.returncode:
        raise SuiteError(f"command exited {completed.returncode}; inspect {log}", exit_code=completed.returncode)
    return completed.stdout


def invoke_harness(arguments: list[str], *, environment: dict[str, str], timeout: int, log: Path) -> str:
    """Run the shared read-only harness and retain its command receipt."""
    return run_checked([sys.executable, str(HARNESS), *arguments], environment=environment,
                       timeout=timeout, log=log)


def factory_test_command(test_name: str) -> list[str]:
    """Return the isolated ignored public-factory test command for one backend."""
    return [
        "cargo", "test", "--locked", "-p", "sc-observability-otlp", "--test", TEST,
        "--features", FEATURES, test_name, "--", "--ignored", "--exact",
    ]


def cleanup_viewer(state: Path, *, environment: dict[str, str], output: Path) -> dict[str, str]:
    """Stop any viewer proven owned by this run, retaining the cleanup outcome."""
    log = output / "cleanup.log"
    if not (state / "viewer.pid").is_file():
        return {"status": "not-needed", "log": str(log)}
    try:
        invoke_harness(
            ["stop", "--state-dir", str(state), "--timeout", "10", "--remove-state"],
            environment=environment,
            timeout=CLEANUP_TIMEOUT_SECONDS,
            log=log,
        )
    except (OSError, SuiteError) as error:
        try:
            with log.open("a", encoding="utf-8") as cleanup_log:
                cleanup_log.write(f"cleanup_error={error}\n")
        except OSError:
            pass
        return {"status": "failed", "error": str(error), "log": str(log)}
    return {"status": "passed", "log": str(log)}


def run(source_sha: str, output: Path) -> dict[str, object]:
    """Run each public factory and its readback under an owned pinned viewer."""
    verify_source_sha(source_sha)
    output.mkdir(parents=True, exist_ok=True)
    environment = os.environ | {"CI": "1", "GITHUB_ACTIONS": "true"}
    setup_log = output / "setup.log"
    downloader_receipt = run_checked(
        [sys.executable, str(DOWNLOADER), str(output / "otel-desktop-viewer")],
        environment=environment,
        timeout=DOWNLOAD_TIMEOUT_SECONDS,
        log=setup_log,
    )
    try:
        pinned = json.loads(downloader_receipt)
        binary = str(pinned["binary"])
        binary_sha256 = str(pinned["binary_sha256"])
        version = str(pinned["version"])
    except (IndexError, KeyError, json.JSONDecodeError) as error:
        raise SuiteError(f"invalid pinned-viewer downloader receipt; inspect {setup_log}") from error

    http, grpc, ui = reserved_ports()
    state = output / "viewer-state"
    start = ["start", "--binary", binary, "--binary-sha256", binary_sha256,
             "--version", version, "--state-dir", str(state), "--host", HOST,
             "--http", str(http), "--grpc", str(grpc), "--ui", str(ui)]
    results: dict[str, object] = {}
    result: dict[str, object] = {
        "schema_version": 1,
        "status": "failed",
        "source_commit": source_sha,
        "viewer": pinned,
        "backends": results,
        "cleanup": {"status": "pending", "log": str(output / "cleanup.log")},
    }
    primary_error: OSError | SuiteError | None = None
    try:
        invoke_harness(start, environment=environment, timeout=START_TIMEOUT_SECONDS, log=setup_log)
        backend_environment = environment | {
            "D9_VIEWER_SYNC_HTTP_ADDRESS": f"{HOST}:{http}",
            "D9_VIEWER_SDK_ADDRESS": f"{HOST}:{grpc}",
        }
        failed_backends: list[str] = []
        for backend, test_name in TESTS.items():
            log = output / f"{backend}.log"
            test_exit: int | None = None
            try:
                run_checked(
                    factory_test_command(test_name),
                    environment=backend_environment,
                    timeout=TEST_TIMEOUT_SECONDS,
                    log=log,
                )
                test_exit = 0
                receipt = invoke_harness(
                    ["assert-production", "--state-dir", str(state), "--backend", backend],
                    environment=backend_environment,
                    timeout=READBACK_TIMEOUT_SECONDS,
                    log=log,
                )
                try:
                    readback = json.loads(receipt)
                except json.JSONDecodeError as error:
                    raise SuiteError(
                        f"invalid assert-production receipt for {backend}; inspect {log}"
                    ) from error
            except (OSError, SuiteError) as error:
                failed_backends.append(backend)
                results[backend] = {
                    "status": "failed",
                    "test_exit": test_exit if test_exit is not None else getattr(error, "exit_code", None),
                    "error": str(error),
                    "log": str(log),
                }
            else:
                results[backend] = {
                    "status": "passed",
                    "test_exit": test_exit,
                    "readback": readback,
                    "log": str(log),
                }

        result["status"] = "failed" if failed_backends else "passed"
        if failed_backends:
            primary_error = SuiteError(f"backend qualification failed: {', '.join(failed_backends)}")
    except (OSError, SuiteError) as error:
        primary_error = error
    finally:
        cleanup = cleanup_viewer(state, environment=environment, output=output)
        result["cleanup"] = cleanup
        if cleanup["status"] == "failed":
            result["status"] = "failed"
        (output / "result.json").write_text(json.dumps(result, indent=2, sort_keys=True) + "\n", encoding="utf-8")

    if primary_error is not None:
        raise primary_error
    if cleanup["status"] == "failed":
        raise SuiteError(f"viewer cleanup failed; inspect {cleanup['log']}")
    return result

def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source-sha", required=True)
    parser.add_argument("--output-dir", required=True, type=Path)
    args = parser.parse_args(argv)
    output = args.output_dir.expanduser().resolve()
    try:
        result = run(args.source_sha, output)
    except (OSError, SuiteError, subprocess.CalledProcessError) as error:
        output.mkdir(parents=True, exist_ok=True)
        (output / "failure-report.txt").write_text(traceback.format_exc(), encoding="utf-8")
        print(f"rust-viewer integration: {error}", file=sys.stderr)
        return 1
    print(json.dumps(result, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
