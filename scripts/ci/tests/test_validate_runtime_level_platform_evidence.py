#!/usr/bin/env python3
"""Focused negative/positive tests for aggregate B.P2 evidence rejection."""

from __future__ import annotations

import json
import sys
import tempfile
import unittest
from pathlib import Path


SCRIPTS = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(SCRIPTS))
from _runtime_level_common import PLATFORM_ASSERTIONS, PLATFORMS  # noqa: E402
from validate_runtime_level_platform_evidence import validate_evidence  # noqa: E402


def result(platform: str, source_commit: str = "source-a") -> dict[str, object]:
    return {
        "status": "passed",
        "platform": platform,
        "assertions": list(PLATFORM_ASSERTIONS),
        "candidate": {
            "version": "1.3.0",
            "source_commit": source_commit,
            "archives": {"sc-observability": "archive-a"},
        },
    }


class PlatformEvidenceTests(unittest.TestCase):
    def write_all(self, directory: Path) -> None:
        for platform in PLATFORMS:
            (directory / f"{platform}.json").write_text(json.dumps(result(platform)))

    def test_accepts_complete_matching_evidence(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            self.write_all(directory)
            validate_evidence(directory)

    def test_rejects_missing_platform(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            self.write_all(directory)
            (directory / "windows.json").unlink()
            with self.assertRaisesRegex(SystemExit, "missing platform"):
                validate_evidence(directory)

    def test_rejects_skipped_or_failed_assertion(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            self.write_all(directory)
            broken = result("ubuntu")
            broken["assertions"] = list(PLATFORM_ASSERTIONS[:-1])
            (directory / "ubuntu.json").write_text(json.dumps(broken))
            with self.assertRaisesRegex(SystemExit, "skipped or failed assertions"):
                validate_evidence(directory)

    def test_rejects_mismatched_candidate_provenance(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            self.write_all(directory)
            (directory / "windows.json").write_text(json.dumps(result("windows", "source-b")))
            with self.assertRaisesRegex(SystemExit, "mismatched candidate provenance"):
                validate_evidence(directory)


if __name__ == "__main__":
    unittest.main()
