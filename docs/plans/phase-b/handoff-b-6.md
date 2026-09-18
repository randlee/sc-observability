# B.6 implementation handoff — qualification in progress

Branch: `feature/phase-b-6-python-async`; draft PR 138.
Direct parent: `feature/phase-b-5-python-integration` (PR 139).
Parent checkpoint `2f11f75` is merged in `2bd3e87`. The implementation is
pushed; distribution qualification has since passed in full, and the lead
completeness review recorded PASS -- see "Terminal qualification" below.
Publication belongs to B.7.

## Implemented behavior

Both owned and attached loggers expose synchronous `submit`. Receipt storage is
prepared before calling the existing context-aware `log` path exactly once.
A rejection preserves the original Failure; successful receipts already contain
the accepted/filtered admission. Receipts retain neither payload nor logger.
Repeated `wait`, including zero timeout, validates then returns the saved Result
without suspension, registration or timers. Ignored results create no warnings.

`flush_async` reserves capacity before one native `start_flush`, then reads
nonblocking `Operation::state` from one loop-local 1 ms timer with an absolute
monotonic deadline. Future completion uses only `set_result`. Timeout and caught
cancellation release observation without cancelling or resubmitting native work.
The shared opaque Python identity owns native atomic permits, bounding concurrent
Python observers at 64 across attached wrappers and loop threads, including
native-completed calls not yet polled. Permits release once on completion,
rejection, timeout, cancellation or destruction. Weak loop registrations reclaim
closed loops before another reservation and during module teardown.

No native completion retains a Python object, acquires the GIL, invokes a Python
callback or creates a per-call worker. Core/bridge slot differences and the lack
of a prior-flush-result accessor remain explicit. Saved receipts and B.4's
retained shutdown results survive observation cancellation and loop closure.

## Verification coverage

The two Python async test files cover preallocation failure, exact admission,
every declared Failure, repeated immediate waits, invalid timeout inputs,
cancellation before and after task start, native rejection, contained foreign
errors, timer cleanup and shared capacity. Actual installed-extension tests
exercise synchronized 32-thread and asyncio producers, context/payload snapshots,
caller-owned receipt memory, closed backends, real broken-pipe writer health,
atomic permit contention and interpreter exit with a held OS pipe.

The Rust embedding executable runs the existing B.5 shared request, then actual
owned/core-attached/bridge-attached held-writer scenarios. It checks heartbeat
responsiveness, bounded queue saturation, native overlap, cancellation and
timeout without resubmission, closed-loop reclamation and the 64-observer
native-completion-before-next-poll race. That race freezes only the fixture's
loop clock while each actual native operation completes; no backend/completion
mock substitutes for the native work. Separate core/bridge processes initialize
and finalize Python once while the native flush is held, then release it and
verify native completion and owner shutdown after interpreter finalization.

Root and async stubs support Python 3.10 exhaustive Result/state narrowing.
Packaged examples distinguish queue admission, sink-flush completion and fsync
durability, and show explicit owned lifecycle. The shared qualification contract
opts into asyncio debug, warnings as errors and embedding in every matrix cell.
The existing complete test discovery and sole source-bundle/distribution tools
are reused. B.6 uses no instrumented `_test*` native hooks.

## Current evidence and remaining gates

The local full Python binding validator passes 78 installed tests, strict Python
3.10 typing, both packaged Python examples, native Rust checks and the real
embedding/finalization suite. A separate actual CPython 3.14 host run at
`2bd3e87` passes all owned/core/bridge scenarios and both finalization children.
All 17 DTO conversion tests pass after the parent's central registry correction.
Raw checkpoint logs and hashes at source `2bd3e87` are retained in
`evidence/b6-local/index.json`; they are local evidence, not a matrix claim.

Production wheels remain authoritative; public async tests stay in normal test
discovery. The inherited B.4 Rust lint cleanup is assigned to its owner. No
skipped matrix cell or development-only artifact can qualify this sprint.

## Terminal qualification (updated 2026-09-18)

The five-wheel/25-cell matrix has since run to completion:
[run 35303039765](https://github.com/randlee/sc-observability/actions/runs/35303039765)
at source SHA `c6d794c5d8c12a69938b2ec3ccd1cec24d1abd18` finished
`conclusion: success`, all 33 jobs, with `embedding_in_each_cell` enabled and
per-cell embedding as B.6 requires. All 25 `installed-suite` cells ran the
complete installed tests directory, including B.6's own async suite, and the
`aggregate` job cross-checked JUnit/hashes/source SHA/test identities across
all cells. This supersedes the provisional single-platform preflight above,
which is retained as history. Final source/archive/wheel hashes and the
aggregate proof are recorded in the shared inventory referenced by
`sprint-b-4a-python-packaging.md` and `handoff-b-4a.md`. The developer
verification checklist is recorded in `checklist-b-6.md`. B.6's development
qualification is therefore complete.

**Lead completeness decision: PASS** (aobs, 2026-09-18), recorded in
`handoff-b-4a.md`. Independent phase-end QA remains pending; formal API/ADR
approval and publication remain deferred to B.7.

## Provisional distribution preflight

Source `98203459bb8779b29b610be4649d55cb4063b07f` produced sdist SHA-256
`a12415ae265f894874e2ec38e5140967f7615a07f3f9762d6c041b95434054b4`
and macOS ARM64 ABI3 wheel SHA-256
`0153ef8c222fb98279e2a63690fea927c59d0a83fb198ddb543fb680342ee6b9`.
The isolated build passed native tests, the real B.5/B.6 embedding executable
and all nine negative package checks. That same wheel passed 78 installed tests
and strict typing under CPython 3.14 with debug/warnings enabled. This is a
provisional single-platform probe, not the final combined matrix or per-cell
embedding claim. Full machine records are retained with hashes under
`evidence/b6-local`; the final matrix supersedes this checkpoint.

The initial preparation exposed B6-P01: ignored `__pycache__` files were copied
into the inventory but excluded by maturin. Removing generated caches allowed
this probe; the packaging owner is implementing a tracked-source-only copy and
regression in the sole helper. B6-R01 moves finalization command dispatch into
its fixture module so the shared host main meets the Rust lint size bound.
