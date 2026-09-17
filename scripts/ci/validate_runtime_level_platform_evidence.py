#!/usr/bin/env python3
"""Reject incomplete B.P2 platform qualification evidence."""

from __future__ import annotations

import argparse
import json
from pathlib import Path


PLATFORMS = ("macos", "ubuntu", "windows")
ASSERTIONS = {
    "baseline_exact_resolution", "candidate_extracted_archive_resolution",
    "threshold_filtering", "log_and_query", "level_state_and_reset", "stale_owner_after_shutdown",
}


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--evidence-dir", required=True, type=Path)
    args = parser.parse_args()
    missing = []
    common_candidate = None
    for platform in PLATFORMS:
        path = args.evidence_dir / f"{platform}.json"
        if not path.is_file():
            missing.append(platform)
            continue
        result = json.loads(path.read_text())
        candidate = result.get("candidate", {})
        identity = (candidate.get("version"), candidate.get("source_commit"), tuple(sorted(candidate.get("archives", {}).items())))
        if result.get("status") != "passed" or result.get("platform") != platform or set(result.get("assertions", [])) != ASSERTIONS or not all(identity):
            raise SystemExit(f"platform evidence has skipped or failed assertions: {path}")
        if common_candidate is None:
            common_candidate = identity
        elif identity != common_candidate:
            raise SystemExit(f"platform evidence uses different candidate provenance: {path}")
    if missing:
        raise SystemExit(f"missing required platform results: {', '.join(missing)}")
    print("B.P2 platform evidence complete: macos, ubuntu, windows")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
