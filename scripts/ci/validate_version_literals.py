#!/usr/bin/env python3
import re
import tomllib
from collections import defaultdict
from pathlib import Path


root = Path(".")
workspace = tomllib.loads((root / "Cargo.toml").read_text(encoding="utf-8"))
workspace_version = workspace["workspace"]["package"]["version"]

version_pattern = re.compile(r"(?<!\d)(\d+\.\d+\.\d+)(?!\d)")
text_suffixes = {".toml", ".md", ".yml", ".yaml", ".txt"}
skip_dirs = {".git", "target", ".claude", ".prompts"}
skip_files = {"Cargo.lock"}

occurrences: dict[str, list[tuple[str, int, str]]] = defaultdict(list)

for path in root.rglob("*"):
    if not path.is_file():
        continue
    if any(part in skip_dirs for part in path.parts):
        continue
    if path.name in skip_files:
        continue
    if path.suffix not in text_suffixes and path.name != "PUBLISHING.md":
        continue

    text = path.read_text(encoding="utf-8")
    for line_no, line in enumerate(text.splitlines(), start=1):
        for match in version_pattern.finditer(line):
            occurrences[match.group(1)].append(
                (path.relative_to(root).as_posix(), line_no, line.strip())
            )

violations = []
for version, hits in sorted(occurrences.items()):
    files = {path for path, _, _ in hits}
    if len(files) <= 1:
        continue
    if version == workspace_version:
        continue
    rendered_hits = ", ".join(f"{path}:{line_no}" for path, line_no, _ in hits)
    violations.append(
        f"duplicated plain-text version {version!r} appears in multiple files "
        f"but does not match workspace.package.version {workspace_version!r}: {rendered_hits}"
    )

if violations:
    raise SystemExit("\n".join(violations))

print(
    "version literal validation passed "
    f"(workspace.package.version={workspace_version})"
)
