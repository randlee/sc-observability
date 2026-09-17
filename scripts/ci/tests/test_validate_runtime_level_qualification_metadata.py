#!/usr/bin/env python3
"""Negative coverage for B.P2 current and frozen version declarations."""

from __future__ import annotations

import sys
import tempfile
import unittest
from pathlib import Path


SCRIPTS = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(SCRIPTS))
from validate_runtime_level_qualification_metadata import validate_version_declarations  # noqa: E402


VALUES = {"candidate_version": "1.3.0", "baseline_version": "1.2.0"}
HANDOFF = "Candidate version: `1.3.0` (the next minor after the current `1.2.0` release).\n"
FIXTURE = """[package]
version = "1.2.0"
[dependencies]
sc-observability = "=1.2.0"
sc-observability-types = "=1.2.0"
"""


class VersionDeclarationTests(unittest.TestCase):
    def write(self, directory: Path, handoff: str = HANDOFF, fixture: str = FIXTURE) -> tuple[Path, Path]:
        handoff_path, fixture_path = directory / "handoff.md", directory / "Cargo.toml"
        handoff_path.write_text(handoff)
        fixture_path.write_text(fixture)
        return handoff_path, fixture_path

    def test_accepts_authoritative_declarations(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            validate_version_declarations(VALUES, *self.write(Path(temporary)))

    def test_rejects_mismatched_handoff_candidate(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            paths = self.write(Path(temporary), handoff=HANDOFF.replace("1.3.0", "9.9.9"))
            with self.assertRaisesRegex(SystemExit, "handoff"):
                validate_version_declarations(VALUES, *paths)

    def test_rejects_mismatched_frozen_baseline(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            paths = self.write(Path(temporary), fixture=FIXTURE.replace("=1.2.0", "=9.9.9", 1))
            with self.assertRaisesRegex(SystemExit, "frozen released-baseline"):
                validate_version_declarations(VALUES, *paths)


if __name__ == "__main__":
    unittest.main()
