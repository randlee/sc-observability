#!/usr/bin/env python3
"""Discover Cargo workspaces from Git's file inventory and run a CI category."""
from __future__ import annotations

import argparse
import fnmatch
from pathlib import Path
import subprocess
import tomllib

# Documentation examples and deliberate validator fixtures are not product workspaces.
EXCLUDED = ("docs/plans/", "scripts/ci/fixtures/")


def discover(root: Path) -> list[Path]:
    """Include standalone workspaces and packages excluded from the root workspace."""
    inventory = subprocess.check_output(
        ["git", "ls-files", "--cached", "--others", "--exclude-standard", "-z"],
        cwd=root,
    ).decode().split("\0")
    root_manifest = root / "Cargo.toml"
    excluded_packages = tomllib.loads(root_manifest.read_text())["workspace"].get("exclude", [])
    manifests = {Path("Cargo.toml")}
    for name in inventory:
        path = Path(name)
        if path.name != "Cargo.toml" or name.startswith(EXCLUDED):
            continue
        if not (root / path).is_file():
            continue
        content = tomllib.loads((root / path).read_text())
        excluded = any(fnmatch.fnmatchcase(path.parent.as_posix(), pattern.rstrip("/"))
                       for pattern in excluded_packages)
        if "workspace" in content or excluded:
            manifests.add(path)
    return sorted(manifests, key=lambda path: (path != Path("Cargo.toml"), path.as_posix()))


def commands(category: str, manifest: Path) -> list[list[str]]:
    selection = ["--manifest-path", str(manifest)]
    fmt = ["cargo", "fmt", *selection, "--all", "--check"]
    clippy = ["cargo", "clippy", *selection, "--locked", "--workspace",
              "--all-targets", "--all-features", "--", "-D", "warnings"]
    tests = ["cargo", "test", *selection, "--locked", "--workspace",
             "--all-targets", "--no-fail-fast"]
    # Default features first, then all features so feature-gated tests run too.
    all_feature_tests = [*tests, "--all-features"]
    return {"fmt": [fmt], "clippy": [clippy], "lint": [fmt, clippy],
            "unit": [tests, all_feature_tests]}[category]


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("category", choices=("list", "fmt", "clippy", "lint", "unit"))
    parser.add_argument("--root", type=Path, default=Path(__file__).resolve().parents[2])
    args = parser.parse_args()
    manifests = discover(args.root)
    failed = []
    for manifest in manifests:
        print(f"workspace: {manifest}", flush=True)
        if args.category == "list":
            continue
        for command in commands(args.category, manifest):
            print("+ " + " ".join(command), flush=True)
            result = subprocess.run(command, cwd=args.root, check=False)
            if result.returncode:
                failed.append((str(manifest), command[1], result.returncode))
    for manifest, category, code in failed:
        print(f"FAILED {manifest}: {category} exit {code}", flush=True)
    return int(bool(failed))


if __name__ == "__main__":
    raise SystemExit(main())
