"""Give each Cargo profile a pristine copy of the same verified source inputs."""
from __future__ import annotations

import hashlib
import shutil
from pathlib import Path


def inventory(root: Path) -> dict[str, str]:
    files = {}
    for path in sorted(root.rglob('*')):
        if path.is_symlink():
            raise ValueError(f'linked build input: {path}')
        if path.is_file():
            files[path.relative_to(root).as_posix()] = hashlib.sha256(path.read_bytes()).hexdigest()
    return files


def verify_inputs(root: Path, expected: dict[str, str]) -> None:
    actual = inventory(root)
    changed = sorted(path for path in actual.keys() | expected.keys()
                     if actual.get(path) != expected.get(path))
    if changed:
        raise ValueError(f'pristine build input drift: {changed[:10]}')


def materialize(source: Path, destination: Path, expected: dict[str, str]) -> None:
    """Never copy a prior build tree or rewrite its registry checksums."""
    verify_inputs(source, expected)
    shutil.copytree(source, destination)
    verify_inputs(destination, expected)
