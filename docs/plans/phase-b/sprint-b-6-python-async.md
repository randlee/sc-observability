---
id: B.6
status: in_progress
branch: feature/phase-b-6-python-async
worktree: /Users/randlee/github/sc-observability-worktrees/feature/phase-b-6-python-async
base: feature/phase-b-5-python-integration
---

# B.6 — Fire-and-forget Python logging with optional async confirmation

## Goal and dependencies

Keep ordinary Python logging immediate and nonfatal, while letting selected
callers asynchronously observe submission or flush completion. `must_follow`
B.5: reuse B.4 runtime ownership and B.5 context capture/standard logging.
B.7 `must_follow` this sprint for publication. This closes an asyncio client
capability without selecting sc-runtime's parallel-worker architecture.
Shared Python runtime/conformance artifacts preclude parallel_safe execution.
Parent pushes trigger merge-forward before child development/fix rounds and
parent PR merge precedes child completion, as defined in the phase index.

## Deliverables (authoritative)

Every listed deliverable must land production-ready for this sprint's stated
scope. Completion requires evidence for every numbered item, including its code,
documentation and validation artifacts; partial completion leaves the sprint open.

1. Add `submit` to owned Logger and AttachedLogger, returning the receipt below,
   and implement async receipt waiting and flush in
   `bindings/python/sc-observability-py/python/sc_observability/async_logging.py`
   with any required PyO3 support. Keep existing `log(event) -> Result[Admission]` as the
   minimal nonblocking path, without requiring receipt allocation or an event loop.
   A caller may intentionally ignore its result.
2. Implement completion transfer between native operations and asyncio loops.
   Never block the event-loop thread on sink I/O, writer capacity, a native
   mutex, or a thread join. Completion belongs to the logger/backend rather
   than the lifetime of a waiting Python task.
   Use asyncio Future values resolved only with set_result. Poll B.3b's
   nonblocking Operation::state from the owning loop using loop.call_later with
   a 1 ms interval while pending; check the monotonic deadline on each callback.
   An already-resolved operation completes immediately. Cancel the timer and
   unregister the waiter on completion/cancellation. At most one timer per
   flush waiter is live. B.6 owns at most 64 flush observers per shared Python
   backend, including completed native calls awaiting their next loop poll.
   Reserve an observer before start_flush; capacity exhaustion returns
   queue_full/BINDING_WAITERS_FULL. Native slot rejection releases the reservation
   immediately without creating a timer. Completion/timeout/cancellation releases
   it and cancels its timer. Closed-loop registrations use weak loop references
   and are reclaimed before reserving another observer or during module teardown.
   This permits successive native calls without an unbounded observer backlog. No native
   thread acquires the GIL or invokes Python, and no Rust executor or thread is
   created per call. This consumes the B.3b native coordinator contract (native-binding-runtime.md)
   as projected by B.4 without a
   cross-thread interpreter-finalization race.
3. Extend the B.4 validator, typed stubs, packaged examples and
   `docs/plans/phase-b/handoff-b-6.md` with receipts ignored and repeatedly awaited, and async flush waits timed out,
   cancelled or completed after a loop closes. Exercise concurrent asyncio
   producers and the embedded Rust host under load and injected failures.

## Public signatures and result semantics

All results and error variants use B.3/B.4's discriminated contract; there are
no exception subclasses or nullable error fields attached to success states.

```python
@dataclass(frozen=True)
class Resolved:
    admission: Admission
    kind: Literal["resolved"] = field(default="resolved", init=False)

ReceiptState = Resolved

class LogReceipt:
    def state(self) -> ReceiptState: ...
    async def wait(self, timeout_ms: int = 2000) -> Result[Admission]: ...

# Added to Logger and AttachedLogger:
def submit(self, event: LogEvent) -> Result[LogReceipt]: ...
async def flush_async(self, timeout_ms: int = 2000) -> Result[Completion]: ...
```

```python
logger.log(event)  # Fire-and-forget: caller deliberately ignores the Result
submitted = logger.submit(event)  # Starts now, without requiring await
if submitted.kind == "ok":
    outcome = await submitted.value.wait()  # Optional result inspection
else:
    handle_at_application_level(submitted.error)
flushed = await logger.flush_async()  # Also a tagged Result, never an error raise
```

`submit` is a regular method, not an unscheduled coroutine. It captures an owned
snapshot of the event and correlation context at the call boundary and begins
nonblocking admission immediately, with no dependency on a running Python loop.
submit calls synchronous nonblocking HostLoggingBackend::try_log once.
An admission failure returns Err directly; a successful admission returns
Ok(LogReceipt) whose state is already Resolved(admission). No native admission
Operation or Pending state exists. The caller may intentionally ignore the
Result. Receipt allocation/conversion failure returns a typed failure without
retrying an already-submitted record; receipt creation is prepared before
native submission so expected allocation/conversion errors occur before admission.

An Ok admission preserves the accepted/filtered discriminator from the backend.
Filtered means no enqueue; accepted means queue admission, not writing or
durability. Queue-full/closed/validation/backend failures use the corresponding
Failure variant. Distinguish immediate submission failure from successful resolved admission; no successful result may hide a known failure.

Receipt waiting is optional and repeatable. After validating timeout_ms using
the shared input rules, wait returns Ok(saved admission) immediately without
suspending, registering a waiter, polling or allocating a timer. Zero is valid
and returns the saved result; there is no receipt-timeout path. Cancellation
before the coroutine body starts belongs to asyncio; once executing, this
immediate method contains no cancellation suspension point. Saved admission
remains inspectable after logger shutdown or loop closure.

flush_async alone polls Operation::state using the loop-local mechanism above.
Its timeout/cancellation returns the corresponding Err and ends observation,
not the native operation. Convert cancellation caught inside that public wait
boundary to the cancelled result; cancellation before task start is separately
controlled by asyncio. No warning, retry or unhandled Future exception is
created when log/submit results are ignored. Failed best-effort accounting
preserves the original outcome and never recurses through logging.

B.6 stores only caller-owned resolved receipts. There is no logger-owned pending
receipt registry, pending-receipt limit or per-receipt waiter registration.
Remove the proposed PY_PENDING_RECEIPTS_FULL and PY_RECEIPT_WAITERS_FULL codes
before first publication; they have no reachable failure path in this scope.
Native operation observer limits remain B.3b infrastructure for actual native
operations. B.6 Python wrappers own only their bounded flush-observer timers.
Receipt values use the shared bounded diagnostic conversion rules; no event
payload is retained. Keeping many resolved receipts is caller-owned memory.
An asynchronous admission transport may introduce pending receipts only in a
future separately reviewed contract; it is not silently assumed here.

At most one native async flush is in flight per logger. Every overlapping request
returns queue_full with code SC_OBSERVABILITY_BINDING_FLUSH_IN_PROGRESS; it does not
join an earlier barrier. A native bridge overlap returned through the backend
retains its distinct SC_OBSERVABILITY_LOG_FLUSH_IN_PROGRESS code. Observer timeout/cancellation alone leaves the adapter slot occupied. Core
flush releases it on actual core return. Bridge flush receives the validated
timeout via start_flush(timeout); native TimedOut resolves its adapter Operation
with that failure and releases only the adapter slot. The bridge slot may remain
occupied, so a new explicit request can receive the native overlap code from
this adapter’s own prior flush. Neither case triggers polling by resubmission. A flush result describes the core sink-flush
contract, not fsync durability; event ordering/barrier semantics must match the
core implementation. A flush request cannot report success merely because its
waiter was cancelled or its event loop closed.

A resolved receipt retains its admission outcome for later state/wait calls.
Shutdown late results remain observable through B.4 wait_stopped. Flush has no
receipt/status accessor in this release: after its caller times out or cancels,
core or bridge continues its native work once; the adapter follows the
mode-specific slot rules above and native health records later failures, but the caller cannot retrieve that prior flush's Result. A later explicit
flush starts a new barrier only after the native slot is free; it is not observation or retry of the previous one.
This limitation is documented in the example and tested, not hidden behind a
promise of universal late-result retrieval.

Loop-local polling leaves saved receipt/shutdown results and logger health
intact when a loop closes. No native completion path holds Python objects or
calls into an interpreter. Factories, validation, query, health and lifecycle
retain their Result contracts. Test a loop closing with every waiter timer
active and interpreter teardown while native flush is held; native work must
finish without attempting GIL attachment or reviving the closed loop.

## Acceptance criteria (authoritative)

- AC1: Unawaited log/submit calls produce expected records with correct context
  and no event-loop requirement; immediate and awaited tagged results match
  submission/admission outcomes and support exhaustive type narrowing.
  Ignored submit failures and resolved receipts generate no warnings or unhandled exceptions.
- AC2: Queue-full, formatting, disconnected/stopped host, writer failure, and
  failure in diagnostic accounting produce the expected Err variant instead of throwing, rejecting or aborting
  application work. Ignoring the result remains the caller's choice. Fault-free configured logging delivers expected records.
- AC3: Concurrent producers leave an asyncio heartbeat responsive during blocked
  writer/flush operations. Outstanding work and retained receipt memory remain
  bounded by one adapter flush slot and 64 Python flush observers/timers; no receipt
  registry exists. N=32 synchronized producers with sufficient writer capacity
  never receive DISPATCH_FULL due to concurrency. Test caller-held resolved
  receipt memory separately from native operation retention.
- AC4: Timeout, task cancellation, multiple waiters, loop closure and host
  shutdown preserve exactly one underlying operation. Receipts retain their already-resolved admission;
  shutdown supports observable late Results; timed/cancelled flush supports only
  eventual health accounting and slot release, without a prior-result accessor.
  Executing flush waits resolve cancellation/timeout variants; receipt wait
  returns immediately after validation with no timeout/cancellation suspension; cancellation
  before task start is tested separately from library completion.
  Admission, flush completion and durability claims are distinguished in examples.

## Required validation (authoritative)

```sh
bash scripts/ci/validate_python_bindings.sh
```

Extend the script to run packaged asyncio tests with debug mode/warnings treated
as failures, deterministic held-writer tests, heartbeat and resource-bound checks,
resolved-receipt repeated/zero-timeout tests, invalid-timeout tests,
multiple-loop flush-completion tests, 64-observer saturation and the native-
completion-before-next-poll race, closed-loop reservation reclamation, and loop/interpreter shutdown
subprocess tests. Run both owned and attached modes on the B.4a Python/platform
matrix. Use deterministic operation counts and bounded outstanding-work checks
rather than a throughput benchmark as correctness evidence.

## Current qualification evidence (updated 2026-09-18)

Status: `in_progress` is accurate, not stale. Remaining closure is the same
B.4a 25-cell installed-distribution matrix B.5 depends on (see
`sprint-b-4a-python-packaging.md`'s current qualification evidence): run
`35295219562` completed with `conclusion: failure` (diagnostic-only, expected
per the known-incomplete Windows process-tree fixture); the acceptance-grade
run has not started. B.6's own implementation/local-check evidence is
recorded in `handoff-b-6.md` and `checklist-b-6.md`; lead completeness
review and the packaged 25-cell matrix are separately still open.

## Paths to delete

None.

## Non-closure

No asynchronous admission transport or Pending receipt state, sc-runtime worker pool, subprocess IPC, subinterpreter/free-threaded support,
worker fairness policy, cross-worker global ordering, per-record persistence
receipt, prior-flush-result accessor after timeout/cancellation, or durable-on-disk guarantee. Those require a runtime/transport spec.
Publication is B.7. Existing B.4/B.5 runtime behavior must remain production-ready;
this sprint adds optional waiting without making it necessary for logging.

## Technical reference

[Python coroutines and tasks](https://docs.python.org/3/library/asyncio-task.html)
distinguishes coroutine creation from execution; therefore immediate submit is
separate from the explicitly awaited receipt method.
