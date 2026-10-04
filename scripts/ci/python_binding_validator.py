#!/usr/bin/env python3
"""Shared, narrow helpers for Python binding candidate validators.

The B.4 validator and the Phase-E wheel integration suite intentionally run
different scopes.  They still need one definition of the candidate build
inputs and the interpreter environment used by the Rust embedding host.
"""
from __future__ import annotations

import argparse
import os
from pathlib import Path
import shutil
import subprocess
import sys
from typing import Mapping

try:
    import tomllib
except ModuleNotFoundError:  # pragma: no cover - Python 3.10 compatibility
    import tomli as tomllib


class BindingValidationError(ValueError):
    """A declared Python binding validation input is invalid."""


INTERPRETER_PROBE_TIMEOUT_SECONDS = 2 * 60


def venv_python(venv: Path, *, platform_name: str | None = None) -> Path:
    """Return the platform-native Python executable for a virtual environment."""
    if (platform_name or os.name) == "nt":
        return venv / "Scripts" / "python.exe"
    return venv / "bin" / "python"


def _prepend_path(current: str | None, directory: str) -> str:
    return directory if not current else directory + os.pathsep + current


def checked_output(command: list[str], *, timeout: int = INTERPRETER_PROBE_TIMEOUT_SECONDS) -> str:
    """Run one interpreter probe with a finite deadline and useful failure."""
    try:
        return subprocess.check_output(command, text=True, timeout=timeout)
    except subprocess.TimeoutExpired as error:
        raise BindingValidationError(f"timed out after {timeout}s: {command!r}") from error


def embedded_environment_updates(
        python: Path, *, platform_name: str | None = None,
        timeout: int = INTERPRETER_PROBE_TIMEOUT_SECONDS) -> dict[str, str]:
    """Return the interpreter and loader variables required by a Rust host."""
    base_prefix = checked_output(
        [str(python), "-I", "-c", "import sys; print(sys.base_prefix)"], timeout=timeout
    ).strip()
    updates = {"PYO3_PYTHON": str(python), "PYTHONHOME": base_prefix}
    selected_platform = platform_name or sys.platform
    if selected_platform in ("nt", "windows"):
        updates["PATH"] = _prepend_path(os.environ.get("PATH"), base_prefix)
    elif selected_platform.startswith("linux"):
        libdir = checked_output(
            [str(python), "-I", "-c", "import sysconfig; print(sysconfig.get_config_var('LIBDIR') or '')"],
            timeout=timeout,
        ).strip()
        if libdir:
            updates["LD_LIBRARY_PATH"] = _prepend_path(os.environ.get("LD_LIBRARY_PATH"), libdir)
    return updates


def embedded_environment(
        python: Path, *, platform_name: str | None = None,
        timeout: int = INTERPRETER_PROBE_TIMEOUT_SECONDS) -> dict[str, str]:
    """Configure the selected interpreter for the Rust-host embedded executable."""
    environment = os.environ.copy()
    environment.pop("PYTHONPATH", None)
    environment.update(embedded_environment_updates(python, platform_name=platform_name, timeout=timeout))
    return environment


def maturin_version(root: Path) -> str:
    """Read the sole candidate-build Maturin pin from its requirements file."""
    requirements = root / "scripts/ci/python-packaging-requirements.txt"
    try:
        values = [
            line.split("==", 1)[1].strip()
            for line in requirements.read_text(encoding="utf-8").splitlines()
            if line.strip() and not line.lstrip().startswith("#") and line.split("==", 1)[0].strip() == "maturin"
        ]
    except OSError as error:
        raise BindingValidationError(f"cannot read Maturin requirements: {requirements}") from error
    if len(values) != 1 or not values[0]:
        raise BindingValidationError("python packaging requirements must declare exactly one maturin== pin")
    return values[0]


def production_maturin_features(root: Path) -> list[str]:
    """Read the production wheel's declared feature set from its pyproject."""
    repository_pyproject = root / "bindings/python/sc-observability-py/pyproject.toml"
    pyproject = repository_pyproject if repository_pyproject.exists() else root / "pyproject.toml"
    try:
        features = tomllib.loads(pyproject.read_text(encoding="utf-8"))["tool"]["maturin"]["features"]
    except (OSError, ValueError, KeyError, TypeError) as error:
        raise BindingValidationError(f"invalid production wheel metadata: {pyproject}") from error
    if not isinstance(features, list) or not features or any(not isinstance(feature, str) or not feature for feature in features):
        raise BindingValidationError("production wheel must declare non-empty maturin features")
    return list(features)


def maturin_build_command(root: Path, wheel_dir: Path) -> list[str]:
    """Build the declared production wheel with the declared Maturin pin/features."""
    maturin = shutil.which("maturin")
    if maturin is not None:
        command = [maturin]
    else:
        uvx = shutil.which("uvx")
        if uvx is None:
            raise BindingValidationError("maturin or uvx is required to build the candidate wheel")
        command = [uvx, "--from", f"maturin=={maturin_version(root)}", "maturin"]
    return [
        *command,
        "build",
        "--locked",
        "--release",
        "--manifest-path",
        str(root / "bindings/python/sc-observability-py/Cargo.toml"),
        "--features",
        ",".join(production_maturin_features(root)),
        "--out",
        str(wheel_dir),
    ]


def installed_origins(installed: Mapping[str, str]) -> dict[str, Path]:
    """Require package and native module imports to be inside the installed venv."""
    try:
        prefix = Path(installed["prefix"]).resolve()
        origins = {name: Path(installed[name]).resolve() for name in ("package", "native")}
    except (KeyError, TypeError) as error:
        raise BindingValidationError("installed-origin probe returned incomplete paths") from error
    escaped = {name: path for name, path in origins.items() if not path.is_relative_to(prefix)}
    if escaped:
        raise BindingValidationError(f"installed artifacts escaped the venv: {escaped}")
    return origins


def embedded_environment_assignments(python: Path) -> list[str]:
    """Emit only the environment updates that shell validators must apply."""
    return [f"{key}={value}" for key, value in embedded_environment_updates(python).items()]


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=("embedded-environment",))
    parser.add_argument("python", type=Path)
    args = parser.parse_args(argv)
    for assignment in embedded_environment_assignments(args.python):
        print(assignment)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
