---
id: B.3b-native-runtime-handoff
status: complete
branch: feature/phase-b-3b-native-runtime
worktree: /Users/randlee/github/sc-observability-worktrees/feature/phase-b-3b-native-runtime
parent: fix/phase-b-1d-qa1
---

# Native binding runtime handoff

The supplied core and bridge backends implement the complete native Rust
contract. The core owner alone controls shutdown and level mutation; cloneable
backend handles expose admission, query, health and flush. The bridge adapter
never owns or shuts down the host's `LogGuard`. Four compile-fail documentation
fixtures enforce the public authority boundary. Existing core and bridge public
APIs are unchanged by this layer.

## Implementation and resource bounds

A backend reserves three parked helpers before constructing native core state:
operation, lifecycle and completion. All constructors share one fallibly
initialized process timer. Spawn failure aborts and joins only parked helpers;
the timer remains reusable. Operations retain immutable completion through
`OnceLock`; completed handles hold no backend/helper ownership. Query and flush
have separate one-operation slots and share FIFO execution. Native return
releases the slot before completion becomes visible. Observation expiry drops
only the observer, preserving pending native work and its slot.

Admissions use the contracted SeqCst open/register/recheck sequence. An admitted
call drops its temporary logger reference before unregistering. The lifecycle
worker closes admission, drains admitted calls/slots, consumes the logger once,
and publishes final health. Owner drop only signals. Bridge last-handle drop
closes only the adapter. Health after close reads the retained native snapshot;
failed helper completion reports Internal and never fabricates stopped health.

An operation reserves at most 64 combined synchronous/future/callback observers.
Each backend reserves at most 128 callbacks, including executing callbacks.
Cancellation physically removes queued callbacks and timer registrations. A
separate completion worker contains and counts callback panics. Native shutdown
and query progress independently of a held callback. All callback invocation,
native I/O, logger shutdown and helper joins occur outside bookkeeping locks.

The exact four workspace dependencies are `sc-observability`,
`sc-observability-types`, `sc-observability-dto`, and `sc-observability-log`.
`arc-swap` publishes immutable logger references and health without an admission
mutex; `serde_json` supports checked native event fields. There is no unsafe
code in this crate and no Tauri/PyO3 dependency. CI resolves the actual edges
and rejects aliased and target-specific injected host dependencies.

## Contract coverage

| Contract area | Executed evidence |
| --- | --- |
| Shared timer/rollback | Isolated timer spawn retry, initialization poison, all three helper failures, core constructor rollback, eight concurrent creations (25 helpers), cross-logger cancellation |
| Observers/deadlines | 64 combined observers including synchronous wait, zero/60000/invalid durations, timeout then saved completion, future cancellation, 128 repeated saved-result reads without helper growth |
| Native slots | Core and bridge query/flush saturation, cross-class FIFO, expiry retaining slots, slot release after completion; distinct binding/native overlap codes |
| Bridge timeout | Native timeout and overlap from own prior or external flush, eventual completion and explicit new barrier; no automatic runtime retry |
| Callback bounds | 128 reservations, overflow, cancellation/completion races, panic accounting, cancelled callbacks never invoked, blocked callback with independent native shutdown/query |
| Admission/lifecycle | 32 synchronized accepted producers and 32 close-crossing registrations, held sink with responsive producer/level change, repeated shutdown, retained read handles, failed helper |
| Teardown | Idle/running/completed last-handle cases, retained completed Operation with no retained coordinator, repeated bridge attachment/drop returning to one timer |
| Conversion/provenance | Both public backends and packaged consumer log/query/flush/health round trips; recursive normalized protected-key rejection; checked-in exact native diagnostic golden |
| Public authority | Four compile-fail fixtures: core/bridge read handles cannot shut down, core read handle cannot mutate level, owner cannot clone |

B3B-C01 is fixed in `296df34`: the test gate uses one absolute deadline and
asserts expiration. Only explicit release opens it. RAII `Release` guards
retain bounded cleanup when an assertion fails. No timeout silently permits
held native work to continue.

## API and packaged artifact evidence

The appointed lead reviewed public API digest
`4b3e55fcc66c721dabb43298f5a7948bdb326330190e45a75a443c93d0e9d99b`.
The actual export is retained under `evidence/b3b-api/`; the independently
issued record is copied verbatim to
`docs/api-approvals/phase-b-native-runtime.json`. Re-export after implementation
cleanup matches exactly. This is public-surface approval only.

The source-bundle consumer reuses B.3's packaging and isolation tools. It runs
both supplied backends from real normalized `.crate` archives with a frozen
lockfile, versioned dependency requirements and confined root patches. All
registered checkouts, ambient Cargo cache and network access are explicitly
denied and probed. Source-lock selections are preserved; six broken-bundle
copies fail with exact expected error codes. Qualified core/bridge archives
retain B.2 artifact provenance. No registry publication occurs.

## Qualification and limits

Source qualification is `94d891dd29f1e0ff61a531ca6afdac3115febce2`.
[CI run 35206736995](https://github.com/randlee/sc-observability/actions/runs/35206736995)
passed all three platform jobs, the sandboxed packaged consumer and aggregation.
The common qualified source digest is
`1268eb3e5dffec5717f9d266ddf63628b62aae5982553eee7e0a347e8e2eb4f3`.
The final evidence index is `evidence/b3b-final/`.
`checks.json` records passing fmt, native Clippy with warnings denied,
dependency/boundary/docs checks, and the full workspace suite (313 tests and
doctests passed; seven existing ignored tests). Those full checks ran at
`296df34`, whose only subsequent implementation change was the CI source-hash
portability correction. Six corrupted evidence copies are rejected separately.
The actual API re-export still matches the independently approved digest.
 The platform gate requires all
26 subprocess cases plus public integration and authority fixtures in debug and
release on Linux, macOS and Windows. It rejects missing platforms, skipped
cases, modified logs, stale source hashes and overwritten golden diagnostics.
The source digest includes runtime/dependency sources, tests, manifests, lockfile
and consumer/gate code, with explicit UTF-8 and stable path ordering.

`validate_binding_runtime.sh --platform-only` creates a platform cell;
`--consumer-only` creates isolated artifact proof; `--aggregate-only` requires
all cells and proof. The default runs local qualification and the consumer,
then requires the other retained platform cells. Missing platform evidence is a
failure. CI runs all cells, isolated consumer and aggregation in one workflow.

Implementation qualification does not grant consolidated independent QA,
owner-deferred contract acceptance, or B.7 registry publication approval.
