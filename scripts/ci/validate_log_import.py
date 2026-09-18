#!/usr/bin/env python3
"""Validate B.1's immutable BTIT-source import/acceptance contract.

`docs/plans/phase-b/import-provenance.json` pins the exact accepted BTIT
commit, its accepted B.P3 review handoff, and a per-file Git blob inventory
for the three copied crates. This script re-derives the same facts from a
live `--source-repo` (BTIT) checkout, from immutable Git objects, and from
the already-copied `--destination` tree, and fails whenever they disagree.

Citations resolve in different repositories depending on who owns the
document: the BTIT source review is BTIT's own acceptance record, so it
resolves in `--source-repo`; the target document and the handoff revision
are sc-observability's own documents, so they resolve in `--doc-repo` (this
repo's own history in real use). Conflating the two would let a fabricated
citation in the wrong repository pass unnoticed.

Independent comparisons exist because they catch different failure modes:

- source vs provenance: catches a provenance file that was authored against
  the wrong commit, or hand-edited/stale relative to BTIT's real history.
  This must match exactly; nothing here is an allowed "adaptation" because
  BTIT's own source is never touched by this repo. The accepted commit is
  looked up directly by SHA, not via the source repo's current `HEAD`, so a
  checkout that has advanced since acceptance does not invalidate a
  still-present immutable source.
- review citation vs BTIT Git objects: catches a handoff that cites a review
  document that does not exist, does not cover the accepted source, or does
  not carry an actual "accepted" verdict -- rather than trusting free-text
  citations, or a provenance-declared verdict, at face value. A rejected
  review document is refused even if the provenance record's own
  `review_verdict` field consistently claims "rejected": the contract
  requires an accepted review, not merely internal self-consistency.
- target-document/handoff-revision vs sc-observability Git objects: catches
  a fabricated or stale citation of either document. The target document's
  own citation is required in the handoff (`Target document: ... at commit`)
  and cross-checked against `provenance.target_document`, and the document
  itself must actually exist at the cited commit -- an orphan commit that
  exists but holds only unrelated content cannot stand in for it. The
  handoff revision's cited content must byte-match the `--handoff` document
  actually used.
- destination vs provenance: catches a copy step that changed more than the
  declared mechanical adaptations (package metadata, dependency paths,
  relocated test/doc paths, or a `trybuild` fixture's toolchain-specific
  wording). A blob difference here is only allowed when its
  path is explicitly listed in `adaptations` with a reason, a permitted
  mechanical `kind`, exact approved before/after content, and every changed
  line fully matching (not merely containing) that kind's own narrow syntax
  pattern -- a kind label alone proves nothing about what actually changed,
  so a runtime rewrite mislabeled `dependency_path`, or an arbitrary code
  line that merely contains a `.rs`/`.md` substring inside unrelated syntax
  (e.g. a string literal) mislabeled `relocated_doc_or_test_path`, is
  rejected on content, not accepted on label or incidental substring. The
  destination tree is enumerated by walking the filesystem without
  following symlinked directories (`os.walk(followlinks=False)`), and every
  directory and file entry encountered is checked for symlink/regular-file
  status and path confinement before anything is read -- a leaf-only
  symlink check would miss an entire extra directory reached only through a
  symlinked parent.
"""

from __future__ import annotations

import argparse
import difflib
import json
import os
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

_KIND_LINE_PATTERNS = {
    # `(\.workspace)?` accepts both a hard-coded literal (`version = "0.1.0"`)
    # and workspace-inherited form (`version.workspace = true`) -- the actual
    # mechanical need for B.1's copy, converting a source crate's hard-coded
    # `[package]` metadata to this workspace's inherited form.
    "package_metadata": re.compile(
        r"^\s*(publish|version|edition|rust-version|description|license|repository|readme|keywords|categories|authors|homepage)"
        r"(\.workspace)?\s*=\s*.+$"
    ),
    "dependency_path": re.compile(
        r'^\s*(path|version|git|branch|rev)\s*=\s*"[^"]*"\s*,?\s*$'
        r'|^\s*[\w.-]+\s*=\s*\{[^{}]*\}\s*$'
        r'|^\s*\[[\w.-]*dependencies[\w.-]*\]\s*$'
    ),
    # Anchored end-to-end on purpose: a changed line must be *nothing but*
    # genuine path/mod/include syntax. A substring search (the prior bug)
    # matched ".rs"/".md" appearing anywhere, including inside a string
    # literal argument to an arbitrary function call -- fullmatch on these
    # narrow shapes preserves all non-path syntax instead of trusting a
    # keyword or extension appearing anywhere on the line.
    "relocated_doc_or_test_path": re.compile(
        r'^\s*mod\s+[\w:]+\s*;\s*$'
        r'|^\s*#\[path\s*=\s*"[^"]+"\]\s*$'
        r'|^\s*include!\(\s*"[^"]+"\s*\)\s*;?\s*$'
        r'|^\s*"[\w./-]+\.(md|rs)"\s*,?\s*$'
    ),
    # A `trybuild` `.stderr` fixture is free-form rustc pretty-printer prose,
    # not fixed syntax, so no narrow per-line shape can bound it the way the
    # other kinds are bounded. The real guarantee is the structural check in
    # `_validate_diagnostic_text_change`: the same diagnostic codes at the
    # same source locations, in the same order:
    "trybuild_diagnostic_text": re.compile(r".*"),
}

# `package_metadata` and `dependency_path` describe Cargo manifest edits only.
_CARGO_TOML_ONLY_KINDS = frozenset({"package_metadata", "dependency_path"})

# `trybuild_diagnostic_text` describes only a `trybuild` UI-test fixture
# snapshot, which is compiler-toolchain-version-specific by construction:
# moving a crate into a workspace pinning a different Rust toolchain than the
# one that generated the fixture (recorded here, not invented) legitimately
# changes rustc's pretty-printer wording without changing what the fixture
# proves.
_TRYBUILD_STDERR_ONLY_KINDS = frozenset({"trybuild_diagnostic_text"})

# These are post-import compatibility annotations on the copied bridge. They
# are intentionally not folded into import-provenance.json: the historical
# record proves the accepted copy, while this separate record proves the
# later warning-only adaptation.
_POST_IMPORT_WARNING_PATHS = frozenset(
    {
        "crates/sc-observability-log/src/control.rs",
        "crates/sc-observability-log/src/handle.rs",
        "crates/sc-observability-log/src/mapping.rs",
    }
)
_POST_IMPORT_ALLOW_BLOCK_RE = re.compile(
    r'^(?P<indent> *)#\[allow\(\n'
    r'(?P=indent)    deprecated,\n'
    r'(?P=indent)    reason = "[^"\n]+"\n'
    r'(?P=indent)\)\]$'
)


def _is_trybuild_stderr_path(path: str) -> bool:
    p = Path(path)
    return p.suffix == ".stderr" and p.parent.name == "ui" and p.parent.parent.name == "tests"

# Dependency-table structural parsing for `dependency_path`: a full-line regex
# proves each changed line is syntactically a dependency assignment, but not
# that it is a *relocation* of an existing dependency rather than a wholesale
# new one -- so this compares the actual before/after dependency tables.
_DEP_TABLE_HEADER_RE = re.compile(r"^\s*\[([\w.-]*dependencies[\w.-]*)\]\s*$")
_OTHER_TABLE_HEADER_RE = re.compile(r"^\s*\[[^\]]*\]\s*$")
_DEP_ENTRY_RE = re.compile(r'^\s*([A-Za-z0-9_-]+)\s*=\s*("(?:[^"\\]|\\.)*"|\{[^{}]*\})\s*,?\s*$')
_INLINE_KV_RE = re.compile(r'([A-Za-z0-9_-]+)\s*=\s*("(?:[^"\\]|\\.)*"|\[[^\]]*\]|true|false|[0-9.]+)')
_LOCATION_KEYS = frozenset({"path", "version", "git", "branch", "rev", "tag"})

# Relocation-declaration profiling for `relocated_doc_or_test_path`: identity
# (mod name) or count (include!/path-attribute/bare-literal, which carry no
# identity independent of the path itself) must be preserved across
# before/after -- a new declaration is a wholesale addition, not a relocation
# of an existing one.
_MOD_NAME_RE = re.compile(r"^\s*mod\s+([\w:]+)\s*;\s*$")
_INCLUDE_LINE_RE = re.compile(r'^\s*include!\(\s*"[^"]+"\s*\)\s*;?\s*$')
_PATH_ATTR_LINE_RE = re.compile(r'^\s*#\[path\s*=\s*"[^"]+"\]\s*$')
_BARE_PATH_LITERAL_RE = re.compile(r'^\s*"[\w./-]+\.(md|rs)"\s*,?\s*$')

_FULL_SHA_RE = re.compile(r"^[0-9a-f]{40}$")
_ACCEPTED_SHA_RE = re.compile(r"Accepted source SHA:\s*`([0-9a-f]{40})`")
_REVIEW_DOC_RE = re.compile(r"Review document:\s*`([^`]+)`\s*at commit\s*`([0-9a-f]{40})`")
_TARGET_DOC_RE = re.compile(r"Target document:\s*`([^`]+)`\s*at commit\s*`([0-9a-f]{40})`")
_VERDICT_RE = re.compile(r"Verdict:\s*(\S+)")
_ACCEPTANCE_RE = re.compile(r"sc-observability acceptance:\s*(\S+)")
_REVIEWED_COMMIT_RE = re.compile(r"Reviewed commit:\s*([0-9a-f]{40})")
_HANDOFF_REVISION_RE = re.compile(r"^([^@]+)@([0-9a-f]{40})$")


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
    target = _TARGET_DOC_RE.search(text)
    if not target:
        raise SystemExit("missing target-document path/immutable commit citation in B.P3 handoff")
    return {
        "accepted_sha": accepted_sha.group(1),
        "review_path": review.group(1),
        "review_commit": review.group(2),
        "target_path": target.group(1),
        "target_commit": target.group(2),
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
    """Independently enumerate the destination tree, rejecting anything unsafe to read.

    Uses `os.walk(followlinks=False)` so a symlinked directory is never
    descended into, and every directory and file entry is explicitly
    checked for symlink status -- a leaf-only symlink check would miss an
    entire extra directory reached only via a symlinked parent.
    """
    dest_resolved = destination.resolve()
    inventory: dict[str, str] = {}
    for prefix in prefixes:
        base = destination / prefix
        if not base.exists():
            continue
        if base.is_symlink():
            raise SystemExit(f"symlink not permitted in destination tree: {prefix}")
        for dirpath, dirnames, filenames in os.walk(base, followlinks=False):
            dirpath_p = Path(dirpath)
            for dirname in dirnames:
                dpath = dirpath_p / dirname
                if dpath.is_symlink():
                    rel = dpath.relative_to(destination).as_posix()
                    raise SystemExit(f"symlink not permitted in destination tree: {rel}")
            for filename in filenames:
                fpath = dirpath_p / filename
                rel = fpath.relative_to(destination).as_posix()
                if fpath.is_symlink():
                    raise SystemExit(f"symlink not permitted in destination tree: {rel}")
                if not fpath.is_file():
                    raise SystemExit(f"non-regular file not permitted in destination tree: {rel}")
                resolved = fpath.resolve()
                if resolved != dest_resolved and dest_resolved not in resolved.parents:
                    raise SystemExit(f"path escapes destination tree: {rel}")
                inventory[rel] = subprocess.run(
                    ["git", "hash-object", str(fpath)], check=True, capture_output=True, text=True,
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


def _changed_lines(before: str, after: str) -> list[str]:
    diff = difflib.ndiff(before.splitlines(), after.splitlines())
    return [
        line[2:] for line in diff
        if (line.startswith("+ ") or line.startswith("- ")) and line[2:].strip()
    ]


def _parse_dependency_tables(content: str) -> dict[str, dict[str, str]]:
    """Map each `*dependencies*` table name to {dependency_name: raw_value_text}."""
    tables: dict[str, dict[str, str]] = {}
    current: str | None = None
    for line in content.splitlines():
        header = _DEP_TABLE_HEADER_RE.match(line)
        if header:
            current = header.group(1)
            tables.setdefault(current, {})
            continue
        if _OTHER_TABLE_HEADER_RE.match(line):
            current = None
            continue
        if current is None:
            continue
        entry = _DEP_ENTRY_RE.match(line)
        if entry:
            tables[current][entry.group(1)] = entry.group(2)
    return tables


def _parse_inline_table(value: str) -> dict[str, str]:
    value = value.strip()
    if not value.startswith("{"):
        return {"version": value}
    return {m.group(1): m.group(2) for m in _INLINE_KV_RE.finditer(value)}


def _validate_dependency_path_change(before: str, after: str, path: str) -> None:
    """Require every dependency-table change to be a relocation, never an addition.

    A full-line regex proves each changed line is syntactically a dependency
    assignment, not that the assignment relocates an *existing* dependency:
    appending a whole new `[dependencies]` table, or a whole new dependency
    entry with its own arbitrary features, previously passed on syntax alone.
    """
    before_tables = _parse_dependency_tables(before)
    after_tables = _parse_dependency_tables(after)
    if set(before_tables) != set(after_tables):
        raise SystemExit(
            f"adaptation for {path} adds or removes a dependency table, "
            "not a permitted dependency_path mechanical change"
        )
    for table, before_deps in before_tables.items():
        after_deps = after_tables[table]
        if set(before_deps) != set(after_deps):
            raise SystemExit(
                f"adaptation for {path} adds or removes a dependency entry in [{table}], "
                "not a permitted dependency_path mechanical change"
            )
        for name, before_raw in before_deps.items():
            before_kv = _parse_inline_table(before_raw)
            after_kv = _parse_inline_table(after_deps[name])
            before_other = {k: v for k, v in before_kv.items() if k not in _LOCATION_KEYS}
            after_other = {k: v for k, v in after_kv.items() if k not in _LOCATION_KEYS}
            if before_other != after_other:
                raise SystemExit(
                    f"adaptation for {path} changes non-location dependency keys for {name!r}, "
                    "not a permitted dependency_path mechanical change"
                )


def _relocation_profile(content: str) -> tuple[set[str], int, int, int]:
    mod_names: set[str] = set()
    include_count = path_attr_count = bare_literal_count = 0
    for line in content.splitlines():
        mod_match = _MOD_NAME_RE.match(line)
        if mod_match:
            mod_names.add(mod_match.group(1))
        elif _INCLUDE_LINE_RE.match(line):
            include_count += 1
        elif _PATH_ATTR_LINE_RE.match(line):
            path_attr_count += 1
        elif _BARE_PATH_LITERAL_RE.match(line):
            bare_literal_count += 1
    return mod_names, include_count, path_attr_count, bare_literal_count


def _validate_relocation_change(before: str, after: str, path: str) -> None:
    """Require every relocation-kind change to replace an existing declaration.

    `mod` identity (its name) and the count of include!/path-attribute/bare
    path-literal declarations (which carry no identity independent of the
    path itself) must be preserved across before/after: a full-line regex
    proves syntax, not that a `mod` or `include!` is a genuine relocation
    rather than a wholesale new declaration appended alongside it.
    """
    before_mods, before_inc, before_attr, before_bare = _relocation_profile(before)
    after_mods, after_inc, after_attr, after_bare = _relocation_profile(after)
    if before_mods != after_mods:
        raise SystemExit(
            f"adaptation for {path} adds or removes a mod declaration, "
            "not a permitted relocated_doc_or_test_path mechanical change"
        )
    if before_inc != after_inc:
        raise SystemExit(
            f"adaptation for {path} adds or removes an include! declaration, "
            "not a permitted relocated_doc_or_test_path mechanical change"
        )
    if before_attr != after_attr:
        raise SystemExit(
            f"adaptation for {path} adds or removes a #[path] attribute, "
            "not a permitted relocated_doc_or_test_path mechanical change"
        )
    if before_bare != after_bare:
        raise SystemExit(
            f"adaptation for {path} adds or removes a path literal, "
            "not a permitted relocated_doc_or_test_path mechanical change"
        )


_DIAGNOSTIC_HEADER_RE = re.compile(r"^(error(?:\[E\d+\])?):")
_DIAGNOSTIC_LOCATION_RE = re.compile(r"^\s*-->\s*(\S+)\s*$")


def _diagnostic_profile(content: str) -> tuple[list[str], list[str]]:
    headers: list[str] = []
    locations: list[str] = []
    for line in content.splitlines():
        header_match = _DIAGNOSTIC_HEADER_RE.match(line)
        if header_match:
            headers.append(header_match.group(1))
            continue
        location_match = _DIAGNOSTIC_LOCATION_RE.match(line)
        if location_match:
            locations.append(location_match.group(1))
    return headers, locations


def _validate_diagnostic_text_change(before: str, after: str, path: str) -> None:
    """Require a `trybuild_diagnostic_text` change to preserve what failed and where.

    rustc's pretty-printer wording (and how much surrounding source context it
    prints) is toolchain-version-specific, so no per-line syntax can bound
    this kind's prose the way the other kinds are bounded. Instead, the same
    ordered sequence of diagnostic codes (`error[EXXXX]`/bare `error`) and the
    same ordered sequence of `-->` source locations must appear on both
    sides -- preserving which error fired and at what source position, while
    still allowing the surrounding message text to differ. Both sides must
    carry a nonempty profile: a stray non-diagnostic file mislabeled with this
    kind (or one whose diagnostics were emptied out) has nothing to prove it
    still rejects the same thing.
    """
    before_headers, before_locations = _diagnostic_profile(before)
    after_headers, after_locations = _diagnostic_profile(after)
    if not before_headers or not before_locations or not after_headers or not after_locations:
        raise SystemExit(
            f"adaptation for {path} has an empty diagnostic error/location profile on one side, "
            "not a permitted trybuild_diagnostic_text mechanical change"
        )
    if before_headers != after_headers:
        raise SystemExit(
            f"adaptation for {path} changes the diagnostic error codes or their order, "
            "not a permitted trybuild_diagnostic_text mechanical change"
        )
    if before_locations != after_locations:
        raise SystemExit(
            f"adaptation for {path} changes the diagnostic source locations, "
            "not a permitted trybuild_diagnostic_text mechanical change"
        )


def validate_adaptations(adaptations: list[dict], recorded_inventory: dict[str, str], cwd: Path) -> dict[str, str]:
    """Verify every declared adaptation and return the expected destination inventory.

    Each adaptation must carry a reason, a permitted mechanical `kind`, exact
    approved `before`/`after` content, and every changed line between them
    must match that kind's own pattern -- a kind label alone does not prove
    the change is actually mechanical, so declaring `dependency_path` over an
    arbitrary runtime rewrite is rejected here, not accepted on label alone.
    `package_metadata`/`dependency_path` are further restricted to Cargo.toml
    files, and `dependency_path`/`relocated_doc_or_test_path` additionally
    require structural before/after equivalence (same dependency names/table
    membership, same mod/include!/path-attribute declarations) -- per-line
    syntax alone cannot distinguish a genuine relocation of something that
    already existed from a wholesale new dependency or module appended
    alongside it.
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
        pattern = _KIND_LINE_PATTERNS.get(kind)
        if pattern is None:
            raise SystemExit(f"adaptation for {path} has a kind that is not a permitted mechanical change: {kind}")
        if kind in _CARGO_TOML_ONLY_KINDS and Path(path).name != "Cargo.toml":
            raise SystemExit(f"adaptation for {path} has kind {kind} but is not a Cargo.toml file")
        if kind in _TRYBUILD_STDERR_ONLY_KINDS and not _is_trybuild_stderr_path(path):
            raise SystemExit(f"adaptation for {path} has kind {kind} but is not a tests/ui/*.stderr file")
        before, after = item.get("before"), item.get("after")
        if before is None or after is None:
            raise SystemExit(f"adaptation for {path} is missing exact approved before/after content")
        if blob_id_of_content(before, cwd) != recorded_inventory[path]:
            raise SystemExit(f"adaptation 'before' content for {path} does not match the recorded source blob")
        for line in _changed_lines(before, after):
            if not pattern.fullmatch(line):
                raise SystemExit(
                    f"adaptation for {path} changes content that is not a permitted {kind} mechanical change: {line!r}"
                )
        if kind == "dependency_path":
            _validate_dependency_path_change(before, after, path)
        elif kind == "relocated_doc_or_test_path":
            _validate_relocation_change(before, after, path)
        elif kind == "trybuild_diagnostic_text":
            _validate_diagnostic_text_change(before, after, path)
        expected[path] = blob_id_of_content(after, cwd)
    return expected


def _git_file_at_commit(repo: Path, commit: str, path: str) -> str:
    result = subprocess.run(
        ["git", "-C", str(repo), "show", f"{commit}:{path}"],
        check=False,
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        raise SystemExit(f"post-import adaptation source file not found: {path}@{commit}")
    return result.stdout


def validate_post_import_adaptations(
    record: dict,
    provenance: dict,
    source_repo: Path,
    destination: Path,
    expected_destination: dict[str, str],
) -> dict[str, str]:
    """Verify the separately recorded warning-only edits after the accepted copy.

    The historical import record remains authoritative for the accepted source
    and all mechanical copy adaptations. This record only permits inserting
    exact, reason-bearing deprecated-warning allowance blocks into the three
    copied bridge modules. Removing every declared block from the destination
    must reproduce the accepted source file byte-for-byte; therefore a body,
    signature, undeclared file, or other warning edit cannot pass by sharing a
    broad textual pattern.
    """
    if record.get("historical_provenance") != "docs/plans/phase-b/import-provenance.json":
        raise SystemExit("post-import adaptation record does not cite historical import provenance")
    source_commit = provenance.get("source_commit")
    if record.get("source_commit") != source_commit:
        raise SystemExit("post-import adaptation source SHA does not match historical provenance")
    adaptations = record.get("adaptations")
    if not isinstance(adaptations, list) or not adaptations:
        raise SystemExit("post-import adaptation record has no adaptations")

    updated = dict(expected_destination)
    seen: set[str] = set()
    for item in adaptations:
        path = item.get("path")
        if not isinstance(path, str) or path in seen:
            raise SystemExit(f"duplicate or missing post-import adaptation path: {path}")
        seen.add(path)
        kind = item.get("kind")
        if kind == "deprecated_warning_allowance" and path not in _POST_IMPORT_WARNING_PATHS:
            raise SystemExit(f"post-import adaptation path is outside the warning-only bridge allowlist: {path}")
        if path not in provenance.get("file_inventory", {}):
            raise SystemExit(f"post-import adaptation path is absent from historical inventory: {path}")
        if not (item.get("reason") or "").strip():
            raise SystemExit(f"post-import adaptation for {path} is missing a reason")
        blocks = item.get("blocks")
        if kind == "deprecated_warning_allowance" and (not isinstance(blocks, list) or not blocks):
            raise SystemExit(f"post-import adaptation for {path} has no exact allowance blocks")
        if kind not in {"deprecated_warning_allowance", "approved_qa_delta"}:
            raise SystemExit(f"post-import adaptation for {path} has an unsupported kind")
        if blocks is None:
            blocks = []
        normalized_blocks: list[str] = []
        for block in blocks:
            if not isinstance(block, str) or not _POST_IMPORT_ALLOW_BLOCK_RE.fullmatch(block):
                raise SystemExit(f"post-import adaptation for {path} contains an invalid allowance block")
            normalized_blocks.append(block)

        before = _git_file_at_commit(source_repo, source_commit, path)
        before_blob = blob_id_of_content(before, destination)
        if before_blob != item.get("before_blob") or before_blob != provenance["file_inventory"][path]:
            raise SystemExit(f"post-import adaptation before blob for {path} does not match accepted source")
        destination_path = destination / path
        if not destination_path.is_file() or destination_path.is_symlink():
            raise SystemExit(f"post-import adaptation destination is not a regular file: {path}")
        after = destination_path.read_text()
        after_blob = blob_id_of_content(after, destination)
        if after_blob != item.get("after_blob"):
            raise SystemExit(f"post-import adaptation after blob for {path} does not match the recorded result")

        reconstructed = after
        for block in normalized_blocks:
            insertion = block + "\n"
            if insertion not in reconstructed:
                raise SystemExit(f"post-import adaptation block missing from destination: {path}")
            reconstructed = reconstructed.replace(insertion, "", 1)
        qa_delta = item.get("qa_delta")
        if qa_delta is None:
            if reconstructed != before:
                raise SystemExit(
                    f"post-import adaptation for {path} changes content beyond declared warning allowances"
                )
        else:
            if not isinstance(qa_delta, dict) or not re.fullmatch(r"[0-9a-f]{40}", qa_delta.get("commit", "")):
                raise SystemExit(f"post-import adaptation QA delta for {path} lacks an immutable commit")
            reason = qa_delta.get("reason")
            patch = qa_delta.get("patch")
            if not isinstance(reason, str) or not reason.strip() or not isinstance(patch, str) or not patch:
                raise SystemExit(f"post-import adaptation QA delta for {path} lacks exact evidence and rationale")
            actual_patch = "".join(
                difflib.unified_diff(
                    before.splitlines(keepends=True),
                    reconstructed.splitlines(keepends=True),
                    fromfile=f"accepted/{path}",
                    tofile=f"approved/{qa_delta['commit']}/{path}",
                )
            )
            if actual_patch != patch:
                raise SystemExit(f"post-import adaptation QA delta for {path} does not match exact result evidence")
        updated[path] = after_blob

    return updated


def verify_review_citation(
    source_repo: Path, review_path: str, review_commit: str, source_commit: str, expected_verdict: str,
) -> None:
    """Resolve the handoff's review citation against BTIT's own Git history.

    The review is BTIT's own acceptance record of its own commit, so it must
    resolve in the source repo, never in sc-observability's own doc history.
    """
    if not git_commit_exists(source_repo, review_commit):
        raise SystemExit("review document commit not found in source repository")
    result = subprocess.run(
        ["git", "-C", str(source_repo), "show", f"{review_commit}:{review_path}"],
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


def verify_target_document(doc_repo: Path, target_path: str, target_commit: str) -> None:
    """Resolve the target-document citation against sc-observability's own Git history.

    Commit existence alone is not enough: an orphan commit that exists but
    contains only unrelated content must not stand in for the real target
    document, so the document itself must actually be present at that commit.
    """
    if not _FULL_SHA_RE.match(target_commit) or not git_commit_exists(doc_repo, target_commit):
        raise SystemExit("target document commit not found in doc repository")
    result = subprocess.run(
        ["git", "-C", str(doc_repo), "show", f"{target_commit}:{target_path}"],
        capture_output=True, text=True,
    )
    if result.returncode != 0:
        raise SystemExit(f"target document not found at cited commit: {target_path}@{target_commit}")


def verify_handoff_revision(doc_repo: Path, handoff_revision: str, handoff_text: str) -> None:
    """Resolve the handoff-revision citation against sc-observability's own Git history.

    The cited commit's content must byte-match the `--handoff` document
    actually used for validation, so a synthetic or stale revision string
    cannot stand in for the real committed handoff.
    """
    match = _HANDOFF_REVISION_RE.match(handoff_revision or "")
    if not match:
        raise SystemExit("import-provenance.json handoff_revision must be '<path>@<full-40-character-commit-sha>'")
    path, commit = match.group(1), match.group(2)
    if not git_commit_exists(doc_repo, commit):
        raise SystemExit("handoff_revision commit not found in doc repository")
    result = subprocess.run(
        ["git", "-C", str(doc_repo), "show", f"{commit}:{path}"],
        capture_output=True, text=True,
    )
    if result.returncode != 0:
        raise SystemExit(f"handoff document not found at cited handoff_revision: {path}@{commit}")
    if result.stdout != handoff_text:
        raise SystemExit("handoff_revision content does not match the provided --handoff document")


def validate_import(
    provenance: dict,
    source_repo: Path,
    destination: Path,
    handoff_text: str,
    doc_repo: Path = ROOT,
    post_import_adaptations: dict | None = None,
    release_adaptations: Path | None = None,
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

    target_document = provenance.get("target_document", {})
    if target_document.get("path") != handoff["target_path"] or target_document.get("commit") != handoff["target_commit"]:
        raise SystemExit("handoff and provenance target-document citation mismatch")

    if handoff["accepted_sha"] != source_commit:
        raise SystemExit("handoff and provenance source SHA mismatch")

    review_verdict = provenance.get("review_verdict", "")
    if review_verdict.lower() != "accepted":
        raise SystemExit("import-provenance.json review_verdict must be accepted")

    if not git_commit_exists(source_repo, source_commit):
        raise SystemExit("accepted source commit not found in source repository")

    # Source-repo comparison is strict: BTIT's own history is never adapted,
    # so any difference here means the provenance record itself is unreliable.
    source_inventory = git_blob_inventory(source_repo, source_commit, IMPORT_CRATE_PREFIXES)
    diff_inventory(source_inventory, recorded_inventory, label="the source repository")

    verify_review_citation(source_repo, handoff["review_path"], handoff["review_commit"], source_commit, review_verdict)
    verify_target_document(doc_repo, handoff["target_path"], handoff["target_commit"])
    verify_handoff_revision(doc_repo, provenance.get("handoff_revision", ""), handoff_text)

    # Destination comparison tolerates only the explicitly declared, verified adaptations.
    expected_destination_inventory = validate_adaptations(
        provenance.get("adaptations", []), recorded_inventory, doc_repo,
    )
    if post_import_adaptations is not None:
        expected_destination_inventory = validate_post_import_adaptations(
            post_import_adaptations,
            provenance,
            source_repo,
            destination,
            expected_destination_inventory,
        )
    if release_adaptations is not None:
        from _log_release_adaptations import apply_release_adaptations
        expected_destination_inventory = apply_release_adaptations(
            expected_destination_inventory, destination, release_adaptations,
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
    parser.add_argument("--post-import-adaptations", type=Path)
    parser.add_argument("--release-adaptations", type=Path)
    args = parser.parse_args()

    if not args.provenance.is_file():
        raise SystemExit(f"missing import provenance file: {args.provenance}")
    provenance = json.loads(args.provenance.read_text())
    post_import_adaptations = None
    if args.post_import_adaptations is not None:
        if not args.post_import_adaptations.is_file():
            raise SystemExit(f"missing post-import adaptation record: {args.post_import_adaptations}")
        post_import_adaptations = json.loads(args.post_import_adaptations.read_text())

    if not args.handoff.is_file():
        raise SystemExit("missing or unaccepted B.P3 handoff")

    validate_import(
        provenance,
        args.source_repo,
        args.destination,
        args.handoff.read_text(),
        doc_repo=args.doc_repo,
        post_import_adaptations=post_import_adaptations,
        release_adaptations=args.release_adaptations,
    )
    if args.release_adaptations is not None:
        print("B.1 historical provenance, declared post-import and B.2 release adaptations are coherent")
    elif post_import_adaptations is None:
        print("B.1 import provenance, BTIT acceptance handoff, and copied inventory are coherent")
    else:
        print("B.1 import provenance and post-import warning-only adaptations are coherent")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
