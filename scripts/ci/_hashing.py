"""Shared byte-exact hashing helpers for CI evidence validators."""

from __future__ import annotations

import hashlib
from pathlib import Path


def digest(path: Path) -> str:
    """Return the lowercase SHA-256 digest of a file's exact bytes."""
    return hashlib.sha256(path.read_bytes()).hexdigest()
