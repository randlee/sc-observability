#!/usr/bin/env python3
"""Validate an immutable sc-lint source receipt and retain its full smoke output."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import sys


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--revision", required=True)
    parser.add_argument("--receipt", type=Path, default=Path(".sc-lint/source-install.json"))
    parser.add_argument("--smoke-output", type=Path, default=Path("sc-lint-smoke.json"))
    parser.add_argument("--smoke-stderr", type=Path, default=Path("sc-lint-smoke.stderr"))
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    if not re.fullmatch(r"[0-9a-f]{40}", args.revision):
        raise SystemExit("expected a full lowercase source revision")
    receipt = json.loads(args.receipt.read_text(encoding="utf-8"))
    if receipt.get("source_revision") != args.revision:
        raise SystemExit("source receipt revision mismatch")
    if receipt.get("repository") != "randlee/sc-lint":
        raise SystemExit("source receipt repository mismatch")
    version = receipt.get("version")
    if not isinstance(version, str) or not re.fullmatch(r"\d+\.\d+\.\d+", version):
        raise SystemExit("source receipt version is invalid")
    binaries = receipt.get("binaries")
    if not isinstance(binaries, dict) or set(binaries) != {
        "sc-lint", "sc-lint-boundary", "sc-lint-portability", "sc-lint-runtime"
    }:
        raise SystemExit("source receipt sibling binary set is incomplete")
    if any(not re.fullmatch(r"[0-9a-f]{64}", value) for value in binaries.values()):
        raise SystemExit("source receipt contains an invalid binary digest")
    wheel = receipt.get("wheel_sha256")
    if not isinstance(wheel, str) or not re.fullmatch(r"[0-9a-f]{64}", wheel):
        raise SystemExit("source receipt wheel digest is invalid")
    wheel_path = Path(receipt.get("wheel", ""))
    if not wheel_path.is_file() or hashlib.sha256(wheel_path.read_bytes()).hexdigest() != wheel:
        raise SystemExit("source receipt wheel digest does not match the retained wheel")
    binary_directory = Path(receipt.get("binary_directory", ""))
    if not binary_directory.is_dir():
        raise SystemExit("source receipt binary directory is missing")
    for name, expected_digest in binaries.items():
        binary_path = binary_directory / name
        if not binary_path.is_file():
            raise SystemExit(f"source receipt binary is missing: {name}")
        if hashlib.sha256(binary_path.read_bytes()).hexdigest() != expected_digest:
            raise SystemExit(f"source receipt binary digest does not match: {name}")
    python_path = Path(receipt.get("python", ""))
    if not python_path.is_file():
        raise SystemExit("source receipt Python interpreter is missing")
    cli = Path(os.environ.get("SC_LINT_BIN", ""))
    if not cli.is_file():
        raise SystemExit("SC_LINT_BIN is missing")
    version_result = subprocess.run(
        [str(cli), "version", "--json"], check=True, capture_output=True, text=True, timeout=30
    )
    version_value = json.loads(version_result.stdout)
    data = version_value.get("data", {})
    if version_value.get("ok") is not True or data.get("version") != version or data.get("status") != "pass":
        raise SystemExit("source CLI version receipt mismatch")
    smoke = subprocess.run(
        [sys.executable, ".github/scripts/setup_sc_lint_source.py", "smoke"],
        capture_output=True, text=True, timeout=120,
    )
    args.smoke_output.write_text(smoke.stdout, encoding="utf-8")
    args.smoke_stderr.write_text(smoke.stderr, encoding="utf-8")
    print("--- sc-lint smoke stdout ---")
    print(smoke.stdout, end="")
    if smoke.stderr:
        print("--- sc-lint smoke stderr ---")
        print(smoke.stderr, end="")
    if smoke.returncode:
        raise SystemExit(f"source smoke command failed with {smoke.returncode}")
    value, _ = json.JSONDecoder().raw_decode(smoke.stdout)
    smoke_data = value.get("data", {})
    if value.get("ok") is not True or value.get("error", {}).get("code") == "CLI.CONFIG_ERROR":
        raise SystemExit("source smoke did not establish root discovery")
    findings = smoke_data.get("findings", [])
    if not isinstance(findings, list):
        raise SystemExit("source smoke findings are not an array")
    print(json.dumps({
        "source_revision": args.revision,
        "version": version,
        "status": smoke_data.get("status"),
        "finding_count": len(findings),
        "ok": value.get("ok"),
        "wheel_sha256": wheel,
        "binary_sha256": hashlib.sha256(cli.read_bytes()).hexdigest(),
    }, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
