---
id: B.1c-QA1-fixes
status: in_progress
branch: fix/phase-b-1c-qa1
worktree: /Users/randlee/github/sc-observability-worktrees/fix/phase-b-1c-qa1
parent: feature/phase-b-1e-migration-validation
authoritative-sprint-doc: sprint-b-1c-observation-errors.md
---

# B.1c QA1 reconciled fixes

## Scope

Apply and verify FTQ-001, FTQ-002, RBP-F002, ATM-QA-001, and ATM-QA-002 only.
Preserve public signatures, routing/lifecycle behavior, health accounting, and
the success-only shutdown outcome. Full B.1c integration and independent QA are
separate gates.

## Required outcomes

- Bound concurrent shutdown and flush fixture completion without busy-spinning
  or unbounded joins, and prove in-flight shutdown remains safe for emit,
  flush, and health.
- Move blocking logger shutdown outside its mutex while keeping a coherent
  transition that never exposes `None` to current callers.
- Exercise custom and wrong-family diagnostic retention in subscriber and all
  projector adapters, both legacy-to-typed and typed-to-legacy.
- Retain actual workspace/doctest evidence at the tested SHA or correct stale
  counts in the handoff.

## Validation

Run affected observation tests and repeated concurrency fixtures, then format,
workspace clippy/doctests, public API semver/docs, and documentation checks.
This task remains open until coordinator completeness PASS.

## Implementation and verification passes

Assignee after the clean `708c85d` transfer: `bp-fix-helper`.
Final merged source: `fba711497b26bc9a0c7816d5352fdc8c0c4c64d3`.
Merged parent: `1164b0ca6d1c90b1220935a3d24b0a3afbbac798`.
The final documentation commit does not change this tested source.

| Finding | Implementation pass | Verification pass |
| --- | --- | --- |
| FTQ-001 | Eight concurrent shutdown calls send completion before any join; all receives are bounded. The controlled sink uses a bounded release receive and disconnect cleanup. | `concurrent_typed_shutdown_is_idempotent` and `in_flight_shutdown_preserves_flush_health_and_repeated_shutdown` pass, including 20 repeated batches. |
| FTQ-002 | The sink prepares its actual flush result before notifying the completion channel; the old counter spin is absent. | `flush_forwards_logger_flush_behavior_directly` passes 20 repeated runs, asserting exactly two flush calls per facade and one recorded flush failure. |
| RBP-F002 | The documented logger mutex protects an explicit Running/ShuttingDown/Stopped state. First shutdown joins outside it; repeated shutdown immediately returns success. Flush and health wait on the logger Condvar, releasing the mutex. Shutdown unwind notifies and resumes under the reacquired mutex, preserving poison behavior. | Both facade paths prove rejected emit, immediate repeated shutdown, blocked flush/health, final retained logging counters, writer diagnostics and sink health. Test-only signals under the logger mutex prove distinct callers reach both wait boundaries before negative assertions or unwind. Failed unwind fixtures release their waiters. |
| ATM-QA-001 | Log/span/metric custom and wrong-family cases cover both direct adapter directions. Subscriber coverage additionally checks native source type and one callback per operation. | Three family tests cover 12 cases with exact Unclassified kind, code, entire diagnostic, original context allocation, original native source pointer/type and one invocation. Existing real registration tests retain all four route families. |
| ATM-QA-002 | The preparation handoff narrows its unsupported historical count claim. Current raw workspace/doctest outputs retain command and immutable tested SHA. | Workspace: 269 non-doc tests and 9 doctests pass, with 7 doctests ignored; explicit doctest rerun: 9 pass, 7 ignored. Evidence is indexed below. |

Both passes are complete for the five reconciled findings. Coordinator
completeness and independent consolidated QA remain separate gates. Full B.1c
remains `proposed` in its authoritative sprint document.

## Retained execution evidence

Raw logs live under this worktree at
`.atm-task-lists/evidence/fba711497b26bc9a0c7816d5352fdc8c0c4c64d3/`.
`results.json` indexes the exact commands, return codes and log names. Each log
contains the immutable tested SHA. `repeated-concurrency.log` retains every
filtered invocation, 20 batches / 100 passing tests. The local work checklist is
`.atm-task-lists/phase-b-b1c-qa1-fixes.md`.

| Command | Result | Raw log |
| --- | --- | --- |
| `cargo fmt --all -- --check` | PASS | `fmt.log` |
| `cargo test --locked -p sc-observe --all-targets` | PASS: 16 unit + 1 routing + 10 typed tests | `observe.log` |
| `cargo test --locked --workspace` | PASS: 269 non-doc + 9 doc tests; 7 ignored doctests | `workspace.log` |
| `cargo test --locked --workspace --doc` | PASS: 9 doctests; 7 ignored | `doctests.log` |
| `cargo clippy --locked --workspace --all-targets --all-features -- -D warnings` | PASS | `clippy.log` |
| `bash scripts/ci/validate_docs_consistency.sh` | PASS | `docs.log` |
| `bash scripts/ci/validate_dependency_bans.sh` | PASS | `dependency-bans.log` |
| `bash scripts/ci/validate_repo_boundaries.sh` | PASS | `boundaries.log` |
| `bash scripts/ci/validate_public_api_diff.sh` | Expected exit 1: Phase B additive APIs, no removed/changed signatures | `api-diff.log` |
| `python3 scripts/ci/validate_public_api_semver.py` | PASS with existing approved deprecations; each published crate has 222 checks pass, one minor-version finding, 30 skips | `api-semver.log` |
| `bash scripts/ci/validate_public_api_docs.sh` | PASS | `api-docs.log` |
| `python3 scripts/ci/validate_error_migration.py` | PASS on the merged parent, additional integration check | `migration.log` |

The API semver result is the repository approval-aware gate outcome; it does
not claim all individual cargo-semver checks passed. The migration parent's
remaining work and consolidated landing QA do not change this scoped
observation implementation result.
