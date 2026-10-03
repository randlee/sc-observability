"""Shared pytest setup for vendored GitHub-script tests."""

from __future__ import annotations

import sys
from pathlib import Path


TESTS = Path(__file__).parent
if str(TESTS) not in sys.path:
    sys.path.insert(0, str(TESTS))
