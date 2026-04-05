#!/usr/bin/env python3
import re
import tomllib
from collections import defaultdict
from pathlib import Path


root = Path(".")
workspace = tomllib.loads((root / "Cargo.toml").read_text(encoding="utf-8"))
workspace_version = workspace["workspace"]["package"]["version"]

version_pattern = re.compile(r'^\s*version\s*=\s*"(\d+\.\d+\.\d+)"\s*$')
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
        if current_table not in {"package", "workspace.package"}:
            continue
        match = version_pattern.match(line)
        if match:
            occurrences[match.group(1)].append(
                (path.relative_to(root).as_posix(), line_no, stripped)
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
    files = {path for path, _, _ in hits}
    if len(files) <= 1:
        continue
    if version == workspace_version:
        continue
    rendered_hits = ", ".join(f"{path}:{line_no}" for path, line_no, _ in hits)
    violations.append(
        f"duplicated tracked version {version!r} appears in multiple files "
        f"but does not match workspace.package.version {workspace_version!r}: {rendered_hits}"
    )

if violations:
    raise SystemExit("\n".join(violations))

print(
    "version literal validation passed "
    f"(workspace.package.version={workspace_version})"
)
