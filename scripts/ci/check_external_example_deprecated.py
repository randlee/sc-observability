#!/usr/bin/env python3
"""Temporary parity gate until obs-d-18 removes the deprecated legacy types."""

from __future__ import annotations

import re
import sys
import tomllib
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
DENY_DEPRECATED = re.compile(
    r"#!\s*\[\s*deny\s*\([^]]*\bdeprecated\b[^]]*\)\s*\]"
)
ALLOW_DEPRECATED = re.compile(
    r"#\s*!?\s*\[\s*allow\s*\([^]]*\bdeprecated\b"
)


def workspace_members() -> set[Path]:
    with (ROOT / "Cargo.toml").open("rb") as manifest_file:
        workspace = tomllib.load(manifest_file)["workspace"]
    members = set()
    for pattern in workspace.get("members", []):
        members.update(path.resolve() for path in ROOT.glob(pattern))
    return members


def crate_roots(package: Path) -> list[Path]:
    source = package / "src"
    roots = [source / name for name in ("main.rs", "lib.rs")]
    roots.extend((source / "bin").rglob("*.rs"))
    return [path for path in roots if path.is_file()]


def main() -> int:
    members = workspace_members()
    manifests = sorted((ROOT / "examples").rglob("Cargo.toml"))
    external = [manifest.parent for manifest in manifests if manifest.parent.resolve() not in members]
    if not external:
        print("error: no workspace-external example packages found", file=sys.stderr)
        return 1

    errors = []
    for package in external:
        roots = crate_roots(package)
        if not roots:
            errors.append(f"{package.relative_to(ROOT)}: no Rust crate root found")
            continue
        for root in roots:
            if not DENY_DEPRECATED.search(root.read_text(encoding="utf-8")):
                errors.append(
                    f"{root.relative_to(ROOT)}: missing #![deny(deprecated)]"
                )
        for source in package.rglob("*.rs"):
            if "target" in source.parts:
                continue
            if ALLOW_DEPRECATED.search(source.read_text(encoding="utf-8")):
                errors.append(
                    f"{source.relative_to(ROOT)}: #[allow(deprecated)] is forbidden"
                )
        print(f"checked deprecated policy: {package.relative_to(ROOT)}")

    if errors:
        print("deprecated lint policy violations:", file=sys.stderr)
        print("\n".join(f"- {error}" for error in errors), file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
