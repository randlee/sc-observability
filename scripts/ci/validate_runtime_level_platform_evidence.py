#!/usr/bin/env python3
"""Reject incomplete B.P2 platform qualification evidence."""

from __future__ import annotations

import argparse
import json
from pathlib import Path

from _runtime_level_common import PLATFORM_ASSERTIONS, PLATFORMS


def validate_evidence(evidence_dir: Path) -> None:
    """Reject partial, failed, or non-identical three-platform qualifications."""
    expected_assertions = set(PLATFORM_ASSERTIONS)
    missing = []
    common_candidate = None
    for platform in PLATFORMS:
        path = evidence_dir / f"{platform}.json"
        if not path.is_file():
            missing.append(platform)
            continue
        result = json.loads(path.read_text())
        candidate = result.get("candidate", {})
        archives = candidate.get("archives")
        identity = (
            candidate.get("version"),
            candidate.get("source_commit"),
            tuple(sorted(archives.items())) if isinstance(archives, dict) else (),
        )
        if result.get("status") != "passed":
            raise SystemExit(f"aggregate script rejected non-passing {platform} result: {path}")
        if result.get("platform") != platform:
            raise SystemExit(f"aggregate script rejected mismatched platform label: {path}")
        if set(result.get("assertions", [])) != expected_assertions:
            raise SystemExit(f"aggregate script rejected skipped or failed assertions: {path}")
        if not all(identity):
            raise SystemExit(f"aggregate script rejected incomplete candidate provenance: {path}")
        if common_candidate is None:
            common_candidate = identity
        elif identity != common_candidate:
            raise SystemExit(f"aggregate script rejected mismatched candidate provenance: {path}")
    if missing:
        raise SystemExit(f"aggregate script rejected missing platform results: {', '.join(missing)}")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--evidence-dir", required=True, type=Path)
    args = parser.parse_args()
    validate_evidence(args.evidence_dir)
    print("B.P2 platform evidence complete: macos, ubuntu, windows")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
