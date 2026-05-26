#!/usr/bin/env python3
"""Validate tracked version literals with intentionally narrow scope.

This check enforces version consistency for:
- Cargo package tables
- internal workspace dependency version pins that point at local paths
- RELEASE-NOTES markdown files

Other documentation is not scanned by this script and is governed separately by
review/docs processes.
"""

import re
import tomllib
from collections import defaultdict
from pathlib import Path


root = Path(".")
workspace = tomllib.loads((root / "Cargo.toml").read_text(encoding="utf-8"))
workspace_version = workspace["workspace"]["package"]["version"]

version_pattern = re.compile(r'^\s*version\s*=\s*"(\d+\.\d+\.\d+)"\s*$')
workspace_dep_version_pattern = re.compile(
    r'^\s*([A-Za-z0-9_.-]+)\s*=\s*\{(?=.*\bversion\s*=\s*"(\d+\.\d+\.\d+)")(?=.*\bpath\s*=\s*"([^"]+)").*\}\s*$'
)
markdown_version_pattern = re.compile(r"(?<!\d)(\d+\.\d+\.\d+)(?!\d)")
release_notes_globs = ("**/RELEASE-NOTES*.md",)
skip_dirs = {".git", "target", ".claude", ".prompts"}

occurrences: dict[str, list[tuple[str, int, str]]] = defaultdict(list)


def iter_repo_files():
    for path in root.rglob("*"):
        if not path.is_file():
            continue
        if any(part in skip_dirs for part in path.parts):
            continue
        yield path


def collect_toml_package_versions(path: Path) -> None:
    current_table = None
    for line_no, line in enumerate(path.read_text(encoding="utf-8").splitlines(), start=1):
        stripped = line.strip()
        if stripped.startswith("[") and stripped.endswith("]"):
            current_table = stripped[1:-1].strip()
            continue
        if current_table in {"package", "workspace.package"}:
            match = version_pattern.match(line)
            if match:
                occurrences[match.group(1)].append(
                    (path.relative_to(root).as_posix(), line_no, stripped)
                )
        elif current_table == "workspace.dependencies":
            match = workspace_dep_version_pattern.match(line)
            if not match:
                continue
            dependency_name, version, dependency_path = match.groups()
            if not dependency_path.startswith("crates/"):
                continue
            occurrences[version].append(
                (
                    path.relative_to(root).as_posix(),
                    line_no,
                    f"{dependency_name} version={version} path={dependency_path}",
                )
            )


def collect_release_note_versions(path: Path) -> None:
    for line_no, line in enumerate(path.read_text(encoding="utf-8").splitlines(), start=1):
        for match in markdown_version_pattern.finditer(line):
            occurrences[match.group(1)].append(
                (path.relative_to(root).as_posix(), line_no, line.strip())
            )


release_note_files = {
    path.resolve()
    for pattern in release_notes_globs
    for path in root.glob(pattern)
    if path.is_file()
}

for path in iter_repo_files():
    if path.suffix == ".toml":
        collect_toml_package_versions(path)
    elif path.suffix == ".md" and path.resolve() in release_note_files:
        collect_release_note_versions(path)

violations = []
for version, hits in sorted(occurrences.items()):
    if version == workspace_version:
        continue
    rendered_hits = ", ".join(f"{path}:{line_no}" for path, line_no, _ in hits)
    violations.append(
        f"version literal {version!r} does not match workspace.package.version "
        f"{workspace_version!r}: {rendered_hits}"
    )

if violations:
    raise SystemExit("\n".join(violations))

print(
    "version literal validation passed "
    f"(workspace.package.version={workspace_version})"
)
