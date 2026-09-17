#!/usr/bin/env python3
"""Validate B.1's immutable BTIT-source import/acceptance contract.

`docs/plans/phase-b/import-provenance.json` pins the exact accepted BTIT
commit, its accepted B.P3 review handoff, and a per-file Git blob inventory
for the three copied crates. This script re-derives the same facts from a
live `--source-repo` checkout, from immutable Git objects in `--doc-repo`
(the repo holding the review document and the target-contract commit; this
repo's own history in real use), and from the already-copied `--destination`
tree, and fails whenever they disagree.

Three independent comparisons exist because they catch different failure
modes:

- source vs provenance: catches a provenance file that was authored against
  the wrong commit, or hand-edited/stale relative to BTIT's real history.
  This must match exactly; nothing here is an allowed "adaptation" because
  BTIT's own source is never touched by this repo. The accepted commit is
  looked up directly by SHA, not via the source repo's current `HEAD`, so a
  checkout that has advanced since acceptance does not invalidate a
  still-present immutable source.
- review/target citations vs doc-repo Git objects: catches a handoff or
  provenance record that cites a review document or target-contract commit
  that does not exist, does not cover the accepted source, or does not carry
  the recorded verdict -- rather than trusting free-text citations at face
  value.
- destination vs provenance: catches a copy step that changed more than the
  declared mechanical adaptations (package metadata, dependency paths,
  relocated test/doc paths). A blob difference here is only allowed when its
  path is explicitly listed in `adaptations` with a reason, a permitted
  mechanical `kind`, and the exact approved before/after content -- not a
  bare path allowlist. The destination tree is enumerated independently by
  walking the filesystem (not just looking up recorded paths), so an
  unrecorded extra file cannot silently pass, and symlinks/non-regular files
  are rejected before their content is ever read.
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

ALLOWED_ADAPTATION_KINDS = {"package_metadata", "dependency_path", "relocated_doc_or_test_path"}

_FULL_SHA_RE = re.compile(r"^[0-9a-f]{40}$")
_ACCEPTED_SHA_RE = re.compile(r"Accepted source SHA:\s*`([0-9a-f]{40})`")
_REVIEW_DOC_RE = re.compile(r"Review document:\s*`([^`]+)`\s*at commit\s*`([0-9a-f]{40})`")
_VERDICT_RE = re.compile(r"Verdict:\s*(\S+)")
_ACCEPTANCE_RE = re.compile(r"sc-observability acceptance:\s*(\S+)")
_REVIEWED_COMMIT_RE = re.compile(r"Reviewed commit:\s*([0-9a-f]{40})")


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


def git_commit_exists(repo: Path, sha: str) -> bool:
    result = subprocess.run(
        ["git", "-C", str(repo), "cat-file", "-e", f"{sha}^{{commit}}"],
        capture_output=True, text=True,
    )
    return result.returncode == 0


def git_blob_inventory(repo: Path, commit: str, prefixes: tuple[str, ...]) -> dict[str, str]:
    """Map every tracked file under `prefixes` at `commit` to its Git blob SHA.

    `commit` is looked up directly (never the repo's ambient `HEAD`), so a
    checkout that has advanced past the accepted commit does not invalidate
    a still-present immutable source.
    """
    result = subprocess.run(
        ["git", "-C", str(repo), "ls-tree", "-r", "--full-tree", commit],
        check=True, capture_output=True, text=True,
    )
    inventory: dict[str, str] = {}
    for line in result.stdout.splitlines():
        meta, path = line.split("\t", 1)
        mode, kind, blob = meta.split()
        if kind != "blob":
            continue
        if not any(path == prefix or path.startswith(prefix + "/") for prefix in prefixes):
            continue
        if mode == "120000":
            raise SystemExit(f"symlink not permitted in source repository tree: {path}")
        inventory[path] = blob
    return inventory


def walk_destination_inventory(destination: Path, prefixes: tuple[str, ...]) -> dict[str, str]:
    """Independently enumerate the destination tree, rejecting anything unsafe to read."""
    dest_resolved = destination.resolve()
    inventory: dict[str, str] = {}
    for prefix in prefixes:
        base = destination / prefix
        if not base.is_dir():
            continue
        for path in sorted(base.rglob("*")):
            if path.is_dir():
                continue
            rel = path.relative_to(destination).as_posix()
            if path.is_symlink():
                raise SystemExit(f"symlink not permitted in destination tree: {rel}")
            if not path.is_file():
                raise SystemExit(f"non-regular file not permitted in destination tree: {rel}")
            resolved = path.resolve()
            if resolved != dest_resolved and dest_resolved not in resolved.parents:
                raise SystemExit(f"path escapes destination tree: {rel}")
            inventory[rel] = subprocess.run(
                ["git", "hash-object", str(path)], check=True, capture_output=True, text=True,
            ).stdout.strip()
    return inventory


def validate_inventory_paths(paths: list[str]) -> None:
    """Reject any recorded path that escapes the three permitted crate directories."""
    for path in paths:
        parts = Path(path).parts
        escapes = path.startswith("/") or ".." in parts
        in_scope = any(path == prefix or path.startswith(prefix + "/") for prefix in IMPORT_CRATE_PREFIXES)
        if escapes or not in_scope:
            raise SystemExit(f"escaping path in import provenance inventory: {path}")


def validate_nonempty_crate_inventories(paths: list[str]) -> None:
    """Reject a provenance record that omits an entire required crate."""
    for prefix in IMPORT_CRATE_PREFIXES:
        if not any(p == prefix or p.startswith(prefix + "/") for p in paths):
            raise SystemExit(f"recorded inventory has no files for required crate: {prefix}")


def diff_inventory(actual: dict[str, str], recorded: dict[str, str], *, label: str) -> None:
    extra = sorted(set(actual) - set(recorded))
    if extra:
        raise SystemExit(f"unexplained extra file(s) in {label} not recorded in provenance: {extra}")
    omitted = sorted(set(recorded) - set(actual))
    if omitted:
        raise SystemExit(f"omitted file(s) recorded in provenance but absent from {label}: {omitted}")
    for path, recorded_blob in recorded.items():
        if actual[path] != recorded_blob:
            raise SystemExit(f"unexplained content difference in {label} for {path}")


def blob_id_of_content(content: str, cwd: Path) -> str:
    return subprocess.run(
        ["git", "hash-object", "--stdin"], input=content, cwd=str(cwd),
        check=True, capture_output=True, text=True,
    ).stdout.strip()


def validate_adaptations(adaptations: list[dict], recorded_inventory: dict[str, str], cwd: Path) -> dict[str, str]:
    """Verify every declared adaptation and return the expected destination inventory.

    Each adaptation must carry a reason, a permitted mechanical `kind`, and
    the exact approved `before`/`after` content -- a bare path allowlist is
    not enough, since that would let an arbitrary content change hide behind
    a declared path.
    """
    expected = dict(recorded_inventory)
    seen: set[str] = set()
    for item in adaptations:
        path = item.get("path")
        if not path or path in seen:
            raise SystemExit(f"duplicate or missing adaptation path: {path}")
        seen.add(path)
        if path not in recorded_inventory:
            raise SystemExit(f"adaptation references path not in recorded inventory: {path}")
        reason = (item.get("reason") or "").strip()
        if not reason:
            raise SystemExit(f"adaptation for {path} is missing a reason")
        kind = item.get("kind")
        if kind not in ALLOWED_ADAPTATION_KINDS:
            raise SystemExit(f"adaptation for {path} has a kind that is not a permitted mechanical change: {kind}")
        before, after = item.get("before"), item.get("after")
        if before is None or after is None:
            raise SystemExit(f"adaptation for {path} is missing exact approved before/after content")
        if blob_id_of_content(before, cwd) != recorded_inventory[path]:
            raise SystemExit(f"adaptation 'before' content for {path} does not match the recorded source blob")
        expected[path] = blob_id_of_content(after, cwd)
    return expected


def verify_review_citation(
    doc_repo: Path, review_path: str, review_commit: str, source_commit: str, expected_verdict: str,
) -> None:
    """Resolve the handoff's review citation against a real Git object and its content."""
    if not git_commit_exists(doc_repo, review_commit):
        raise SystemExit("review document commit not found in doc repository")
    result = subprocess.run(
        ["git", "-C", str(doc_repo), "show", f"{review_commit}:{review_path}"],
        capture_output=True, text=True,
    )
    if result.returncode != 0:
        raise SystemExit(f"review document not found at cited commit: {review_path}@{review_commit}")
    content = result.stdout
    reviewed = _REVIEWED_COMMIT_RE.search(content)
    verdict = _VERDICT_RE.search(content)
    if not reviewed or reviewed.group(1) != source_commit:
        raise SystemExit("cited review document does not cover the accepted source commit")
    if not verdict or verdict.group(1).lower() != expected_verdict.lower():
        raise SystemExit("cited review document verdict does not match the recorded review verdict")


def verify_target_contract_commit(doc_repo: Path, target_commit: str) -> None:
    if not _FULL_SHA_RE.match(target_commit) or not git_commit_exists(doc_repo, target_commit):
        raise SystemExit("target contract commit not found in doc repository")


def validate_import(
    provenance: dict,
    source_repo: Path,
    destination: Path,
    handoff_text: str,
    doc_repo: Path = ROOT,
) -> None:
    recorded_inventory = provenance.get("file_inventory", {})
    recorded_paths = list(recorded_inventory)
    validate_inventory_paths(recorded_paths)
    validate_nonempty_crate_inventories(recorded_paths)

    handoff = parse_handoff(handoff_text)

    source_commit = provenance.get("source_commit", "")
    if not _FULL_SHA_RE.match(source_commit):
        raise SystemExit("import-provenance.json source_commit must be a full 40-character SHA")

    review_document = provenance.get("review_document", {})
    if review_document.get("path") != handoff["review_path"] or review_document.get("commit") != handoff["review_commit"]:
        raise SystemExit("handoff and provenance review-document citation mismatch")

    if handoff["accepted_sha"] != source_commit:
        raise SystemExit("handoff and provenance source SHA mismatch")

    if not git_commit_exists(source_repo, source_commit):
        raise SystemExit("accepted source commit not found in source repository")

    # Source-repo comparison is strict: BTIT's own history is never adapted,
    # so any difference here means the provenance record itself is unreliable.
    source_inventory = git_blob_inventory(source_repo, source_commit, IMPORT_CRATE_PREFIXES)
    diff_inventory(source_inventory, recorded_inventory, label="the source repository")

    verify_review_citation(
        doc_repo, handoff["review_path"], handoff["review_commit"], source_commit,
        provenance.get("review_verdict", ""),
    )
    verify_target_contract_commit(doc_repo, provenance.get("target_contract_commit", ""))

    # Destination comparison tolerates only the explicitly declared, verified adaptations.
    expected_destination_inventory = validate_adaptations(
        provenance.get("adaptations", []), recorded_inventory, doc_repo,
    )
    destination_inventory = walk_destination_inventory(destination, IMPORT_CRATE_PREFIXES)
    diff_inventory(destination_inventory, expected_destination_inventory, label="the destination tree")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source-repo", required=True, type=Path)
    parser.add_argument("--destination", default=ROOT, type=Path)
    parser.add_argument("--doc-repo", default=ROOT, type=Path)
    parser.add_argument("--provenance", default=DEFAULT_PROVENANCE, type=Path)
    parser.add_argument("--handoff", default=DEFAULT_HANDOFF, type=Path)
    args = parser.parse_args()

    if not args.provenance.is_file():
        raise SystemExit(f"missing import provenance file: {args.provenance}")
    provenance = json.loads(args.provenance.read_text())

    if not args.handoff.is_file():
        raise SystemExit("missing or unaccepted B.P3 handoff")

    validate_import(provenance, args.source_repo, args.destination, args.handoff.read_text(), doc_repo=args.doc_repo)
    print("B.1 import provenance, BTIT acceptance handoff, and copied inventory are coherent")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
