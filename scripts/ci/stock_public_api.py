#!/usr/bin/env python3
"""Capture and check native ``cargo public-api`` text snapshots.

This deliberately has no renderer, row projection, feature matrix, or private
snapshot format. ``setup`` is the potentially-building candidate setup: it
writes native text only below its caller-owned target directory. ``release-cut``
is the *separate explicit action* permitted to update committed baselines.
``check`` accepts only the rustdoc JSON emitted by setup and reads it with the
stock tool; it never calls Cargo with a package or manifest and therefore
cannot build a candidate crate.

Text equality is an audit of the generated native surface.  Release
compatibility itself remains the stock ``cargo public-api diff --deny changed
--deny removed`` gate: equality intentionally notices additions while that
semantic gate permits them.
"""
from __future__ import annotations

import argparse
import json
import subprocess
import sys
import time
import tomllib
from dataclasses import dataclass
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
TOOLCHAIN = (ROOT / "scripts/ci/public-api-toolchain").read_text(encoding="utf-8").strip()
BASELINES = ROOT / "schema/api/rust-stock"


class SnapshotError(RuntimeError):
    pass


@dataclass(frozen=True)
class Package:
    name: str
    manifest: Path
    lib_name: str
    version: str

    @property
    def baseline(self) -> Path:
        return BASELINES / self.name / f"{self.version}.txt"


def command(*args: str) -> list[str]:
    return ["cargo", f"+{TOOLCHAIN}", "public-api", *args, "-sss", "--color", "never"]


def run(args: list[str]) -> subprocess.CompletedProcess[str]:
    return subprocess.run(args, cwd=ROOT, text=True, capture_output=True)


def published_packages(root: Path = ROOT) -> list[Package]:
    manifest = tomllib.loads((root / "release/publish-artifacts.toml").read_text(encoding="utf-8"))
    workspace = tomllib.loads((root / "Cargo.toml").read_text(encoding="utf-8"))["workspace"]["package"]
    packages: list[Package] = []
    for item in manifest.get("crates", []):
        if item.get("publish") is not True:
            continue
        path = root / item["cargo_toml"]
        cargo = tomllib.loads(path.read_text(encoding="utf-8"))
        package = cargo["package"]
        if package["name"] != item["package"]:
            raise SnapshotError(f"publish manifest package mismatch for {path}")
        version = package["version"]
        if isinstance(version, dict):
            version = workspace["version"]
        packages.append(Package(package["name"], path, cargo.get("lib", {}).get("name", package["name"].replace("-", "_")), str(version)))
    if len(packages) != 10 or len({package.name for package in packages}) != len(packages):
        raise SnapshotError("publish-artifacts must name exactly ten unique released Rust crates")
    return packages


def rustdoc_json(target_dir: Path, package: Package) -> Path:
    return target_dir / "doc" / f"{package.lib_name}.json"


def require_success(result: subprocess.CompletedProcess[str], label: str) -> str:
    if result.returncode:
        detail = (result.stderr or result.stdout).strip()
        raise SnapshotError(f"{label} failed (exit {result.returncode}): {detail[-4000:]}")
    if not result.stdout:
        raise SnapshotError(f"{label} produced no native public-api text")
    return result.stdout


def setup(target_dir: Path, *, write_baselines: bool = False) -> dict[str, object]:
    started = time.monotonic()
    results = []
    current_text = target_dir / "native-text"
    for package in published_packages():
        output = require_success(run(command("--target-dir", str(target_dir), "--manifest-path", str(package.manifest))), package.name)
        current_path = current_text / f"{package.name}.txt"
        current_path.parent.mkdir(parents=True, exist_ok=True)
        current_path.write_text(output, encoding="utf-8")
        if write_baselines:
            package.baseline.parent.mkdir(parents=True, exist_ok=True)
            package.baseline.write_text(output, encoding="utf-8")
        results.append({"package": package.name, "current_text": str(current_path), "lines": len(output.splitlines())})
    return {"packages": results, "setup_seconds": round(time.monotonic() - started, 3),
            "target_dir": str(target_dir), "baselines_written": write_baselines}


def check(target_dir: Path) -> dict[str, object]:
    started = time.monotonic()
    results = []
    for package in published_packages():
        current_json = rustdoc_json(target_dir, package)
        if not current_json.is_file():
            raise SnapshotError(f"{package.name}: missing current rustdoc JSON {current_json}; run setup first")
        if not package.baseline.is_file():
            raise SnapshotError(f"{package.name}: missing committed native baseline {package.baseline}")
        current = require_success(run(command("--rustdoc-json", str(current_json))), package.name)
        expected = package.baseline.read_text(encoding="utf-8")
        if current != expected:
            raise SnapshotError(
                f"{package.name}: native public-api text differs from {package.baseline}; "
                "run the semantic compatibility gate to classify additions versus breaking changes"
            )
        results.append({"package": package.name, "lines": len(current.splitlines())})
    elapsed = time.monotonic() - started
    if elapsed >= 60:
        raise SnapshotError(f"native public-api comparison exceeded 60 seconds: {elapsed:.3f}s")
    return {"packages": results, "comparison_seconds": round(elapsed, 3), "target_dir": str(target_dir)}


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("mode", choices=("setup", "release-cut", "check"))
    parser.add_argument("--target-dir", required=True, type=Path)
    args = parser.parse_args()
    report = (setup(args.target_dir, write_baselines=args.mode == "release-cut")
              if args.mode != "check" else check(args.target_dir))
    print(json.dumps(report, sort_keys=True))
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (SnapshotError, OSError, subprocess.SubprocessError, tomllib.TOMLDecodeError) as error:
        print(f"stock public-api {error}", file=sys.stderr)
        raise SystemExit(1)
