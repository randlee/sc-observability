from pathlib import Path

import pytest

from _shell import _resolve_bash_from_git


@pytest.mark.parametrize(
    "git_parts",
    [
        ("cmd", "git.exe"),
        ("bin", "git.exe"),
        ("mingw64", "bin", "git.exe"),
    ],
)
def test_resolves_bash_for_git_for_windows_layouts(
    tmp_path: Path, git_parts: tuple[str, ...]
) -> None:
    git = tmp_path / "Git" / Path(*git_parts)
    git.parent.mkdir(parents=True)
    git.touch()
    bash = tmp_path / "Git" / "bin" / "bash.exe"
    bash.parent.mkdir(parents=True, exist_ok=True)
    bash.touch()

    assert Path(_resolve_bash_from_git(git)) == bash.resolve()


def test_missing_bash_lists_every_tried_path(tmp_path: Path) -> None:
    git = tmp_path / "Git" / "mingw64" / "bin" / "git.exe"
    git.parent.mkdir(parents=True)
    git.touch()
    expected_paths = [
        tmp_path / "Git" / "mingw64" / "bin" / "bin" / "bash.exe",
        tmp_path / "Git" / "mingw64" / "bin" / "bash.exe",
        tmp_path / "Git" / "bin" / "bash.exe",
    ]

    with pytest.raises(RuntimeError) as exc_info:
        _resolve_bash_from_git(git)

    for path in expected_paths:
        assert str(path) in str(exc_info.value)
