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

## Deliverables (authoritative)

1. Add `submit` to owned Logger and AttachedLogger, returning the receipt below,
   and implement async receipt waiting and flush in
   `bindings/python/sc-observability-py/python/sc_observability/async_logging.py`
   with any required PyO3 support. Keep existing `log(event) -> Result[Admission]` as the
   minimal nonblocking path, without requiring receipt allocation or an event loop.
   A caller may intentionally ignore its result.
2. Implement completion transfer between native operations and asyncio loops.
   Never block the event-loop thread on sink I/O, writer capacity, a native
   mutex, or a thread join. Completion belongs to the logger/backend rather
   than the lifetime of a waiting Python task. Pin the supported async bridge
   mechanism in the package; reuse the host runtime where appropriate instead
   of starting a Rust executor for each Python call.
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

An Ok admission includes core level-filter handling; it is not proof of writing
or durability. Queue-full/closed/validation/backend failures use the corresponding
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

Other waiters can still observe final completion. An ignored receipt never
creates an unawaited-coroutine warning, unhandled Future exception, or logger-owned
collection that grows without bound. Best-effort health accounts for failures
even when no receipt is observed. If accounting fails, preserve the original
Result without raising or attempting to log the accounting failure.

Do not add an unbounded queue/thread pool in front of the bounded Rust queue.
Receipt completion stores only fixed-size status/diagnostic data after admission,
not the submitted payload. Async flush operations are coalesced/bounded using the
established lifecycle owner. A flush result describes the core sink-flush
contract, not fsync durability; event ordering/barrier semantics must match the
core implementation. A flush request cannot report success merely because its
waiter was cancelled or its event loop closed.

Native callbacks must resolve Python waiters on their owning loops through a
thread-safe scheduling bridge. A closed loop leaves the saved native result and
logger health intact; no callback attempts to revive a closed interpreter. Factories,
validation, queries, health and lifecycle operations retain their Result contracts;
no exception-based alternate path is introduced for async use.

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
  bounded by explicit limits, not the number of historical calls.
- AC4: Timeout, task cancellation, multiple waiters, loop closure and host
  shutdown preserve exactly one underlying operation and observable late results.
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
subprocess tests. Run both owned and attached modes on the B.4 Python/platform
matrix. Use deterministic operation counts and bounded outstanding-work checks
rather than a throughput benchmark as correctness evidence.

## Paths to delete

None.

## Non-closure

No sc-runtime worker pool, subprocess IPC, subinterpreter/free-threaded support,
worker fairness policy, cross-worker global ordering, per-record persistence
receipt, or durable-on-disk guarantee. Those require a runtime/transport spec.
Publication is B.7. Existing B.4/B.5 runtime behavior must remain production-ready;
this sprint adds optional waiting without making it necessary for logging.

## Technical reference

[Python coroutines and tasks](https://docs.python.org/3/library/asyncio-task.html)
distinguishes coroutine creation from execution; therefore immediate submit is
separate from the explicitly awaited receipt method.
