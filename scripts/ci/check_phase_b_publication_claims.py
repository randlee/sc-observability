#!/usr/bin/env python3
"""Reject Phase B doc text that claims B.2 or B.P2 perform live publication.

Phase B policy: B.P1 implements, B.P2 and B.2 only qualify/stage immutable
candidate artifacts, and B.7 alone performs the live crates.io/registry
publication at phase end (RBQA-F003). This is a focused lexical gate for that
one recurring normative mistake, not a general prose linter: it only flags the
specific phrasings this mistake has repeatedly taken, and explicitly allows
references to the already-released 1.2.0 compatibility baseline and to B.7's
own (real) publication work.

Run directly for a human-readable report, or import `scan_text`/`scan_paths`
for use from another check.
"""

from __future__ import annotations

import re
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Iterable

PHASE_B_DOCS_DIR = Path("docs/plans/phase-b")

# Historical/instance-evidence records describe what already happened and are
# not sequencing policy prose; they are out of this gate's scope.
EXCLUDED_NAME_PREFIXES = ("handoff-",)

ALLOW_CONTEXT_WORDS = (
    "1.2.0",
    "baseline",
    "existing",
    "already",
    "compatibility",
)

# "published core"/"released core" is only in this gate's scope when the text
# actually attributes that published/released state to B.P2's runtime-level
# capability specifically, either by naming B.P2 directly or by making the
# "published/released before BTIT/copy" timing claim B.P2 sequencing prose
# repeatedly got wrong. A bare "B.P3"/"BTIT"/"handoff" mention is not enough:
# e.g. "No published core derive list changes" next to a B.P3 fixture
# reference is a legitimate, unrelated statement about the real
# already-published 1.2.0 API and must not be flagged.
SEQUENCING_NAME_TRIGGER = "b.p2"
SEQUENCING_TIMING_TRIGGER_PAIRS = (
    ("before", "btit"),
    ("before", "copy"),
)

WHITESPACE_RUN = re.compile(r"\s+")


@dataclass(frozen=True)
class Finding:
    path: Path
    rule: str
    snippet: str

    def __str__(self) -> str:  # pragma: no cover - trivial formatting
        return f"{self.path}: [{self.rule}] {self.snippet.strip()}"


def _normalize(text: str) -> str:
    """Collapse whitespace (including line wraps) to single spaces.

    This lets the regexes below catch a stale phrase that a markdown
    formatter has wrapped across two source lines, which plain per-line
    grepping misses.
    """
    return WHITESPACE_RUN.sub(" ", text)


def _snippet(text: str, start: int, end: int, pad: int = 40) -> str:
    lo = max(0, start - pad)
    hi = min(len(text), end + pad)
    return text[lo:hi]


_DIRECT_VERB_PATTERNS = [
    ("bp2-or-b2-publish-verb", re.compile(r"B\.P?2\s+publish(es|ed|ing)?\b", re.IGNORECASE)),
    ("bp2-release-verb", re.compile(r"B\.P2\s+release[sd]?\b(?!\s+train)", re.IGNORECASE)),
]

_REGISTRY_VERSION_PATTERN = re.compile(r"registry\s+version", re.IGNORECASE)
_NOT_A_REGISTRY_PATTERN = re.compile(r"not\s+(?:a|the)\s+registry\s+version", re.IGNORECASE)

_PUBLISHED_OR_RELEASED_CORE_PATTERN = re.compile(
    r"\b(published|released)\s+core\b", re.IGNORECASE
)

# Catches the verb-phrase variant of the same claim, e.g. "core support is
# published before BTIT bridge integration" / "core support must be released
# before accepted BTIT integration", where "published"/"released" is a verb
# rather than an adjective in front of "core".
_CORE_STATE_VERB_PATTERN = re.compile(
    r"\bcore\b(?:\s+\w+){0,4}?\s+(?:is|are|must be|remains|stays)\s+(published|released)\b",
    re.IGNORECASE,
)


def scan_text(path: Path, raw_text: str) -> list[Finding]:
    findings: list[Finding] = []
    text = _normalize(raw_text)

    for rule, pattern in _DIRECT_VERB_PATTERNS:
        for match in pattern.finditer(text):
            findings.append(Finding(path, rule, _snippet(text, match.start(), match.end())))

    for match in _REGISTRY_VERSION_PATTERN.finditer(text):
        window = _snippet(text, match.start(), match.end(), pad=15)
        if _NOT_A_REGISTRY_PATTERN.search(window):
            continue
        findings.append(Finding(path, "bare-registry-version", _snippet(text, match.start(), match.end())))

    for pattern, rule in (
        (_PUBLISHED_OR_RELEASED_CORE_PATTERN, "unqualified-published-or-released-core"),
        (_CORE_STATE_VERB_PATTERN, "unqualified-core-publication-verb"),
    ):
        for match in pattern.finditer(text):
            if _is_scoped_and_unqualified(text, match):
                findings.append(Finding(path, rule, _snippet(text, match.start(), match.end())))

    return findings


def _is_scoped_and_unqualified(text: str, match: re.Match[str]) -> bool:
    wide_window = _snippet(text, match.start(), match.end(), pad=90).lower()
    names = SEQUENCING_NAME_TRIGGER in wide_window
    timing = any(a in wide_window and b in wide_window for a, b in SEQUENCING_TIMING_TRIGGER_PAIRS)
    if not (names or timing):
        return False  # unrelated to B.P2's runtime-level sequencing claim
    narrow_window = _snippet(text, match.start(), match.end(), pad=50)
    if any(word.lower() in narrow_window.lower() for word in ALLOW_CONTEXT_WORDS):
        return False
    return True


def _iter_phase_b_docs(root: Path) -> Iterable[Path]:
    docs_dir = root / PHASE_B_DOCS_DIR
    for path in sorted(docs_dir.glob("*.md")):
        if path.name.startswith(EXCLUDED_NAME_PREFIXES):
            continue
        yield path


def scan_paths(root: Path) -> list[Finding]:
    findings: list[Finding] = []
    for path in _iter_phase_b_docs(root):
        findings.extend(scan_text(path, path.read_text(encoding="utf-8")))
    return findings


def _self_test() -> None:
    """Demonstrate the gate against representative fixtures before real use.

    Covers: a same-line stale claim, a claim wrapped across a line break (the
    multiline variant reviewers flagged), the verb-phrase variant ("core...is
    published before BTIT"), an explicitly B.P2-named capability claim, and
    text that must be permitted (the real 1.2.0 baseline and B.7's own
    publication).
    """
    stale_same_line = (
        "sc-observability checks the handoff against both accepted contracts "
        "and B.P2's registry version."
    )
    assert scan_text(Path("<fixture>"), stale_same_line), "same-line stale claim was not caught"

    stale_wrapped = (
        "B.1 `must_follow` this sprint's published baseline. Merge-forward\n"
        "follows the phase dependency rule; the consumer check waits for\n"
        "registry visibility. B.2\npublishes the companion pair."
    )
    assert scan_text(Path("<fixture>"), stale_wrapped), "line-wrapped stale claim was not caught"

    stale_verb_form_before_btit = (
        "BTIT owns the integration sprint. Core support is published before\n"
        "BTIT bridge integration."
    )
    assert scan_text(
        Path("<fixture>"), stale_verb_form_before_btit
    ), "verb-form 'core...is published before BTIT' claim was not caught"

    stale_named_capability = (
        "BTIT resolves B.P2's published core capability and implements LogGuard."
    )
    assert scan_text(
        Path("<fixture>"), stale_named_capability
    ), "'B.P2's published core capability' claim was not caught"

    allowed = (
        "The existing released `1.2.0` core remains the published compatibility "
        "baseline; the additive runtime-level capability is B.P2's distinct "
        "staged `1.3.0` candidate, not a pre-B.P3 release. B.7 alone turns "
        "qualified staged artifacts into live published artifacts after the "
        "phase-end gates. B.P2's exact staged core package version/checksum, "
        "not a registry version."
    )
    assert not scan_text(Path("<fixture>"), allowed), "legitimate baseline/B.7 text was flagged"


def main() -> int:
    _self_test()
    root = Path(".")
    findings = scan_paths(root)
    if findings:
        print("phase-b publication-claim check FAILED:", file=sys.stderr)
        for finding in findings:
            print(f"  - {finding}", file=sys.stderr)
        return 1
    print("phase-b publication-claim self-test and doc scan passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
