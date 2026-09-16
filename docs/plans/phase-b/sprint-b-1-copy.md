---
id: B.1
status: proposed
branch: feature/phase-b-1-copy
base: develop
---

# B.1 — Copy the corrected generic BTIT crates

## Goal and entry gate

This section is the authoritative copy-entry gate; other Phase B documents
link here rather than define competing entry criteria.

Copy only the accepted implementation of the
[sc-observability-owned target bridge API](target-bridge-api.md), with mechanical
workspace adaptation. The target contract is reviewed/locked now so BTIT can
complete its initial implementation against it. This is not a post-copy API
redesign and does not freeze the unfinished inspected baseline.

Entry also requires the [runtime-level prerequisite](runtime-level-contract.md):
published core support and accepted BTIT integration, including release-mode
elevation evidence.

Entry requires the recorded approved target-contract commit, completed BTIT
initial design/implementation, resolved critical-review findings and accepted
re-review, plus the full immutable source commit implementing that contract.
The authoritative acceptance record is B.P3's
`docs/plans/phase-b/handoff-b-p3.md`. It must cite the actual BTIT-side critical
review/re-review document by repository-relative path and immutable document
commit, record its accepted verdict and the exact source SHA reviewed, and
identify the sc-observability acceptance of that handoff. A historical review
filename or a moving branch link does not establish acceptance. The accepted
source SHA in that handoff must equal the SHA in `import-provenance.json` and
the source commit read by the import validator. Neither `f6f69dc` nor `5fd63ca`
is an approved import SHA. There is no BTIT semver or
backward-compatibility obligation for this initial destination public API;
source parity is checked against the newly accepted target, not legacy behavior.

## Dependencies

- `must_follow`: B.P3 accepted source handoff, incorporating target contract
  approval, B.P2 published core and BTIT completed implementation/critical review.
- B.1a `must_follow` B.1; no binding sprint, publication or second public-API
  redesign is part of B.1.

## Deliverables (authoritative)

Every listed deliverable must land production-ready for this sprint's stated
scope. Completion requires evidence for every numbered item, including its code,
documentation and validation artifacts; partial completion leaves the sprint open.

1. Copy the complete reviewed `crates/sc-observability-log/`,
   `crates/sc-observability-log-macros/`, and CI-only
   `crates/sc-observability-log-consumer-check/` trees, including Rust source,
   docs, tests, UI fixtures, licenses, and macro-consumer evidence. Do not copy
   BTIT's transitional workspace root, Tauri application, UI, paths, or policy.
2. Wire root `Cargo.toml`/`Cargo.lock`, workspace lints, test dependencies, and
   CI so the copied packages build against this repo's public core. Both library
   crates temporarily retain `publish = false`; the consumer check stays private
   permanently. Preserve the exact bridge-to-macro version pin. Allowed source
   differences are package metadata, inherited dependency paths/versions, and
   paths in tests/docs needed for relocation. Do not alter runtime algorithms,
   public signatures, or diagnostic behavior.
3. Create `docs/plans/phase-b/import-provenance.json` and
   `scripts/ci/validate_log_import.py`. Record repository URL, accepted full SHA,
   approved target-contract commit, the accepted B.P3 handoff revision and its
   immutable BTIT review document/commit citations, source design/review verdicts,
   every copied file's source Git blob ID, and each
   permitted adaptation with reason. The validator accepts `--source-repo PATH`,
   verifies the immutable source and complete file inventory, and rejects any
   unexplained addition, deletion, or content difference. Verify the complete
   exported API/impl inventory against the approved target matrix, including
   additions made after the provisional source inspection.
4. Update the affected dependency/structural CI allowlists and normative
   dependency diagram only to recognize the companion crates. Resolve any
   conflict with TYP-030 by an explicit scoped exception for the copied bridge's
   public errors/health types, documented in requirements/API checklist; keep
   existing core ownership intact. Add `docs/api-approvals/phase-b-log-import.md`
   with Scope/Approval/Affected Artifacts and actual reviewer approval before
   merge. Store execution evidence in `docs/plans/phase-b/handoff-b-1.md`.

The [BTIT handoff record](btit-api-handoff.md) describes the joint gate.
Destination signatures and intentional behavior changes are settled in the
target contract before BTIT's final implementation and this copy, not invented
while copying. The completed source must carry the accepted contract reference.

## Boundary samples

Destination manifest shape (versions inherit this workspace; do not hard-code
an old BTIT version):

```toml
[package]
version.workspace = true
edition.workspace = true
license.workspace = true
rust-version.workspace = true
repository.workspace = true
publish = false

[lints]
workspace = true
```

`sc-observability-log` depends on the macros through an exact `=V` version plus
workspace path; `V` equals both copied packages' workspace version. The macros
crate must not acquire a dependency back to the bridge. The consumer-check
package must depend directly only on the bridge, preserving the external
macro-expansion hygiene check.

## Acceptance criteria (authoritative)

- AC1: The provenance validator covers the entire accepted source tree and
  allows only the recorded mechanical changes. The copied runtime/public API
  matches the accepted locked target API and its BTIT implementation; both
  contract approval and completed source/review acceptance are present. The
  accepted source SHA in handoff-b-p3.md equals import-provenance.json's source
  SHA and the exact imported Git commit. The handoff's immutable review citation
  explicitly covers that SHA; stale, absent or mismatched evidence fails AC1.
- AC2: All imported compatibility, formatter/panic, lifecycle, health, and UI
  tests pass on macOS/Linux/Windows, alongside existing workspace tests; no
  ignored failing fixtures or regeneration used to disguise behavioral changes.
- AC3: Existing core dependency/API invariants hold; neither core crate gains
  `log`, Tauri, Specta, PyO3, or any dependency on the copied crates. Every
  governance exception is scoped and approved; the three packages remain
  unpublished in this sprint.

## Required validation (authoritative)

From repo root; `BTIT_SOURCE_REPO` is the local checkout containing the accepted
commit, recorded in the handoff:

```sh
python3 scripts/ci/validate_log_import.py --source-repo "$BTIT_SOURCE_REPO"
cargo fmt --all -- --check
cargo test --locked --workspace --all-targets
cargo test --locked --workspace --doc
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo check --locked -p sc-observability-log-consumer-check
bash scripts/ci/validate_dependency_bans.sh
bash scripts/ci/validate_repo_boundaries.sh
bash scripts/ci/validate_docs_consistency.sh
```

The import validator must reject a missing/unaccepted B.P3 handoff, missing
review-document path/immutable commit, a review verdict covering a different
source commit, or a mismatch between handoff and provenance SHAs. Include one
valid immutable handoff fixture and negative fixtures for each of those cases;
never substitute the previously inspected source or a conventional review path.

The imported API freeze/consumer fixtures validate the new private packages;
existing published-core API/semver gates must also remain green in normal CI.
A pre-existing failure needs recorded disposition; it cannot be silently waived.

## Paths to delete

None. No files in BTIT are deleted or edited by this sprint.

## Non-closure

No publish, BTIT dependency switch, new runtime design, binding, or claim that
moving code itself closes unresolved source review findings. If mechanical
adaptation cannot preserve the accepted source contract, stop this copy scope
and return a concrete incompatibility finding before planning additional work.
