#!/usr/bin/env python3
"""Build one candidate wheel and exercise its installed and embedded runtimes.

This is intentionally a candidate-artifact check, rather than a replacement
for the B4a publishing qualification.  It reuses only the narrow clean-venv,
installed-origin, and embedded-host checks used by that qualification.
"""
from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import sys
import traceback


ROOT = Path(__file__).resolve().parents[4]
if str(ROOT) not in sys.path:
    sys.path.insert(0, str(ROOT))

from scripts.ci import python_binding_validator


BUILD_TIMEOUT_SECONDS = 10 * 60
RUNTIME_TIMEOUT_SECONDS = 2 * 60


class SuiteError(RuntimeError):
    """Reports one bounded command with the output needed to diagnose it."""


def run_checked(command: list[str], *, cwd: Path, environment: dict[str, str] | None = None,
                timeout: int = RUNTIME_TIMEOUT_SECONDS) -> str:
    """Run one bounded command and retain its stdout and stderr in the error."""
    try:
        completed = subprocess.run(
            command,
            cwd=cwd,
            env=environment,
            check=True,
            capture_output=True,
            text=True,
            timeout=timeout,
        )
    except subprocess.TimeoutExpired as error:
        raise SuiteError(f"timed out after {timeout}s: {command!r}\nstdout:\n{error.stdout or ''}\nstderr:\n{error.stderr or ''}") from error
    except subprocess.CalledProcessError as error:
        raise SuiteError(
            f"failed ({error.returncode}): {command!r}\nstdout:\n{error.stdout}\nstderr:\n{error.stderr}"
        ) from error
    return completed.stdout


def wheel_digest(wheel: Path) -> str:
    """Return the immutable candidate wheel digest recorded in the suite report."""
    return hashlib.sha256(wheel.read_bytes()).hexdigest()


def build_wheel(output: Path) -> Path:
    """Build exactly one release-configured candidate wheel for this checkout."""
    wheel_dir = output / "wheel"
    wheel_dir.mkdir(parents=True, exist_ok=False)
    run_checked(
        python_binding_validator.maturin_build_command(ROOT, wheel_dir),
        cwd=ROOT,
        timeout=BUILD_TIMEOUT_SECONDS,
    )
    wheels = sorted(wheel_dir.glob("*.whl"))
    if len(wheels) != 1:
        raise SuiteError(f"expected exactly one candidate wheel, found: {wheels}")
    return wheels[0]


IMPORT_AND_RUNTIME_PROBE = r'''
import json
import pathlib
import sys
from sc_observability import (
    LogEvent,
    LoggerConfig,
    LogQuery,
    Ok,
    Telemetry,
    TelemetryErr,
    create_logger,
)
import sc_observability
import sc_observability._native as native

origins = {
    "package": pathlib.Path(sc_observability.__file__).resolve(),
    "native": pathlib.Path(native.__file__).resolve(),
}
created = create_logger(LoggerConfig(service="e5-installed-wheel", log_root="."))
if not isinstance(created, Ok):
    raise SystemExit(f"owned logger was not a tagged Ok: {created!r}")
logger = created.value
for result in (
    logger.log(LogEvent(level="info", target="e5.wheels", action="installed-runtime")),
    logger.flush(),
    logger.query(LogQuery(action="installed-runtime")),
    logger.shutdown(),
):
    if not isinstance(result, Ok):
        raise SystemExit(f"owned runtime returned an untagged failure: {result!r}")

# The candidate wheel is built with ``otlp-telemetry``.  Exercise that
# feature through its installed public facade rather than test-only hooks.
if not callable(getattr(native, "open", None)):
    raise SystemExit("candidate wheel omitted the otlp-telemetry native factory")
opened = Telemetry.open(
    store_path="telemetry-store",
    endpoint="http://127.0.0.1:9",
    service_name="e5-installed-wheel-telemetry",
)
if not isinstance(opened, Ok):
    raise SystemExit(f"installed telemetry did not return a tagged Ok: {opened!r}")
telemetry = opened.value
submission = {
    "version": 1,
    "record_key": "e5-installed-wheel-telemetry",
    "resource": {
        "attributes": {"service.name": "e5-installed-wheel-telemetry"},
        "dropped_attributes_count": 0,
        "entity_refs": [],
        "schema_url": None,
    },
    "scope": {
        "name": "e5.wheels",
        "version": None,
        "attributes": {},
        "dropped_attributes_count": 0,
        "schema_url": None,
    },
    "logs": [{
        "time": "2026-10-04T00:00:00Z",
        "body": "installed telemetry",
        "attributes": {},
    }],
    "spans": [],
    "metrics": [],
}
receipt = telemetry.emit(submission)
if not isinstance(receipt, Ok):
    raise SystemExit(f"installed telemetry submit did not return a tagged Ok: {receipt!r}")
for operation, result in (
    ("flush", telemetry.flush(timeout_s=0.1)),
    ("status", telemetry.status()),
    ("shutdown", telemetry.shutdown(timeout_s=0.1)),
):
    if not isinstance(result, (Ok, TelemetryErr)):
        raise SystemExit(f"installed telemetry {operation} returned an untyped outcome: {result!r}")
if not isinstance(telemetry.flush(timeout_s=float("nan")), TelemetryErr):
    raise SystemExit("installed telemetry invalid timeout did not return TelemetryErr")
print(json.dumps({"prefix": sys.prefix, **{key: str(value) for key, value in origins.items()}}, sort_keys=True))
'''


def install_and_probe(wheel: Path, output: Path) -> dict[str, str]:
    """Install one wheel in a clean environment and prove its typed owned runtime."""
    runtime = output / "runtime"
    runtime.mkdir(parents=True, exist_ok=False)
    venv = runtime / "venv"
    run_checked([sys.executable, "-m", "venv", str(venv)], cwd=runtime)
    python = python_binding_validator.venv_python(venv)
    run_checked(
        [str(python), "-m", "pip", "install", "--disable-pip-version-check", "--no-input", str(wheel)],
        cwd=runtime,
    )
    output_text = run_checked([str(python), "-I", "-c", IMPORT_AND_RUNTIME_PROBE], cwd=runtime)
    installed = {"python": str(python), **json.loads(output_text)}
    try:
        python_binding_validator.installed_origins(installed)
    except python_binding_validator.BindingValidationError as error:
        raise SuiteError(str(error)) from error
    return installed


def run_embedded_host(python: Path, package: Path) -> None:
    """Run the Rust-host proof against the installed binding package file."""
    environment = python_binding_validator.embedded_environment(python)
    environment["SC_OBSERVABILITY_ATTACHED_PACKAGE"] = str(package)
    run_checked(
        ["cargo", "run", "--locked", "-p", "rust-python-logging"],
        cwd=ROOT,
        environment=environment,
        timeout=BUILD_TIMEOUT_SECONDS,
    )


def verify_source_sha(source_sha: str) -> None:
    """Reject a runner invocation that is not bound to its checked-out candidate."""
    if len(source_sha) != 40 or any(character not in "0123456789abcdef" for character in source_sha.lower()):
        raise SuiteError("source-sha must be a full 40-hex commit")
    head = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip()
    if head != source_sha.lower():
        raise SuiteError(f"checkout HEAD {head} does not match source-sha {source_sha}")


def run(source_sha: str, output: Path) -> dict[str, object]:
    """Execute the bounded candidate wheel, installed runtime, and attached-host checks."""
    verify_source_sha(source_sha)
    output.mkdir(parents=True, exist_ok=True)
    wheel = build_wheel(output)
    installed = install_and_probe(wheel, output)
    package = python_binding_validator.installed_origins(installed)["package"]
    run_embedded_host(Path(installed["python"]), package)
    result: dict[str, object] = {
        "schema_version": 1,
        "status": "passed",
        "source_commit": source_sha,
        "wheel": {"path": str(wheel), "sha256": wheel_digest(wheel)},
        "installed_artifacts": installed,
        "checks": {
            "clean_install": "passed",
            "installed_origin": "passed",
            "owned_typed_runtime": "passed",
            "installed_telemetry_lifecycle": "passed",
            "rust_host_attached_python": "passed",
        },
    }
    (output / "result.json").write_text(json.dumps(result, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    return result


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source-sha", required=True)
    parser.add_argument("--output-dir", required=True, type=Path)
    args = parser.parse_args(argv)
    try:
        result = run(args.source_sha, args.output_dir.resolve())
    except (OSError, SuiteError, json.JSONDecodeError, subprocess.CalledProcessError) as error:
        args.output_dir.mkdir(parents=True, exist_ok=True)
        (args.output_dir / "failure-report.txt").write_text(traceback.format_exc(), encoding="utf-8")
        print(f"wheels integration: {error}", file=sys.stderr)
        return 1
    print(json.dumps(result, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
