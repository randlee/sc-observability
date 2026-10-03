"""Resolve the bash executable tests must spawn.

On Windows a bare "bash" resolves to the WSL stub in System32 before PATH is
consulted, so use the bash that ships with Git for Windows.
"""

from __future__ import annotations

import os
import shutil
from pathlib import Path


def _resolve_bash_from_git(git: str | Path) -> str:
    ancestor = Path(git).resolve().parent
    tried: list[Path] = []
    for _ in range(3):
        bash = ancestor / "bin" / "bash.exe"
        tried.append(bash)
        if bash.is_file():
            return str(bash)
        ancestor = ancestor.parent
    raise RuntimeError(
        "Git for Windows bash.exe not found; paths tried: "
        + ", ".join(str(path) for path in tried)
    )


def _resolve_bash() -> str:
    if os.name != "nt":
        return "bash"
    git = shutil.which("git")
    if git is None:
        raise RuntimeError("git was not found on PATH; cannot locate Git for Windows bash.exe")
    return _resolve_bash_from_git(git)


BASH = _resolve_bash()


def prepend_path(directory: str | Path, path: str | None = None) -> str:
    """Return a PATH value that searches ``directory`` first.

    Native Windows processes use ``;`` between PATH entries; Git for Windows
    bash converts that form to its own ``:`` list on startup.  Joining with
    ``:`` on Windows yields one unusable entry, so stubs are never found.
    """
    current = os.environ.get("PATH", "") if path is None else path
    return f"{directory}{os.pathsep}{current}" if current else str(directory)


def write_shell_script(path: Path, text: str) -> None:
    """Write an executable shell stub with LF line endings on every OS."""
    path.write_text(text, encoding="utf-8", newline="\n")
    path.chmod(0o755)
