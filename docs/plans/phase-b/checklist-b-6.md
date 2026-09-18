# B.6 implementation and verification checklist

Every row receives an implementation pass followed by an independent verification pass. Lead completeness is separate from these developer checks and is recorded PASS in `handoff-b-4a.md` (aobs, 2026-09-18). The local verification pass inspected the implementation and fixtures and executed the full required gate at `2bd3e87`; raw logs and hashes are in `evidence/b6-local/index.json`. The packaged 25-cell matrix has since passed in run 35303039765 -- see row below.

| Criterion | Implement | Verify | Evidence |
| --- | --- | --- | --- |
| Synchronous owned/attached submit, context snapshot, one try_log, no event loop | implemented | local pass |  facade submit -> _submit -> log; test_submit_snapshots_context_and_nested_values_at_call_boundary |
| Prepare receipt before admission; return direct failures without retry | implemented | local pass |  _submit preallocation; allocation failure and original-Failure fixtures |
| Resolved admission state preserves accepted/filtered and retains no event | implemented | local pass |  Resolved/LogReceipt; no-event weakref and post-shutdown receipt fixtures |
| Repeat/zero receipt waits validate then return without suspension/timer | implemented | local pass |  coroutine.send immediate StopIteration for zero/repeat/max; invalid timeout cases |
| Native start_flush/state projection, no callback/GIL attachment/per-call worker | implemented | local pass |  NativeFlushOperation::state; no subscribe/completion callback or per-call worker |
| Shared backend cap64 reserved before native call, including completed/unpolled | implemented | local pass |  NativeObserverIdentity/Permit atomic cap; real 64 completed/unpolled operations in all modes |
| One 1ms timer, monotonic deadline, release on rejection/completion/timeout/cancel | implemented | local pass |  _Observer.poll/finish/release; zero, timeout, cancellation, rejection and foreign-error fixtures |
| Weak loop references, closed-loop reclamation and module teardown cleanup | implemented | local pass |  WeakKeyDictionary/weak loop/atexit; real closed-loop reclamation and finalization subprocesses |
| Timeout/cancel end observation only; overlap/late health/native slot semantics | implemented | local pass |  embedded timeout/overlap/cancel; original native bridge codes; late owned shutdown |
| Contained boundary errors/accounting preserve exact tagged original outcomes | implemented | local pass |  every generated Failure unchanged; real broken-pipe health, queue-full, foreign boundary errors |
| N32 synchronized producers, responsive heartbeat under held writer/flush | implemented | local pass |  embedded held-writer N32 ThreadPool and asyncio producers with live heartbeat in all modes |
| Real owned + core/bridge attached packaged fixtures and injected failures | implemented | passed |  installed wheel runtime suite plus embedded real core/bridge; full packaged matrix passed in run 35303039765 |
| Multiple loops, native completion before next poll and observer saturation | implemented | local pass |  clock-controlled loop with real native flushes and alias handles; atomic permit thread fixture |
| Loop closure/interpreter teardown/late shutdown subprocess checks | implemented | local pass |  owned held-pipe interpreter exit; core/bridge actual PyO3 finalization then native completion |
| Typed stubs/examples, debug asyncio/warnings strict, no obsolete receipt codes | implemented | local pass | root/async stubs; strict Python 3.10 fixtures/example; full gate78 tests with debug/warnings; obsolete-code scan clean |
| Full B.4a matrix and required Python validator | implemented | passed | run 35303039765 (source `c6d794c5d8c12a69938b2ec3ccd1cec24d1abd18`, `conclusion: success`, all 33 jobs, `embedding_in_each_cell` enabled) -- see handoff-b-6.md "Terminal qualification" |
| Parent merge, complete handoff/hash evidence and documentation | implemented | passed | direct B.5 checkpoint2f11f75 merged in2bd3e87; final combined-candidate evidence recorded in handoff-b-6.md "Terminal qualification" |

## Findings and follow-up verification

| Finding | Implement | Verify | Evidence |
| --- | --- | --- | --- |
| B6-R01: async finalization dispatch pushed the shared host main above its lint size bound | implemented | pass | dispatch moved to async_conformance::finalize_if_requested; cargo fmt and host clippy --all-targets --no-deps -D warnings pass |
| B6-P01: ignored Python caches copied into distribution inventory but omitted by maturin | fixed: source assembly now copies only Git-tracked Python/embedding inputs (`handoff-b-4a.md`) | passed in final parent | initial prepare fails on inventoried .pyc; cache-free provisional sdist/build and installed cell pass; run 35303039765's `sdist`/`installed-suite` jobs pass with the tracked-copy fix in place |
