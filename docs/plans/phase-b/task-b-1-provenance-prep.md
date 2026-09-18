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
file reached through a symlinked destination directory), so the fix continued
on this new top-of-stack layer, `fix/phase-b-1-provenance-integrity`, cut from
`fix/phase-b-1a-neutral-fixtures`; the frozen layer is never re-edited. A
second lead-completeness recheck (at this layer's own `8aeabb5`) reproduced two
further bypasses surviving the first round: a target-contract commit that
existed but was checked for existence only, never for actually holding the
target document or for its reference being cross-checked against the
immutable handoff; and the `relocated_doc_or_test_path` mechanical-kind
pattern matching `.rs`/`.md` as a bare substring anywhere in a changed line
(including inside an unrelated string literal), rather than only in genuine
path/mod/include syntax. Both are fixed on this same layer. A third
lead-completeness recheck (at this layer's own `0a6fa29`) found that the
fullmatch tightening validated per-line syntax shape but not mechanical
equivalence: appending a whole new dependency (with arbitrary features) under
`dependency_path`, or a whole new `mod` declaration under
`relocated_doc_or_test_path`, both passed because each added line was
syntactically valid on its own, even though neither relocates anything that
already existed. Fixed with structural before/after comparison (dependency
table/entry set-equality plus non-location-key preservation; `mod` identity
set-equality and `include!`/`#[path]`/bare-path-literal count-equality), also
on this same layer. Merge the parent forward before every round once it has
pushed commits. Full B.1 closure, the real
`docs/plans/phase-b/import-provenance.json`, and the real copy remain blocked
on B.P3's accepted source handoff.

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
   (e.g. a matching "rejected") is not enough; a target document
   (`target_document: {path, commit}`, mirroring `review_document`) and a
   `handoff_revision` (`<path>@<full-40-character-commit-sha>`), both
   resolved in the **doc repo** (sc-observability's own documents). The
   target document's own citation (`Target document: `<path>` at commit
   `<sha>``) is required in the handoff and cross-checked against the
   provenance record exactly, and the document itself must actually exist at
   the cited commit -- an existing commit that holds only unrelated content
   is not enough, closing an existence-only-check bypass. The
   handoff-revision's cited content is required to byte-match the `--handoff`
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
   every changed line between them **fully matching** (not merely
   containing) that kind's own narrow syntax pattern -- the kind label alone
   does not authorize the change, and a changed line that only incidentally
   contains a keyword or extension substring (e.g. `.rs` inside an unrelated
   string literal) does not either. Per-line syntax alone cannot distinguish a
   genuine relocation from a wholesale new addition that merely has valid
   syntax on its own line, so `package_metadata`/`dependency_path` are further
   restricted to files named `Cargo.toml`, and `dependency_path`/
   `relocated_doc_or_test_path` additionally require structural before/after
   equivalence: the same dependency tables and entries must be present on both
   sides with only their location keys (`path`/`version`/`git`/`branch`/`rev`/
   `tag`) differing, and the same `mod` names (set-equality) and the same
   count of `include!`/`#[path=...]`/bare-quoted-path declarations (which
   carry no identity independent of the path argument itself) must be present
   on both sides -- a whole new dependency or module appended alongside a
   real relocation is rejected even when its own line is syntactically valid.
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
   missing or nonexistent target-document citation, a real (true orphan)
   commit that exists but does not hold the target document at the cited
   path, a handoff/provenance target-document citation mismatch, a malformed
   or fake `handoff_revision`, a `handoff_revision` whose cited content does
   not match the `--handoff` document used, an adaptation missing a reason,
   an adaptation with a disallowed kind, an adaptation whose declared
   `before` content does not match the recorded source blob, an arbitrary
   runtime content change mislabeled with a permitted kind (e.g.
   `dependency_path`), a changed line merely containing a `.rs`/`.md`
   substring inside unrelated syntax (e.g. a string literal) mislabeled
   `relocated_doc_or_test_path`, a `dependency_path`/`package_metadata` kind
   applied to a non-`Cargo.toml` file, a `dependency_path` adaptation that
   appends a whole new dependency table or a whole new entry to an existing
   table, a `dependency_path` relocation that also changes a dependency's
   non-location keys (e.g. `features`), and a `relocated_doc_or_test_path`
   adaptation that appends a whole new `mod` declaration alongside a genuine
   relocation.
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
