# B.6 implementation and verification checklist

Every row receives an implementation pass followed by an independent verification pass. Lead completeness is separate from these developer checks.

| Criterion | Implement | Verify | Evidence |
| --- | --- | --- | --- |
| Synchronous owned/attached submit, context snapshot, one try_log, no event loop | pending | pending | |
| Prepare receipt before admission; return direct failures without retry | pending | pending | |
| Resolved admission state preserves accepted/filtered and retains no event | pending | pending | |
| Repeat/zero receipt waits validate then return without suspension/timer | pending | pending | |
| Native start_flush/state projection, no callback/GIL attachment/per-call worker | pending | pending | |
| Shared backend cap64 reserved before native call, including completed/unpolled | pending | pending | |
| One 1ms timer, monotonic deadline, release on rejection/completion/timeout/cancel | pending | pending | |
| Weak loop references, closed-loop reclamation and module teardown cleanup | pending | pending | |
| Timeout/cancel end observation only; overlap/late health/native slot semantics | pending | pending | |
| Contained boundary errors/accounting preserve exact tagged original outcomes | pending | pending | |
| N32 synchronized producers, responsive heartbeat under held writer/flush | pending | pending | |
| Real owned + core/bridge attached packaged fixtures and injected failures | pending | pending | |
| Multiple loops, native completion before next poll and observer saturation | pending | pending | |
| Loop closure/interpreter teardown/late shutdown subprocess checks | pending | pending | |
| Typed stubs/examples, debug asyncio/warnings strict, no obsolete receipt codes | pending | pending | |
| Full B.4a matrix and required Python validator | pending | pending | |
| Parent merge, complete handoff/hash evidence and documentation | pending | pending | |
