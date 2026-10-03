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


def write_crlf_jq(bin_dir: Path) -> Path:
    """Install a ``jq`` that emits CRLF for ``-r`` output, like native jq.exe.

    The real jq runs underneath; only raw-output lines gain a CR, which is what
    a Windows runner hands to workflow ``read`` loops.  Tests use it on every
    OS so a loop that forgets to strip CR fails everywhere, not only on Windows.
    """
    real = shutil.which("jq")
    if real is None:
        raise RuntimeError("jq was not found on PATH; workflow shell tests need it")
    bin_dir.mkdir(parents=True, exist_ok=True)
    shim = bin_dir / "jq"
    write_shell_script(
        shim,
        "#!/usr/bin/env bash\n"
        "set -o pipefail\n"
        f"real='{Path(real).as_posix()}'\n"
        "for argument in \"$@\"; do\n"
        "  if [[ \"$argument\" == -r ]]; then\n"
        "    \"$real\" \"$@\" | awk '{ printf \"%s\\r\\n\", $0 }'\n"
        "    exit $?\n"
        "  fi\n"
        "done\n"
        "exec \"$real\" \"$@\"\n",
    )
    return shim


# Sourced by every non-interactive bash through BASH_ENV, i.e. after Git for
# Windows' bin/bash.exe launcher has prepended its own mingw64/bin and usr/bin
# (which hold the real git.exe) to PATH.  cygpath turns the native stub path
# into the POSIX form that bash's ':'-separated PATH needs.
_STUB_FIRST_HOOK = (
    'sc_test_stub_bin="$SC_TEST_STUB_BIN"\n'
    "if command -v cygpath >/dev/null 2>&1; then\n"
    '  sc_test_stub_bin="$(cygpath -u "$sc_test_stub_bin")"\n'
    "fi\n"
    'PATH="$sc_test_stub_bin:$PATH"\n'
    "unset sc_test_stub_bin\n"
)


def stub_first_env(bin_dir: Path, base: dict[str, str] | None = None) -> dict[str, str]:
    """Return an environment in which bash resolves commands from ``bin_dir`` first.

    Prepending ``bin_dir`` to PATH is not enough on Windows: the Git for
    Windows ``bin/bash.exe`` launcher prepends its own tool directories, so a
    stub named after a tool it ships (``git``) loses to the real binary.  The
    BASH_ENV hook re-prepends ``bin_dir`` inside bash, after the launcher.
    """
    hook = bin_dir.with_name(f"{bin_dir.name}-stub-first.sh")
    hook.write_text(_STUB_FIRST_HOOK, encoding="utf-8", newline="\n")
    environment = dict(os.environ if base is None else base)
    environment.update(
        {
            "PATH": prepend_path(bin_dir, environment.get("PATH", "")),
            "SC_TEST_STUB_BIN": bin_dir.as_posix(),
            "BASH_ENV": hook.as_posix(),
        }
    )
    return environment


def write_tool_dir_prepending_bash(directory: Path, tool: str) -> list[str]:
    """Return a bash command that shadows ``tool`` the way bin/bash.exe does.

    The launcher puts a directory holding a failing ``tool`` ahead of the
    caller's PATH and then runs bash, reproducing on every OS the Git for
    Windows condition that hides PATH-prepended stubs.
    """
    shadow = directory / "launcher-tools"
    shadow.mkdir(parents=True, exist_ok=True)
    write_shell_script(
        shadow / tool,
        f"#!/usr/bin/env bash\necho 'launcher-prepended {tool} ran instead of the stub' >&2\nexit 97\n",
    )
    launcher = directory / "tool-dir-prepending-bash"
    write_shell_script(
        launcher,
        "#!/usr/bin/env bash\n"
        f"shadow='{shadow.as_posix()}'\n"
        'if command -v cygpath >/dev/null 2>&1; then shadow="$(cygpath -u "$shadow")"; fi\n'
        'PATH="$shadow:$PATH"\n'
        'exec bash "$@"\n',
    )
    return [BASH, str(launcher)]
