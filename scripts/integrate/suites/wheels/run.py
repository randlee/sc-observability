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
import os
from pathlib import Path
import shutil
import subprocess
import sys
import traceback
from collections.abc import Callable
from typing import TypeVar


ROOT = Path(__file__).resolve().parents[4]
if str(ROOT) not in sys.path:
    sys.path.insert(0, str(ROOT))

from scripts.ci import python_binding_validator


BUILD_TIMEOUT_SECONDS = 10 * 60
RUNTIME_TIMEOUT_SECONDS = 2 * 60
T = TypeVar("T")


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


def recreate_output_directory(directory: Path) -> None:
    """Clear one runner-owned subdirectory so a retained output can be rerun."""
    if directory.exists():
        if not directory.is_dir() or directory.is_symlink():
            raise SuiteError(f"runner output path must be a directory: {directory}")
        shutil.rmtree(directory)
    directory.mkdir(parents=True)


def provision_maturin(output: Path) -> Path:
    """Install the repository-pinned Maturin into this suite's isolated builder venv."""
    build_venv = output / "build-venv"
    recreate_output_directory(build_venv)
    run_checked(
        [sys.executable, "-m", "venv", str(build_venv)],
        cwd=ROOT,
        timeout=BUILD_TIMEOUT_SECONDS,
    )
    builder = python_binding_validator.venv_python(build_venv)
    try:
        maturin_pin = f"maturin=={python_binding_validator.maturin_version(ROOT)}"
    except python_binding_validator.BindingValidationError as error:
        raise SuiteError(f"cannot provision the candidate-wheel builder: {error}") from error
    run_checked(
        [str(builder), "-m", "pip", "install", "--disable-pip-version-check", "--no-input", maturin_pin],
        cwd=ROOT,
        timeout=BUILD_TIMEOUT_SECONDS,
    )
    return builder


def build_wheel(output: Path) -> Path:
    """Build exactly one release-configured candidate wheel for this checkout."""
    wheel_dir = output / "wheel"
    recreate_output_directory(wheel_dir)
    builder = provision_maturin(output)
    run_checked(
        python_binding_validator.maturin_build_command(
            ROOT, wheel_dir, maturin_command=[str(builder), "-m", "maturin"]
        ),
        cwd=ROOT,
        timeout=BUILD_TIMEOUT_SECONDS,
    )
    wheels = sorted(wheel_dir.glob("*.whl"))
    if len(wheels) != 1:
        raise SuiteError(f"expected exactly one candidate wheel, found: {wheels}")
    return wheels[0]


IMPORT_AND_TELEMETRY_PROBE = r'''
import json
import pathlib
import sys
from sc_observability import (
    Ok,
    Telemetry,
    TelemetryErr,
)
import sc_observability
import sc_observability._native as native

origins = {
    "package": pathlib.Path(sc_observability.__file__).resolve(),
    "native": pathlib.Path(native.__file__).resolve(),
}

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


def strict_runtime_environment() -> dict[str, str]:
    """Use the qualification suite's warnings and asyncio diagnostics."""
    environment = os.environ.copy()
    environment.update({
        "SC_OBSERVABILITY_RUNTIME_TEST": "1",
        "PYTHONDEVMODE": "1",
        "PYTHONASYNCIODEBUG": "1",
        "PYTHONWARNINGS": "error",
    })
    return environment


def stage_installed_runtime_suite(runtime: Path) -> Path:
    """Relocate the public runtime and typing suite beside the clean venv."""
    tests = runtime / "tests"
    shutil.copytree(ROOT / "bindings/python/sc-observability-py/tests", tests)
    return tests


def run_installed_runtime_tests(python: Path, runtime: Path, tests: Path) -> None:
    """Exercise owned and async public APIs through the installed candidate."""
    run_checked(
        [
            str(python), "-I", "-X", "dev", "-W", "error", "-m", "pytest",
            str(tests / "test_runtime.py"),
            str(tests / "test_async_runtime.py"),
            "-ra",
        ],
        cwd=runtime,
        environment=strict_runtime_environment(),
    )


def run_installed_typing_tests(python: Path, runtime: Path, tests: Path) -> None:
    """Run the public tagged-result typing suites against the installed wheel."""
    run_checked(
        [
            str(python), "-I", "-m", "mypy", "--strict", "--python-version", "3.10",
            str(tests / "typing/test_result_narrowing.py"),
            str(tests / "typing/test_async_narrowing.py"),
            str(tests / "typing/test_telemetry_typing.py"),
        ],
        cwd=runtime,
        environment=strict_runtime_environment(),
    )


def prepare_installed_runtime(wheel: Path, output: Path) -> tuple[Path, Path, Path]:
    """Install one wheel and return the independent installed checks' inputs."""
    runtime = output / "runtime"
    recreate_output_directory(runtime)
    venv = runtime / "venv"
    run_checked([sys.executable, "-m", "venv", str(venv)], cwd=runtime)
    python = python_binding_validator.venv_python(venv)
    run_checked(
        [str(python), "-m", "pip", "install", "--disable-pip-version-check", "--no-input", str(wheel)],
        cwd=runtime,
    )
    run_checked(
        [str(python), "-m", "pip", "install", "--disable-pip-version-check", "--no-input", "pytest==9.1.1", "mypy==2.3.1"],
        cwd=runtime,
    )
    tests = stage_installed_runtime_suite(runtime)
    return python, runtime, tests


def probe_installed_runtime(python: Path, runtime: Path) -> dict[str, str]:
    """Run the installed telemetry probe and return its reported artifacts."""
    output_text = run_checked(
        [str(python), "-I", "-X", "dev", "-W", "error", "-c", IMPORT_AND_TELEMETRY_PROBE],
        cwd=runtime,
        environment=strict_runtime_environment(),
    )
    return {"python": str(python), **json.loads(output_text)}


def run_embedded_host(python: Path, package: Path) -> None:
    """Run the Rust-host proof against the installed binding package file."""
    try:
        environment = python_binding_validator.embedded_environment(
            python, timeout=RUNTIME_TIMEOUT_SECONDS
        )
    except python_binding_validator.BindingValidationError as error:
        raise SuiteError(str(error)) from error
    environment["SC_OBSERVABILITY_ATTACHED_PACKAGE"] = str(package)
    environment.update({
        "PYTHONDEVMODE": "1",
        "PYTHONASYNCIODEBUG": "1",
        "PYTHONWARNINGS": "error",
    })
    run_checked(
        ["cargo", "run", "--locked", "-p", "rust-python-logging"],
        cwd=ROOT,
        environment=environment,
        timeout=BUILD_TIMEOUT_SECONDS,
    )


def observe(
        checks: dict[str, dict[str, str]], name: str, action: Callable[[], T]
) -> T | None:
    """Run one check without hiding its bounded failure from later independent checks."""
    try:
        value = action()
    except (OSError, SuiteError, json.JSONDecodeError, subprocess.CalledProcessError,
            python_binding_validator.BindingValidationError) as error:
        checks[name] = {"status": "failed", "detail": str(error)}
        return None
    checks[name] = {"status": "passed"}
    return value


def mark_blocked(
        checks: dict[str, dict[str, str]], names: tuple[str, ...], reason: str
) -> None:
    """Record checks that cannot run because their required setup did not succeed."""
    for name in names:
        checks[name] = {"status": "blocked", "detail": reason}


def write_result(output: Path, result: dict[str, object]) -> None:
    """Persist every observed check outcome, including failed and blocked checks."""
    (output / "result.json").write_text(
        json.dumps(result, indent=2, sort_keys=True) + "\n", encoding="utf-8"
    )


def run(source_sha: str, output: Path) -> dict[str, object]:
    """Execute every check whose prerequisites succeeded and retain each observed outcome."""
    output.mkdir(parents=True, exist_ok=True)
    (output / "failure-report.txt").unlink(missing_ok=True)
    checks: dict[str, dict[str, str]] = {}
    wheel = observe(checks, "wheel_build", lambda: build_wheel(output))
    if wheel is None:
        mark_blocked(
            checks,
            ("clean_install", "owned_typed_runtime", "installed_typing",
             "installed_telemetry_lifecycle", "installed_origin", "rust_host_attached_python"),
            "wheel_build failed",
        )
        installed: dict[str, str] | None = None
    else:
        prepared = observe(
            checks, "clean_install", lambda: prepare_installed_runtime(wheel, output)
        )
        if prepared is None:
            mark_blocked(
                checks,
                ("owned_typed_runtime", "installed_typing", "installed_telemetry_lifecycle",
                 "installed_origin", "rust_host_attached_python"),
                "clean_install failed",
            )
            installed = None
        else:
            python, runtime, tests = prepared
            observe(checks, "owned_typed_runtime", lambda: run_installed_runtime_tests(python, runtime, tests))
            observe(checks, "installed_typing", lambda: run_installed_typing_tests(python, runtime, tests))
            installed = observe(checks, "installed_telemetry_lifecycle", lambda: probe_installed_runtime(python, runtime))
            if installed is None:
                mark_blocked(
                    checks,
                    ("installed_origin", "rust_host_attached_python"),
                    "installed_telemetry_lifecycle failed",
                )
            else:
                origins = observe(
                    checks,
                    "installed_origin",
                    lambda: python_binding_validator.installed_origins(installed),
                )
                if origins is None:
                    mark_blocked(
                        checks,
                        ("rust_host_attached_python",),
                        "installed_origin failed",
                    )
                else:
                    observe(
                        checks,
                        "rust_host_attached_python",
                        lambda: run_embedded_host(Path(installed["python"]), origins["package"]),
                    )
    result: dict[str, object] = {
        "schema_version": 1,
        "status": "passed" if all(value["status"] == "passed" for value in checks.values()) else "failed",
        "source_commit": source_sha,
        "wheel": None if wheel is None else {"path": str(wheel), "sha256": wheel_digest(wheel)},
        "installed_artifacts": installed,
        "checks": checks,
    }
    write_result(output, result)
    return result


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source-sha", required=True)
    parser.add_argument("--output-dir", required=True, type=Path)
    args = parser.parse_args(argv)
    try:
        result = run(args.source_sha, args.output_dir.resolve())
        if result["status"] != "passed":
            (args.output_dir / "failure-report.txt").write_text(
                json.dumps(result, indent=2, sort_keys=True) + "\n", encoding="utf-8"
            )
            print("wheels integration: one or more observed checks failed", file=sys.stderr)
            return 1
    except (OSError, SuiteError, json.JSONDecodeError, subprocess.CalledProcessError) as error:
        args.output_dir.mkdir(parents=True, exist_ok=True)
        (args.output_dir / "failure-report.txt").write_text(traceback.format_exc(), encoding="utf-8")
        print(f"wheels integration: {error}", file=sys.stderr)
        return 1
    print(json.dumps(result, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
