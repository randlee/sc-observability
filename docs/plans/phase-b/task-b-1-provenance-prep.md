---
id: B.1-provenance-prep
status: complete
qa_status: pending_independent_qa
merge_status: unmerged
branch: feature/phase-b-1-provenance-prep
worktree: /Users/randlee/github/sc-observability-worktrees/feature/phase-b-1-provenance-prep
target: feature/phase-b-1a-neutral-prep
base: develop
---

# B.1 provenance-prep — Build and prove the import/acceptance validator ahead of B.1

## Goal and dependencies

This is preparation tooling for [B.1](sprint-b-1-copy.md), not B.1 itself: it
builds and proves `scripts/ci/validate_log_import.py` (B.1 deliverable 3) in
parallel with B.P3's active source corrections and B.1a's neutral-API prep, so
the validator is ready the moment B.1's real copy has an accepted source. It
does not copy BTIT source, does not grant source approval, and does not
publish anything. Linear child of `feature/phase-b-1a-neutral-prep`; merge
that parent forward before every round once it has pushed commits. Full B.1
closure, the real `docs/plans/phase-b/import-provenance.json`, and the real
copy remain blocked on B.P3's accepted source handoff, which is not yet
granted (QA1 on the BTIT source implementation returned FAIL).

## Deliverables (authoritative)

1. `scripts/ci/validate_log_import.py`: given `--source-repo`, the destination
   tree (defaults to this repo root), `--provenance`
   (`docs/plans/phase-b/import-provenance.json`) and `--handoff`
   (`docs/plans/phase-b/handoff-b-p3.md`), it enforces the full B.1 deliverable
   3 contract: a full 40-character source SHA cross-checked between the
   provenance record, the B.P3 handoff's own citation, and the source repo's
   actual `HEAD`; a handoff carrying an accepted verdict plus an immutable
   review-document path/commit; and a per-file Git blob inventory diffed
   strictly against the source repo (BTIT history is never adapted, so any
   difference there means the provenance record itself is unreliable) and
   tolerantly against the destination tree (a blob difference is allowed only
   when its path is explicitly listed in the provenance `adaptations` array
   with a reason).
2. `scripts/ci/tests/test_validate_log_import.py`: fixture coverage built from
   temporary, synthetic Git repositories and honest made-up review text --
   never a fake production `import-provenance.json` or `handoff-b-p3.md`,
   neither of which exists yet. Covers the valid case, a declared adaptation,
   and every required negative case: unaccepted handoff, missing
   review-document citation, source-repo/provenance SHA mismatch,
   handoff/provenance SHA mismatch, extra and omitted files on both the
   source and destination sides, an undeclared destination content
   difference, and escaping/out-of-scope inventory paths.
3. This task-plan document and its execution evidence.

The validator's handoff markers (`Accepted source SHA:`, `Review document: ...
at commit`, `Verdict:`, `sc-observability acceptance:`) are this task's own
design choice, defined because no accepted-handoff text format exists yet.
Whoever writes the real accepted `handoff-b-p3.md` must either use these
markers or the validator must be updated to match; this task does not
prejudge B.P3's real acceptance format, only proposes one so the tool is
testable now.

## Acceptance criteria (authoritative)

- AC1: The valid immutable fixture passes; every required negative fixture
  fails for its specifically expected reason (verified by asserting on the
  distinct error message each raises, not just that the process exits
  non-zero).
- AC2: Two complete implementation/verification passes against the actual
  code and fixture evidence, recorded in the local task checklist.
- AC3: This preparation task closes on its tooling alone. Full B.1 and the
  real BTIT copy remain pending B.P3's accepted source and are explicitly out
  of scope here.
- AC4: No Cargo/workspace membership, crate, or phase-policy-document change;
  this task owns only the validator, its tests, and this plan/evidence.

## Required validation (authoritative)

```sh
python3 scripts/ci/tests/test_validate_log_import.py -v
```

Plus a direct CLI smoke test of `scripts/ci/validate_log_import.py` against a
fixture written to real files on disk (not just in-process function calls),
to prove the `argparse`/file-loading path independently of the fixture
harness.

## Paths to delete

None.

## Non-closure

No B.1 copy, no real `import-provenance.json`, no source approval, no
publication, and no Cargo/types/phase-policy change. Full B.1 remains blocked
on B.P3's accepted source handoff.
