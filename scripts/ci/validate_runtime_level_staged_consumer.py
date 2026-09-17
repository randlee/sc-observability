#!/usr/bin/env python3
"""Compatibility entry point for B.P2's explicit staged-package consumer gate."""

from __future__ import annotations

import subprocess
import sys


if __name__ == "__main__":
    raise SystemExit(
        subprocess.call(
            [
                sys.executable,
                "scripts/ci/validate_runtime_level_registry_consumer.py",
                "--mode",
                "staged",
                *sys.argv[1:],
            ]
        )
    )
