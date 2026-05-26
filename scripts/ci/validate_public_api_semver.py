#!/usr/bin/env python3
"""Validate public API semver compatibility for published workspace crates."""

from __future__ import annotations

import json
import os
import subprocess
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
CACHE_DIR = ROOT / "target" / "public-api"
REPORT_PATH = CACHE_DIR / "public-api-semver.txt"


def ensure_repo_root() -> None:
    if not (ROOT / "Cargo.toml").is_file() or not (ROOT / "scripts" / "ci").is_dir() or not (
        ROOT / "docs"
    ).is_dir():
        raise SystemExit("ERROR: run from repo root")


def run(command: list[str], *, check: bool = True, capture_output: bool = False) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        command,
        cwd=ROOT,
        check=check,
        text=True,
        capture_output=capture_output,
    )


def ensure_cargo_subcommand(subcommand: str, crate: str) -> None:
    version_cmd = ["cargo", subcommand, "--version"]
    if subprocess.run(
        version_cmd,
        cwd=ROOT,
        check=False,
        text=True,
        capture_output=True,
    ).returncode == 0:
        return
    run(["cargo", "+stable", "install", crate, "--locked"])


def workspace_public_crates() -> list[tuple[str, str]]:
    metadata = json.loads(
        run(
            ["cargo", "metadata", "--format-version", "1", "--no-deps"],
            capture_output=True,
        ).stdout
    )
    crates: list[tuple[str, str]] = []
    for package in metadata["packages"]:
        publish = package.get("publish", None)
        if publish is False or publish == []:
            continue
        targets = package.get("targets", [])
        if not any(
            "lib" in target.get("kind", []) or "proc-macro" in target.get("kind", [])
            for target in targets
        ):
            continue
        crates.append((package["name"], package["manifest_path"]))
    return crates


def docs_api_approvals_present() -> bool:
    approvals_dir = ROOT / "docs" / "api-approvals"
    if not approvals_dir.is_dir():
        return False
    return any(path.name != "README.md" for path in approvals_dir.glob("*.md"))


def main() -> int:
    os.chdir(ROOT)
    ensure_repo_root()
    ensure_cargo_subcommand("semver-checks", "cargo-semver-checks")

    CACHE_DIR.mkdir(parents=True, exist_ok=True)
    semver_failed = False
    lines: list[str] = []

    for package, manifest_path in workspace_public_crates():
        lines.append(f"=== {package} ===")
        result = run(
            [
                "cargo",
                "semver-checks",
                "--manifest-path",
                manifest_path,
                "--release-type",
                "patch",
            ],
            check=False,
            capture_output=True,
        )
        if result.stdout:
            lines.append(result.stdout.rstrip())
        if result.stderr:
            lines.append(result.stderr.rstrip())
        if result.returncode != 0:
            semver_failed = True
        lines.append("")

    report = "\n".join(lines).rstrip() + "\n"
    REPORT_PATH.write_text(report, encoding="utf-8")
    sys.stdout.write(report)

    if not semver_failed:
        print("public API semver validation passed")
        return 0

    if docs_api_approvals_present():
        print("public API semver validation passed with approved breaking changes")
        return 0

    print("public API semver validation failed", file=sys.stderr)
    return 1


if __name__ == "__main__":
    raise SystemExit(main())
