#!/usr/bin/env python3
"""Validate B.1's immutable BTIT-source import/acceptance contract.

`docs/plans/phase-b/import-provenance.json` pins the exact accepted BTIT
commit, its accepted B.P3 review handoff, and a per-file Git blob inventory
for the three copied crates. This script re-derives the same facts from a
live `--source-repo` checkout and from the already-copied `--destination`
tree, and fails whenever they disagree. Two independent comparisons exist
because they catch different failure modes:

- source vs provenance: catches a provenance file that was authored against
  the wrong commit, or hand-edited/stale relative to BTIT's real history.
  This must match exactly; nothing here is an allowed "adaptation" because
  BTIT's own source is never touched by this repo.
- destination vs provenance: catches a copy step that changed more than the
  declared mechanical adaptations (package metadata, dependency paths,
  relocated test/doc paths). A blob difference here is only allowed when its
  path is explicitly listed in `adaptations` with a reason.
"""

from __future__ import annotations

import argparse
import json
import re
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
DEFAULT_PROVENANCE = ROOT / "docs" / "plans" / "phase-b" / "import-provenance.json"
DEFAULT_HANDOFF = ROOT / "docs" / "plans" / "phase-b" / "handoff-b-p3.md"

IMPORT_CRATE_PREFIXES = (
    "crates/sc-observability-log",
    "crates/sc-observability-log-macros",
    "crates/sc-observability-log-consumer-check",
)

_FULL_SHA_RE = re.compile(r"^[0-9a-f]{40}$")
_ACCEPTED_SHA_RE = re.compile(r"Accepted source SHA:\s*`([0-9a-f]{40})`")
_REVIEW_DOC_RE = re.compile(r"Review document:\s*`([^`]+)`\s*at commit\s*`([0-9a-f]{40})`")
_VERDICT_RE = re.compile(r"Verdict:\s*(\S+)")
_ACCEPTANCE_RE = re.compile(r"sc-observability acceptance:\s*(\S+)")


def parse_handoff(text: str) -> dict[str, str]:
    """Extract the accepted SHA and review citation from a B.P3 handoff document."""
    verdict = _VERDICT_RE.search(text)
    acceptance = _ACCEPTANCE_RE.search(text)
    accepted_sha = _ACCEPTED_SHA_RE.search(text)
    if (
        not accepted_sha
        or not verdict
        or verdict.group(1).lower() != "accepted"
        or not acceptance
        or acceptance.group(1).lower() != "accepted"
    ):
        raise SystemExit("missing or unaccepted B.P3 handoff")
    review = _REVIEW_DOC_RE.search(text)
    if not review:
        raise SystemExit("missing review-document path/immutable commit citation in B.P3 handoff")
    return {
        "accepted_sha": accepted_sha.group(1),
        "review_path": review.group(1),
        "review_commit": review.group(2),
    }


def git_head(repo: Path) -> str:
    return subprocess.run(
        ["git", "-C", str(repo), "rev-parse", "HEAD"],
        check=True, capture_output=True, text=True,
    ).stdout.strip()


def git_blob_inventory(repo: Path, commit: str, prefixes: tuple[str, ...]) -> dict[str, str]:
    """Map every tracked file under `prefixes` at `commit` to its Git blob SHA."""
    result = subprocess.run(
        ["git", "-C", str(repo), "ls-tree", "-r", "--full-tree", commit],
        check=True, capture_output=True, text=True,
    )
    inventory: dict[str, str] = {}
    for line in result.stdout.splitlines():
        meta, path = line.split("\t", 1)
        _, kind, blob = meta.split()
        if kind != "blob":
            continue
        if any(path == prefix or path.startswith(prefix + "/") for prefix in prefixes):
            inventory[path] = blob
    return inventory


def validate_inventory_paths(paths: list[str]) -> None:
    """Reject any recorded path that escapes the three permitted crate directories."""
    for path in paths:
        parts = Path(path).parts
        escapes = path.startswith("/") or ".." in parts
        in_scope = any(path == prefix or path.startswith(prefix + "/") for prefix in IMPORT_CRATE_PREFIXES)
        if escapes or not in_scope:
            raise SystemExit(f"escaping path in import provenance inventory: {path}")


def diff_inventory(
    actual: dict[str, str],
    recorded: dict[str, str],
    *,
    tolerated: set[str] = frozenset(),
    label: str,
) -> None:
    extra = sorted(set(actual) - set(recorded))
    if extra:
        raise SystemExit(f"unexplained extra file(s) in {label} not recorded in provenance: {extra}")
    omitted = sorted(set(recorded) - set(actual))
    if omitted:
        raise SystemExit(f"omitted file(s) recorded in provenance but absent from {label}: {omitted}")
    for path, recorded_blob in recorded.items():
        actual_blob = actual[path]
        if actual_blob != recorded_blob and path not in tolerated:
            raise SystemExit(f"unexplained content difference in {label} for {path} (not a declared adaptation)")


def validate_import(provenance: dict, source_repo: Path, destination: Path, handoff_text: str) -> None:
    recorded_inventory = provenance.get("file_inventory", {})
    validate_inventory_paths(list(recorded_inventory))

    handoff = parse_handoff(handoff_text)

    source_commit = provenance.get("source_commit", "")
    if not _FULL_SHA_RE.match(source_commit):
        raise SystemExit("import-provenance.json source_commit must be a full 40-character SHA")

    if handoff["accepted_sha"] != source_commit:
        raise SystemExit("handoff and provenance source SHA mismatch")

    actual_head = git_head(source_repo)
    if actual_head != source_commit:
        raise SystemExit("review verdict covers a different source commit than the source repository HEAD")

    # Source-repo comparison is strict: BTIT's own history is never adapted,
    # so any difference here means the provenance record itself is unreliable.
    source_inventory = git_blob_inventory(source_repo, actual_head, IMPORT_CRATE_PREFIXES)
    diff_inventory(source_inventory, recorded_inventory, label="the source repository")

    # Destination comparison tolerates only the explicitly declared adaptations.
    adaptations = {item["path"] for item in provenance.get("adaptations", [])}
    destination_inventory = {
        path: subprocess.run(
            ["git", "hash-object", str(destination / path)],
            check=True, capture_output=True, text=True,
        ).stdout.strip()
        for path in recorded_inventory
        if (destination / path).is_file()
    }
    missing = set(recorded_inventory) - set(destination_inventory)
    if missing:
        raise SystemExit(f"omitted file(s) recorded in provenance but absent from the destination tree: {sorted(missing)}")
    diff_inventory(destination_inventory, recorded_inventory, tolerated=adaptations, label="the destination tree")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source-repo", required=True, type=Path)
    parser.add_argument("--destination", default=ROOT, type=Path)
    parser.add_argument("--provenance", default=DEFAULT_PROVENANCE, type=Path)
    parser.add_argument("--handoff", default=DEFAULT_HANDOFF, type=Path)
    args = parser.parse_args()

    if not args.provenance.is_file():
        raise SystemExit(f"missing import provenance file: {args.provenance}")
    provenance = json.loads(args.provenance.read_text())

    if not args.handoff.is_file():
        raise SystemExit("missing or unaccepted B.P3 handoff")

    validate_import(provenance, args.source_repo, args.destination, args.handoff.read_text())
    print("B.1 import provenance, BTIT acceptance handoff, and copied inventory are coherent")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
