#!/usr/bin/env python3
"""Negative coverage for B.P2 current and frozen version declarations."""

from __future__ import annotations

import sys
import tempfile
import unittest
from pathlib import Path


SCRIPTS = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(SCRIPTS))
from validate_runtime_level_qualification_metadata import (  # noqa: E402
    validate_version_declarations,
    validate_workspace_member_roster,
)


STAGED_PACKAGES = ("sc-observability-types", "sc-observability", "sc-observe", "sc-observability-otlp")
STAGED_MEMBERS = tuple(f"crates/{package}" for package in STAGED_PACKAGES)
COMPANION_MEMBERS = (
    "crates/sc-observability-log",
    "crates/sc-observability-log-macros",
    "crates/sc-observability-log-consumer-check",
    "crates/sc-observability-dto",
    "crates/sc-observability-binding-runtime",
    "bindings/python/sc-observability-py",
    "examples/rust-python-logging",
)


class WorkspaceMemberRosterTests(unittest.TestCase):
    def test_accepts_staged_only_roster(self) -> None:
        validate_workspace_member_roster(STAGED_MEMBERS, STAGED_PACKAGES)

    def test_accepts_companions_interleaved_after_staged_order_preserved(self) -> None:
        validate_workspace_member_roster(STAGED_MEMBERS + COMPANION_MEMBERS, STAGED_PACKAGES)

    def test_rejects_staged_members_out_of_order(self) -> None:
        reordered = (STAGED_MEMBERS[1], STAGED_MEMBERS[0]) + STAGED_MEMBERS[2:] + COMPANION_MEMBERS
        with self.assertRaisesRegex(SystemExit, "workspace member order"):
            validate_workspace_member_roster(reordered, STAGED_PACKAGES)

    def test_rejects_companion_interleaved_between_staged_members(self) -> None:
        interleaved = (STAGED_MEMBERS[0], COMPANION_MEMBERS[0]) + STAGED_MEMBERS[1:]
        # companion between staged[0] and staged[1] does not disturb the staged subsequence itself
        validate_workspace_member_roster(interleaved, STAGED_PACKAGES)

    def test_rejects_missing_staged_member(self) -> None:
        with self.assertRaisesRegex(SystemExit, "workspace member order"):
            validate_workspace_member_roster(STAGED_MEMBERS[:-1] + COMPANION_MEMBERS, STAGED_PACKAGES)

    def test_rejects_unrecognized_extra_member(self) -> None:
        with self.assertRaisesRegex(SystemExit, "unrecognized members"):
            validate_workspace_member_roster(STAGED_MEMBERS + ("crates/some-unreviewed-crate",), STAGED_PACKAGES)

    def test_rejects_private_only_companion_staged_for_publish(self) -> None:
        packages_with_leak = STAGED_PACKAGES + ("sc-observability-log-consumer-check",)
        with self.assertRaisesRegex(SystemExit, "unpublished companion package roster overlaps"):
            validate_workspace_member_roster(STAGED_MEMBERS + COMPANION_MEMBERS, packages_with_leak)


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
