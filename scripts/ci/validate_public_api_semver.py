#!/usr/bin/env python3
"""Run crate-specific semver policy; no approval can waive a failed check."""
import subprocess
import sys
from pathlib import Path

if __name__ == '__main__':
    root = Path(__file__).resolve().parents[2]
    for subcommand, package in (('semver-checks', 'cargo-semver-checks'), ('public-api', 'cargo-public-api')):
        if subprocess.run(['cargo', subcommand, '--version'], capture_output=True).returncode:
            subprocess.run(['cargo', '+stable', 'install', package, '--locked'], check=True)
    raise SystemExit(subprocess.call([sys.executable, str(root / 'scripts/ci/validate_public_api.py'), 'semver'], cwd=root))
