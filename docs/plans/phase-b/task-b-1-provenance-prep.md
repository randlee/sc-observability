---
id: B.1-provenance-prep
status: complete
qa_status: pending_independent_qa
merge_status: unmerged
branch: fix/phase-b-1-provenance-integrity
worktree: /Users/randlee/github/sc-observability-worktrees/fix/phase-b-1-provenance-integrity
target: fix/phase-b-1a-neutral-fixtures
base: develop
---

# B.1 provenance-prep — Build and prove the import/acceptance validator ahead of B.1

## Goal and dependencies

This is preparation tooling for [B.1](sprint-b-1-copy.md), not B.1 itself: it
builds and proves `scripts/ci/validate_log_import.py` (B.1 deliverable 3) in
parallel with B.P3's active source corrections and B.1a's neutral-API prep, so
the validator is ready the moment B.1's real copy has an accepted source. It
does not copy BTIT source, does not grant source approval, and does not
publish anything. Originally implemented on `feature/phase-b-1-provenance-prep`
(closed/frozen at `b69b8c5`, PR119); an independent lead-completeness recheck
of that closed layer reproduced four further bypasses (a self-consistently
"rejected" review/provenance verdict, a fake `handoff_revision`, an arbitrary
runtime change mislabeled with a permitted mechanical `kind`, and an extra
file reached through a symlinked destination directory), so the fix continues
on this new top-of-stack layer, `fix/phase-b-1-provenance-integrity`, cut from
`fix/phase-b-1a-neutral-fixtures`; the frozen layer is never re-edited. Merge
that parent forward before every round once it has pushed commits. Full B.1
closure, the real `docs/plans/phase-b/import-provenance.json`, and the real
copy remain blocked on B.P3's accepted source handoff.

## Deliverables (authoritative)

1. `scripts/ci/validate_log_import.py`: given `--source-repo` (BTIT),
   `--doc-repo` (sc-observability's own history; defaults to this repo root),
   `--destination` (defaults to this repo root), `--provenance`
   (`docs/plans/phase-b/import-provenance.json`) and `--handoff`
   (`docs/plans/phase-b/handoff-b-p3.md`), it enforces the full B.1 deliverable
   3 contract: a full 40-character source SHA cross-checked between the
   provenance record, the B.P3 handoff's own citation, and the source repo's
   accepted commit (looked up directly by SHA, never via the source repo's
   ambient `HEAD`, so a checkout that has advanced past acceptance does not
   invalidate a still-present immutable source); a handoff carrying an
   accepted verdict plus an immutable review-document path/commit, where the
   review document itself is resolved and read from real Git objects in the
   **source repo** (BTIT's own acceptance record) and must actually carry an
   "accepted" verdict covering the accepted source commit -- a
   provenance-declared verdict that is merely internally self-consistent
   (e.g. a matching "rejected") is not enough; a target-contract commit and a
   `handoff_revision` (`<path>@<full-40-character-commit-sha>`), both
   resolved in the **doc repo** (sc-observability's own documents), with the
   handoff-revision's cited content required to byte-match the `--handoff`
   document actually used; a per-file Git blob inventory diffed strictly
   against the source repo (BTIT history is never adapted, so any difference
   there means the provenance record itself is unreliable) and, for the
   destination tree, walked independently from the filesystem (never a
   recorded-path lookup, so an unrecorded extra file or an extra file reached
   through a symlinked directory cannot pass) with symlinks, non-regular
   files, and path escapes rejected before anything is read. A destination
   blob difference is allowed only when its path is explicitly listed in the
   provenance `adaptations` array with a non-empty reason, a `kind` drawn
   from a fixed mechanical set (`package_metadata`, `dependency_path`,
   `relocated_doc_or_test_path`), exact approved `before`/`after` content
   matching the recorded source blob and the actual destination blob, and
   every changed line between them matching that kind's own pattern -- the
   kind label alone does not authorize the change.
2. `scripts/ci/tests/test_validate_log_import.py`: fixture coverage built from
   two separate temporary, synthetic Git repositories (a `source_repo`
   modeling BTIT and a `doc_repo` modeling sc-observability's own history)
   and honest made-up review/handoff text -- never a fake production
   `import-provenance.json` or `handoff-b-p3.md`, neither of which exists
   yet. Covers the valid case, a declared `package_metadata` adaptation, a
   declared `dependency_path` relocation, a source checkout that has advanced
   past the accepted commit, and every required negative case: unaccepted
   handoff, missing review-document citation, a nonexistent accepted source
   commit, handoff/provenance SHA mismatch, extra and omitted files on the
   source and destination sides (including a file reached only through a
   symlinked destination directory), an undeclared destination content
   difference, escaping/out-of-scope inventory paths, an empty required-crate
   inventory, a fake/nonexistent review commit, a review document that does
   not cover the accepted source, a self-consistently "rejected" review and
   provenance verdict, a handoff/provenance review-citation mismatch, a
   nonexistent target-contract commit, a malformed or fake `handoff_revision`,
   a `handoff_revision` whose cited content does not match the `--handoff`
   document used, an adaptation missing a reason, an adaptation with a
   disallowed kind, an adaptation whose declared `before` content does not
   match the recorded source blob, and an arbitrary runtime content change
   mislabeled with a permitted kind (e.g. `dependency_path`).
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
