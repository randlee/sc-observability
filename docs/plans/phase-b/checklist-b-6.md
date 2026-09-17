# B.6 implementation and verification checklist

Every row receives an implementation pass followed by an independent verification pass. Lead completeness is separate from these developer checks.

| Criterion | Implement | Verify | Evidence |
| --- | --- | --- | --- |
| Synchronous owned/attached submit, context snapshot, one try_log, no event loop | implemented | pending |  facade submit -> _submit -> log; test_submit_snapshots_context_and_nested_values_at_call_boundary |
| Prepare receipt before admission; return direct failures without retry | implemented | pending |  _submit preallocation; allocation failure and original-Failure fixtures |
| Resolved admission state preserves accepted/filtered and retains no event | implemented | pending |  Resolved/LogReceipt; no-event weakref and post-shutdown receipt fixtures |
| Repeat/zero receipt waits validate then return without suspension/timer | implemented | pending |  coroutine.send immediate StopIteration for zero/repeat/max; invalid timeout cases |
| Native start_flush/state projection, no callback/GIL attachment/per-call worker | implemented | pending |  NativeFlushOperation::state; no subscribe/completion callback or per-call worker |
| Shared backend cap64 reserved before native call, including completed/unpolled | implemented | pending |  NativeObserverIdentity/Permit atomic cap; real 64 completed/unpolled operations in all modes |
| One 1ms timer, monotonic deadline, release on rejection/completion/timeout/cancel | implemented | pending |  _Observer.poll/finish/release; zero, timeout, cancellation, rejection and foreign-error fixtures |
| Weak loop references, closed-loop reclamation and module teardown cleanup | implemented | pending |  WeakKeyDictionary/weak loop/atexit; real closed-loop reclamation and finalization subprocesses |
| Timeout/cancel end observation only; overlap/late health/native slot semantics | implemented | pending |  embedded timeout/overlap/cancel; original native bridge codes; late owned shutdown |
| Contained boundary errors/accounting preserve exact tagged original outcomes | implemented | pending |  every generated Failure unchanged; real broken-pipe health, queue-full, foreign boundary errors |
| N32 synchronized producers, responsive heartbeat under held writer/flush | implemented | pending |  embedded held-writer N32 ThreadPool and asyncio producers with live heartbeat in all modes |
| Real owned + core/bridge attached packaged fixtures and injected failures | implemented | pending |  installed wheel runtime suite plus embedded real core/bridge; full packaged matrix pending |
| Multiple loops, native completion before next poll and observer saturation | implemented | pending |  clock-controlled loop with real native flushes and alias handles; atomic permit thread fixture |
| Loop closure/interpreter teardown/late shutdown subprocess checks | implemented | pending |  owned held-pipe interpreter exit; core/bridge actual PyO3 finalization then native completion |
| Typed stubs/examples, debug asyncio/warnings strict, no obsolete receipt codes | pending | pending | |
| Full B.4a matrix and required Python validator | pending parent integration | pending | shared runner supports debug/warnings switches; final parent suite not inherited |
| Parent merge, complete handoff/hash evidence and documentation | pending | pending | |
