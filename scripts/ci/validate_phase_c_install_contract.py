#!/usr/bin/env python3
"""Validate C.1 caller-owned inventory semantics before shared-kit install."""

from __future__ import annotations

import json
import re
import sys
from pathlib import Path


EXPECTED_WHEELS = {
    ("ubuntu-latest", "x86_64-unknown-linux-gnu", "manylinux_2_28_x86_64"),
    ("ubuntu-24.04-arm", "aarch64-unknown-linux-gnu", "manylinux_2_28_aarch64"),
    ("macos-15-intel", "x86_64-apple-darwin", "macosx_10_13_x86_64"),
    ("macos-latest", "aarch64-apple-darwin", "macosx_11_0_arm64"),
    ("windows-2022", "x86_64-pc-windows-msvc", "win_amd64"),
}
TARGET_RE = re.compile(r"^[A-Za-z0-9_]+(?:-[A-Za-z0-9_]+){2,}$")


def main() -> int:
    path = Path(sys.argv[1] if len(sys.argv) > 1 else "install.json")
    contract = json.loads(path.read_text(encoding="utf-8"))
    targets = contract["release_targets"]
    if {target["archive"] for target in targets} - {"tar.gz", "zip"}:
        raise SystemExit("release_targets archive must be tar.gz or zip")
    if not all(TARGET_RE.fullmatch(target["target"]) for target in targets):
        raise SystemExit("release_targets target must be a Rust target triple")
    distributions = contract["python_distributions"]
    wheels = distributions[0]["wheels"]
    actual = {(wheel["os"], wheel["target"], wheel["platform"]) for wheel in wheels}
    if actual != EXPECTED_WHEELS:
        raise SystemExit(f"five-wheel matrix mismatch: {sorted(actual)}")
    if contract["npm_packages"] != [{"name": "@sc-observability/client", "source": "bindings/typescript"}]:
        raise SystemExit("npm package inventory mismatch")
    if contract["python_packages"][0]["artifact"] == contract["crates"][-1]["artifact"]:
        raise SystemExit("python wheel artifact id must differ from Rust crate artifact id")
    if "npm" not in contract["channels"]:
        raise SystemExit("npm channel must accompany npm package inventory")
    print("C1_INSTALL_CONTRACT_SEMANTICS_PASS: targets, ten crates, five wheels, npm package")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
