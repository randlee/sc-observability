# B.6 implementation and verification checklist

Every row receives an implementation pass followed by an independent verification pass. Lead completeness is separate from these developer checks.

| Criterion | Implement | Verify | Evidence |
| --- | --- | --- | --- |
| Synchronous owned/attached submit, context snapshot, one try_log, no event loop | implemented | pending | |
| Prepare receipt before admission; return direct failures without retry | implemented | pending | |
| Resolved admission state preserves accepted/filtered and retains no event | implemented | pending | |
| Repeat/zero receipt waits validate then return without suspension/timer | implemented | pending | |
| Native start_flush/state projection, no callback/GIL attachment/per-call worker | implemented | pending | |
| Shared backend cap64 reserved before native call, including completed/unpolled | implemented | pending | |
| One 1ms timer, monotonic deadline, release on rejection/completion/timeout/cancel | implemented | pending | |
| Weak loop references, closed-loop reclamation and module teardown cleanup | implemented | pending | |
| Timeout/cancel end observation only; overlap/late health/native slot semantics | implemented | pending | |
| Contained boundary errors/accounting preserve exact tagged original outcomes | implemented | pending | |
| N32 synchronized producers, responsive heartbeat under held writer/flush | implemented | pending | |
| Real owned + core/bridge attached packaged fixtures and injected failures | implemented | pending | |
| Multiple loops, native completion before next poll and observer saturation | implemented | pending | |
| Loop closure/interpreter teardown/late shutdown subprocess checks | implemented | pending | |
| Typed stubs/examples, debug asyncio/warnings strict, no obsolete receipt codes | pending | pending | |
| Full B.4a matrix and required Python validator | pending parent integration | pending | shared runner supports debug/warnings switches; final parent suite not inherited |
| Parent merge, complete handoff/hash evidence and documentation | pending | pending | |
