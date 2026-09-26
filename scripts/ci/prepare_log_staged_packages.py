#!/usr/bin/env python3
"""Run Cargo's full multi-package preflight and retain immutable B.2 candidates."""
from __future__ import annotations

import argparse
import json
import shutil
import subprocess
import sys
import tomllib
from pathlib import Path

from _log_staging import PACKAGES, PRIVATE_PACKAGE, inspect_archive, sha256, verify_stage

ROOT = Path(__file__).resolve().parents[2]


def package_command(metadata: dict, build: Path) -> list[str]:
    """Select only B.2's public packages, even in a later expanded workspace."""
    members = set(metadata["workspace_members"])
    packages = {p["name"]: p for p in metadata["packages"] if p["id"] in members}
    for name in PACKAGES:
        if name not in packages or packages[name].get("publish") == []:
            raise ValueError(f"B.2 selected package missing or private: {name}")
    if PRIVATE_PACKAGE not in packages or packages[PRIVATE_PACKAGE].get("publish") != []:
        raise ValueError("CI-only consumer must remain private")
    command = ["cargo", "package", "--locked", "--target-dir", str(build)]
    for name in PACKAGES:
        command.extend(["-p", name])
    return command


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--version", required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--source", type=Path, default=ROOT)
    parser.add_argument("--verify-existing", action="store_true")
    args = parser.parse_args()
    source, output = args.source.resolve(), args.output.resolve()
    if args.verify_existing:
        verify_stage(output, args.version)
        print(f"verified immutable six-package stage: {output}")
        return 0
    if output.exists():
        raise SystemExit(f"refusing to overwrite immutable stage: {output}")
    def git(*arguments: str) -> str:
        return subprocess.check_output(["git", *arguments], cwd=source, text=True).strip()
    if git("status", "--porcelain"):
        raise SystemExit("candidate source must be committed and clean before staging")
    source_sha = git("rev-parse", "HEAD")
    workspace = tomllib.loads((source / "Cargo.toml").read_text())
    if workspace["workspace"]["package"]["version"] != args.version:
        raise SystemExit("requested version differs from the committed workspace train")
    roster = sorted(tomllib.loads((source / "release/publish-artifacts.toml").read_text())["crates"], key=lambda x: x["publish_order"])
    if (tuple(item["package"] for item in roster if item["package"] in PACKAGES) != PACKAGES
            or any(item["package"] == PRIVATE_PACKAGE for item in roster)):
        raise SystemExit("release inventory differs from the six-package qualification order")
    metadata = json.loads(subprocess.check_output(["cargo", "metadata", "--locked", "--no-deps", "--format-version", "1"], cwd=source))
    build = source / "target" / "b2-package-build"
    command = package_command(metadata, build)
    output.mkdir(parents=True)
    with (output / "cargo-package.log").open("w") as log:
        result = subprocess.run(command, cwd=source, stdout=log, stderr=subprocess.STDOUT)
    if result.returncode:
        raise SystemExit(f"Cargo package verification failed; see {output / 'cargo-package.log'}")
    archives = output / "archives"
    archives.mkdir()
    packages = []
    for name in PACKAGES:
        archive = archives / f"{name}-{args.version}.crate"
        shutil.copyfile(build / "package" / archive.name, archive)
        inspected = inspect_archive(archive, name, args.version, source_sha)
        packages.append({"name": name, "version": args.version, "archive": archive.relative_to(output).as_posix(), "archive_sha256": sha256(archive), **inspected})
    evidence = {"schema_version": 1, "candidate_version": args.version, "source_commit": source_sha,
                "publication": "pending_B.7", "package_command": command,
                "package_log_sha256": sha256(output / "cargo-package.log"), "packages": packages}
    (output / "stage-manifest.json").write_text(json.dumps(evidence, indent=2) + "\n")
    verify_stage(output, args.version, source_sha)
    print(f"staged and verified all six packages at {source_sha}: {output}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
