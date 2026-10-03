"""Run vendored Bash snippets consistently on Unix and Windows test hosts."""

from __future__ import annotations

import os
import sys
from collections.abc import Callable, Mapping
from pathlib import Path, PureWindowsPath


def _windows(platform: str | None = None) -> bool:
    """Return whether shell selection must avoid Windows' WSL shim."""
    return (platform or sys.platform) == "win32"


def git_bash_path(
    *,
    environ: Mapping[str, str] | None = None,
    platform: str | None = None,
    exists: Callable[[Path], bool] | None = None,
) -> Path:
    """Find Git Bash explicitly instead of accepting ``bash.exe`` from WSL."""
    if not _windows(platform):
        return Path("bash")

    environment = os.environ if environ is None else environ
    candidates: list[Path] = []
    if configured := environment.get("GIT_BASH_PATH"):
        candidates.append(Path(configured))
    for variable in ("ProgramW6432", "ProgramFiles", "ProgramFiles(x86)"):
        if root := environment.get(variable):
            candidates.append(Path(root) / "Git" / "bin" / "bash.exe")
    path_exists = Path.is_file if exists is None else exists
    for candidate in candidates:
        if path_exists(candidate):
            return candidate
    raise RuntimeError(
        "Git Bash is required for publish-kit shell tests on Windows; "
        "set GIT_BASH_PATH or install Git for Windows."
    )


def bash_command(**kwargs: object) -> list[str]:
    """Return the shell command, choosing Git Bash instead of the WSL shim."""
    return [str(git_bash_path(**kwargs))]


def bash_path(path: Path | str, *, platform: str | None = None) -> str:
    """Translate a Windows path to the spelling Git Bash uses inside scripts."""
    value = str(path)
    if not _windows(platform):
        return value
    native = PureWindowsPath(value)
    if not native.drive:
        return value.replace("\\", "/")
    relative = native.relative_to(native.anchor).as_posix()
    drive = native.drive.rstrip(":").lower()
    return f"/{drive}/{relative}" if relative != "." else f"/{drive}"


def bash_path_list(value: str, *, platform: str | None = None) -> str:
    """Translate a Windows PATH list before passing it to Git Bash."""
    if not _windows(platform):
        return value
    return ":".join(bash_path(entry, platform=platform) for entry in value.split(";") if entry)


def bash_environment(
    base: Mapping[str, str] | None = None,
    *,
    prepend_path: Path | str | None = None,
    github_output: Path | str | None = None,
    platform: str | None = None,
) -> dict[str, str]:
    """Build Git-Bash-ready environment variables for fixture subprocesses."""
    environment = dict(os.environ if base is None else base)
    path = bash_path_list(environment.get("PATH", ""), platform=platform)
    if prepend_path is not None:
        prefix = bash_path(prepend_path, platform=platform)
        path = f"{prefix}:{path}" if path else prefix
    environment["PATH"] = path
    if github_output is not None:
        environment["GITHUB_OUTPUT"] = bash_path(github_output, platform=platform)
    return environment
