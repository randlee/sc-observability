"""Resolve the bash executable tests must spawn.

On Windows a bare "bash" resolves to the WSL stub in System32 before PATH is
consulted, so use the bash that ships with Git for Windows.
"""

from __future__ import annotations

import os
import shutil
from pathlib import Path


def _resolve_bash() -> str:
    if os.name != "nt":
        return "bash"
    git = shutil.which("git")
    if git is None:
        raise RuntimeError("git was not found on PATH; cannot locate Git for Windows bash.exe")
    bash = Path(git).resolve().parent.parent / "bin" / "bash.exe"
    if not bash.is_file():
        raise RuntimeError(f"Git for Windows bash.exe not found at {bash}")
    return str(bash)


BASH = _resolve_bash()
