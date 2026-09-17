# B.6 implementation handoff — qualification in progress

Branch: `feature/phase-b-6-python-async`; draft PR 138.
Direct parent: `feature/phase-b-5-python-integration` (PR 139).
Parent checkpoint `2f11f75` is merged in `2bd3e87`. The implementation is
pushed; final parent propagation, distribution qualification and lead
completeness remain open. Publication belongs to B.7.

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

The final five-wheel/25-cell matrix must use the forthcoming direct parent with
the packaging owner's per-cell embedding and separate same-sdist fault companion.
Production wheels remain authoritative; public async tests stay in normal test
discovery. The inherited B.4 Rust lint cleanup is assigned to its owner. Final
source/archive/wheel hashes, CI run and aggregate proof will be added after those
gates pass. The developer verification checklist and lead completeness must both
finish before this task closes. No skipped matrix cell or development-only
artifact can qualify this sprint.
