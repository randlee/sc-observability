"""Unit tests for the cross-platform Bash fixture helpers."""

from __future__ import annotations

from pathlib import Path, PureWindowsPath

from shell_helpers import bash_command, bash_environment, bash_path


def test_uses_git_bash_and_translates_windows_fixture_paths() -> None:
    assert bash_command(platform="darwin") == ["bash"]
    configured = r"C:\Program Files\Git\bin\bash.exe"
    assert bash_command(
        platform="win32",
        environ={"GIT_BASH_PATH": configured},
        exists=lambda path: PureWindowsPath(path) == PureWindowsPath(configured),
    ) == [configured]
    environment = bash_environment(
        {"PATH": r"C:\Tools;C:\Program Files\Git\bin"},
        prepend_path=r"C:\Temp Space\stub-bin",
        github_output=r"C:\Temp Space\github-output",
        platform="win32",
    )
    assert environment["PATH"] == "/c/Temp Space/stub-bin:/c/Tools:/c/Program Files/Git/bin"
    assert environment["PATH"].split(":", 1)[0] == "/c/Temp Space/stub-bin"
    assert environment["GITHUB_OUTPUT"] == "/c/Temp Space/github-output"
    assert bash_path(r"C:\Temp Space\stub-bin", platform="win32") == "/c/Temp Space/stub-bin"


def test_refuses_the_windows_wsl_shim_when_git_bash_is_missing() -> None:
    try:
        bash_command(platform="win32", environ={}, exists=lambda _path: False)
    except RuntimeError as error:
        assert "Git Bash is required" in str(error)
    else:
        raise AssertionError("Windows shell selection accepted an unconfigured bash executable")


def test_rejects_a_missing_explicit_git_bash_path() -> None:
    configured = r"C:\Missing\Git\bin\bash.exe"
    try:
        bash_command(
            platform="win32",
            environ={"GIT_BASH_PATH": configured},
            exists=lambda _path: False,
        )
    except RuntimeError as error:
        assert configured in str(error)
    else:
        raise AssertionError("missing GIT_BASH_PATH did not fail closed")


def test_uses_ordered_system_then_per_user_git_bash_candidates() -> None:
    environment = {
        "ProgramW6432": r"C:\Program Files",
        "ProgramFiles": r"D:\Program Files",
        "ProgramFiles(x86)": r"E:\Program Files (x86)",
        "LOCALAPPDATA": r"F:\Users\tester\AppData\Local",
    }
    first = Path(environment["ProgramW6432"]) / "Git" / "bin" / "bash.exe"
    selected = bash_command(
        platform="win32",
        environ=environment,
        exists=lambda path: PureWindowsPath(path) == PureWindowsPath(first),
    )
    assert selected == [str(first)]

    per_user = Path(environment["LOCALAPPDATA"]) / "Programs" / "Git" / "bin" / "bash.exe"
    selected = bash_command(
        platform="win32",
        environ=environment,
        exists=lambda path: PureWindowsPath(path) == PureWindowsPath(per_user),
    )
    assert selected == [str(per_user)]


def test_rejects_unc_paths_and_normalizes_path_key_case() -> None:
    try:
        bash_path(r"\\server\share\fixture", platform="win32")
    except ValueError as error:
        assert "UNC" in str(error)
    else:
        raise AssertionError("UNC fixture path was accepted")

    environment = bash_environment({"Path": r"C:\Tools"}, platform="win32")
    assert environment["PATH"] == "/c/Tools"
    assert "Path" not in environment
