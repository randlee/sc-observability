---
id: B.6
status: proposed
branch: feature/phase-b-6-python-async
base: develop
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
   registered waiter is live; the existing 64-waiter bound applies. No native
   thread acquires the GIL or invokes Python, and no Rust executor or thread is
   created per call. This consumes B.4's defined native coordinator without a
   cross-thread interpreter-finalization race.
3. Extend the B.4 validator, typed stubs, packaged examples and
   `docs/plans/phase-b/handoff-b-6.md` with receipts ignored, awaited, timed out,
   cancelled and completed after a loop closes. Exercise concurrent asyncio
   producers and the embedded Rust host under load and injected failures.

## Public signatures and result semantics

All results and error variants use B.3/B.4's discriminated contract; there are
no exception subclasses or nullable error fields attached to success states.

```python
@dataclass(frozen=True)
class Pending:
    kind: Literal["pending"] = field(default="pending", init=False)

@dataclass(frozen=True)
class Resolved:
    result: Result[Admission]
    kind: Literal["resolved"] = field(default="resolved", init=False)

ReceiptState = Union[Pending, Resolved]

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
Failure to start returns Err; successful start returns Ok(LogReceipt). The
receipt retains the final admission Result, including any later error.
B.4's in-process try_log backend may resolve the receipt before submit returns.
A caller ignoring the returned Result causes no warning, escalation, or retry.

An Ok admission preserves the accepted/filtered discriminator from the backend.
Filtered means no enqueue; accepted means queue admission, not writing or
durability. Queue-full/closed/validation/backend failures use the corresponding
Failure variant. Distinguish immediate submission failure, pending confirmation,
and resolved admission explicitly; no successful result may hide a known failure.

Waiting is optional and repeatable. A waiter timeout/cancellation resolves Err
with the timeout/cancelled variant and only ends that wait; it cannot retract
admission, cancel a shared flush, or start a retry. Convert asyncio cancellation
caught inside the public wait/flush boundary into the cancelled result and
release waiter resources. Document that callers inspect this result rather than
expecting the library to raise CancelledError. A task cancelled before its body
starts or a loop that never executes it is controlled by asyncio; the submission
still exists independently and remains inspectable through the receipt. There
is no exception-based cancellation API implemented inside the library.

Other receipt waiters can still observe final admission completion. An ignored receipt never
creates an unawaited-coroutine warning, unhandled Future exception, or logger-owned
collection that grows without bound. Best-effort health accounts for failures
even when no receipt is observed. If accounting fails, preserve the original
Result without raising or attempting to log the accounting failure.

Do not add an unbounded queue/thread pool in front of the bounded Rust queue.
Receipt completion stores bounded status/diagnostic data after admission,
not the submitted payload. Native retention is limited to 64 pending receipts per
logger and 64 waiters per receipt; exceeding either returns queue_full with a
distinct stable operation code before allocating more work. Completed receipts
are caller-owned and removed from the native pending registry immediately.
Retaining arbitrarily many completed receipts is caller-owned memory, not a
claim of constant total process memory. Receipt payloads use the shared bounded Failure conversion contract: per-string
4096 UTF-8 bytes and at most 32 remediation steps. Oversized foreign diagnostics
produce the explicit binding validation result with code
SC_OBSERVABILITY_BINDING_DIAGNOSTIC_TOO_LARGE; do not truncate into an undeclared
metadata field or claim a native failure was preserved verbatim. The native
operation/health evidence remains its source of truth, while the receipt retains
the representable conversion failure. Receipt-limit failure uses
SC_OBSERVABILITY_PY_PENDING_RECEIPTS_FULL; waiter-limit failure uses
SC_OBSERVABILITY_PY_RECEIPT_WAITERS_FULL. Both use Failure.queue_full and neither
starts extra work or silently retries.

At most one native async flush is in flight per logger. Every overlapping request
returns queue_full with code SC_OBSERVABILITY_BINDING_FLUSH_IN_PROGRESS; it does not
join an earlier barrier. A native bridge overlap returned through the backend
retains its distinct SC_OBSERVABILITY_LOG_FLUSH_IN_PROGRESS code. The slot remains occupied until actual completion even
if the caller times out or cancels. A flush result describes the core sink-flush
contract, not fsync durability; event ordering/barrier semantics must match the
core implementation. A flush request cannot report success merely because its
waiter was cancelled or its event loop closed.

A completed admission receipt retains its Result for later state/wait calls.
Shutdown late results remain observable through B.4 wait_stopped. Flush has no
receipt/status accessor in this release: after its caller times out or cancels,
the native operation finishes once, updates health on failure and releases its
slot, but the caller cannot retrieve that prior flush's Result. A later explicit
flush starts a new barrier; it is not observation or retry of the previous one.
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
  Ignored failure receipts generate no warnings or unhandled exceptions.
- AC2: Queue-full, formatting, disconnected/stopped host, writer failure, and
  failure in diagnostic accounting produce the expected Err variant instead of throwing, rejecting or aborting
  application work. Ignoring the result remains the caller's choice. Fault-free configured logging delivers expected records.
- AC3: Concurrent producers leave an asyncio heartbeat responsive during blocked
  writer/flush operations. Outstanding work and retained receipt memory remain
  bounded by the 64-pending/64-waiter/one-flush limits and do not grow with
  historical calls after completed receipts leave the registry. Test caller-held
  receipt memory separately from logger-owned retention.
- AC4: Timeout, task cancellation, multiple waiters, loop closure and host
  shutdown preserve exactly one underlying operation. Receipt admission and
  shutdown support observable late Results; timed/cancelled flush supports only
  eventual health accounting and slot release, without a prior-result accessor.
  Executing wait boundaries resolve cancellation/timeout variants; cancellation
  before task start is tested separately from library completion.
  Admission, flush completion and durability claims are distinguished in examples.

## Required validation (authoritative)

```sh
bash scripts/ci/validate_python_bindings.sh
```

Extend the script to run packaged asyncio tests with debug mode/warnings treated
as failures, deterministic held-writer tests, heartbeat and resource-bound checks,
multiple-loop/worker-thread completion tests, and loop/interpreter shutdown
subprocess tests. Run both owned and attached modes on the B.4a Python/platform
matrix. Use deterministic operation counts and bounded outstanding-work checks
rather than a throughput benchmark as correctness evidence.

## Paths to delete

None.

## Non-closure

No sc-runtime worker pool, subprocess IPC, subinterpreter/free-threaded support,
worker fairness policy, cross-worker global ordering, per-record persistence
receipt, prior-flush-result accessor after timeout/cancellation, or durable-on-disk guarantee. Those require a runtime/transport spec.
Publication is B.7. Existing B.4/B.5 runtime behavior must remain production-ready;
this sprint adds optional waiting without making it necessary for logging.

## Technical reference

[Python coroutines and tasks](https://docs.python.org/3/library/asyncio-task.html)
distinguishes coroutine creation from execution; therefore immediate submit is
separate from the explicitly awaited receipt method.
