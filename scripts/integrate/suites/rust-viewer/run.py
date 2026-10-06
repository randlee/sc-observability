#!/usr/bin/env python3
"""Retired Rust viewer qualification entrypoint.

The Rust ``full_stack_integration`` target was removed because it covered only
the 1.x facade. The canonical ``canonical_ingress`` tests exercise a separate
in-process collector and do not send data to the pinned desktop viewer, so they
cannot replace this viewer readback qualification. The suite is excluded from
integration dispatch until a canonical viewer-readback test is available.
"""

from __future__ import annotations

import argparse
import json
from pathlib import Path


def run(source_sha: str, output: Path) -> dict[str, object]:
    """Write an explicit skip receipt instead of invoking a deleted test."""
    if len(source_sha) != 40 or any(char not in "0123456789abcdef" for char in source_sha.lower()):
        raise ValueError("source-sha must be a full 40-hex commit")
    output.mkdir(parents=True, exist_ok=True)
    result: dict[str, object] = {
        "schema_version": 1,
        "status": "skipped",
        "source_commit": source_sha.lower(),
        "reason": (
            "full_stack_integration covered only the 1.x facade and was removed; "
            "canonical_ingress uses a local collector and cannot qualify pinned desktop-viewer readback"
        ),
    }
    (output / "result.json").write_text(
        json.dumps(result, indent=2, sort_keys=True) + "\n", encoding="utf-8"
    )
    return result


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source-sha", required=True)
    parser.add_argument("--output-dir", required=True, type=Path)
    args = parser.parse_args(argv)
    result = run(args.source_sha, args.output_dir.expanduser().resolve())
    print(json.dumps(result, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
