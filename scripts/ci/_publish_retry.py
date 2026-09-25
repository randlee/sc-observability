"""Small, testable command-shape helpers for the installed publish workflows.

These helpers model only repository-owned orchestration decisions.  They do
not pretend to re-test duplicate handling implemented inside Cargo, maturin,
or the GitHub CLI; those tools' own behavior remains an upstream concern.
"""
from __future__ import annotations

import subprocess
from collections.abc import Callable, Sequence


Runner = Callable[..., subprocess.CompletedProcess[str]]


def invoke(command: Sequence[str], runner: Runner | None = None) -> None:
    """Run one already-constructed command and map its exit code to outcome."""
    if runner is None:
        runner = subprocess.run
    result = runner(list(command), check=False, capture_output=True, text=True)
    if result.returncode:
        raise RuntimeError(f"publish orchestration command failed: {command[0]}")


def pypi_invocation(repository: str, artifacts: Sequence[str]) -> list[str]:
    return ["maturin", "upload", "--repository", repository, "--non-interactive", "--skip-existing", *artifacts]


def crates_io_invocation(manifest: str) -> list[str]:
    return ["cargo", "publish", "--manifest-path", manifest, "--locked"]


def github_release_probe_invocation(repository: str, tag: str) -> list[str]:
    return ["gh", "release", "view", tag, "--repo", repository, "--json", "isDraft,assets"]
