#!/usr/bin/env python3
"""Build an immutable, local-only B.P2 package stage without publishing it."""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import shutil
import subprocess
from pathlib import Path


PACKAGES = (
    "sc-observability-types",
    "sc-observability",
    "sc-observe",
    "sc-observability-otlp",
)
VERSION_RE = re.compile(r'(?m)^version = "\d+\.\d+\.\d+"$')


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for block in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def tree_sha256(root: Path) -> str:
    digest = hashlib.sha256()
    for path in sorted(item for item in root.rglob("*") if item.is_file()):
        digest.update(path.relative_to(root).as_posix().encode())
        digest.update(sha256(path).encode())
    return digest.hexdigest()


def run(args: list[str], cwd: Path) -> None:
    subprocess.run(args, cwd=cwd, check=True)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--version", required=True)
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--source", type=Path, default=Path.cwd())
    args = parser.parse_args()
    if not re.fullmatch(r"\d+\.\d+\.\d+", args.version):
        raise SystemExit("--version must be an exact X.Y.Z candidate")

    source = args.source.resolve()
    output = args.output.resolve()
    if output.exists():
        raise SystemExit(f"refusing to overwrite existing stage: {output}")
    stage_root = output / "workspace"
    shutil.copytree(
        source,
        stage_root,
        ignore=shutil.ignore_patterns(".git", "target", ".DS_Store"),
    )
    root_toml = stage_root / "Cargo.toml"
    rendered = VERSION_RE.sub(f'version = "{args.version}"', root_toml.read_text(), count=1)
    rendered = rendered.replace('version = "1.2.0", path =', f'version = "{args.version}", path =')
    root_toml.write_text(rendered)

    package_lists = output / "package-lists"
    package_lists.mkdir(parents=True)
    for package in PACKAGES:
        result = subprocess.run(
            ["cargo", "package", "--list", "-p", package],
            cwd=stage_root,
            check=True,
            text=True,
            capture_output=True,
        )
        (package_lists / f"{package}.txt").write_text(result.stdout)

    source_sha = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=source, text=True).strip()
    evidence = {
        "schema_version": 1,
        "candidate_version": args.version,
        "source_commit": source_sha,
        "publication": "deferred_until_phase_end",
        "packages": [
            {
                "name": package,
                "staged_root": f"workspace/crates/{package}",
                "tree_sha256": tree_sha256(stage_root / "crates" / package),
                "package_file_list": f"package-lists/{package}.txt",
            }
            for package in PACKAGES
        ],
    }
    (output / "stage-manifest.json").write_text(json.dumps(evidence, indent=2) + "\n")
    print(output / "stage-manifest.json")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
