#!/usr/bin/env python3
"""Authoritative B.P2 command: auto-stage a clean source when --stage is omitted."""

from __future__ import annotations

import argparse
import subprocess
import sys
import tempfile
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
PREPARE = ROOT / "scripts" / "ci" / "prepare_runtime_level_staged_packages.py"
VALIDATE = ROOT / "scripts" / "ci" / "validate_runtime_level_registry_consumer.py"


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--version", required=True)
    parser.add_argument("--stage", type=Path)
    parser.add_argument("--result-file", type=Path)
    parser.add_argument("--platform", choices=("macos", "ubuntu", "windows"))
    args = parser.parse_args()
    if args.stage:
        command = [sys.executable, str(VALIDATE), "--mode", "staged", "--version", args.version, "--stage", str(args.stage.resolve())]
        if args.result_file:
            command.extend(["--result-file", str(args.result_file)])
        if args.platform:
            command.extend(["--platform", args.platform])
        return subprocess.call(command)
    with tempfile.TemporaryDirectory(prefix="bp2-runtime-stage-") as temporary:
        stage = Path(temporary) / "stage"
        subprocess.run([sys.executable, str(PREPARE), "--source", str(ROOT), "--version", args.version, "--output", str(stage)], check=True)
        command = [sys.executable, str(VALIDATE), "--mode", "staged", "--version", args.version, "--stage", str(stage)]
        if args.result_file:
            command.extend(["--result-file", str(args.result_file)])
        if args.platform:
            command.extend(["--platform", args.platform])
        return subprocess.call(command)


if __name__ == "__main__":
    raise SystemExit(main())
