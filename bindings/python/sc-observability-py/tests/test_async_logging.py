"""Observer mechanics regressions; actual backend cases extend these below."""
from __future__ import annotations

import asyncio
import gc
import json
import weakref

import pytest

from sc_observability import Err, LogEvent, Logger, Ok, generated
from sc_observability.async_logging import _flush_async, _pools, _submit, _teardown

DONE = json.dumps({"kind": "ok", "value": {"kind": "completed"}})
STARTED = json.dumps({"kind": "ok", "value": None})


class AdmissionBackend:
    def __init__(self, admission: str = "accepted") -> None:
        self.calls = 0
        self.admission = admission

    def log(self, event: LogEvent) -> object:
        self.calls += 1
        value = generated.OutputAdmissionAccepted() if self.admission == "accepted" else generated.OutputAdmissionFiltered()
        return Ok(value)


class Operation:
    def __init__(self, done: bool = False) -> None:
        self.done = done
        self.reads = 0

    def state(self) -> str | None:
        self.reads += 1
        return DONE if self.done else None


class Native:
    def __init__(self, *, done: bool = False) -> None:
        self.calls = 0
        self.operations: list[Operation] = []
        self.done = done

    def observer_key(self) -> int:
        return id(self)

    def start_flush(self, timeout: str) -> tuple[Operation, str]:
        self.calls += 1
        operation = Operation(self.done)
        self.operations.append(operation)
        return operation, STARTED


@pytest.mark.parametrize("admission", ["accepted", "filtered"])
def test_receipt_prepares_before_single_admission_and_wait_never_suspends(admission: str) -> None:
    native = AdmissionBackend(admission)
    event = LogEvent(level="info", target="async.test", action="receipt")
    reference = weakref.ref(event)
    submitted = _submit(native, event)
    assert isinstance(submitted, Ok)
    assert native.calls == 1
    del event
    gc.collect()
    assert reference() is None
    receipt = submitted.value
    assert receipt.state().kind == "resolved"
    assert receipt.state().admission.kind == admission
    for timeout in (0, 2000, 60_000):
        coroutine = receipt.wait(timeout)
        with pytest.raises(StopIteration) as stopped:
            coroutine.send(None)
        assert stopped.value.value == Ok(receipt.state().admission)
    for timeout in (True, -1, 60_001, 1.5, float("nan")):
        result = asyncio.run(receipt.wait(timeout))
        assert isinstance(result, Err) and result.error.kind == "validation"


def test_receipt_allocation_failure_does_not_submit(monkeypatch: pytest.MonkeyPatch) -> None:
    import sc_observability.async_logging as module
    def fail(*args: object) -> object:
        raise MemoryError("injected allocation")
    monkeypatch.setattr(module, "LogReceipt", fail)
    native = AdmissionBackend()
    result = _submit(native, LogEvent(level="info", target="async.test", action="receipt"))
    assert isinstance(result, Err) and result.error.kind == "internal"
    assert native.calls == 0


def test_zero_timeout_inspects_once_without_timer_or_retry() -> None:
    async def run() -> None:
        completed = Native(done=True)
        pending = Native()
        assert isinstance(await _flush_async(completed, 0), Ok)
        result = await _flush_async(pending, 0)
        assert isinstance(result, Err) and result.error.kind == "timeout"
        assert pending.calls == 1 and pending.operations[0].reads == 1
        assert not _pools
    asyncio.run(run(), debug=True)


def test_timeout_cancel_and_success_release_observers_without_resubmission() -> None:
    async def run() -> None:
        native = Native()
        timed = await _flush_async(native, 2)
        assert isinstance(timed, Err) and timed.error.kind == "timeout"
        task = asyncio.create_task(_flush_async(native))
        await asyncio.sleep(0)
        task.cancel()
        cancelled = await task
        assert isinstance(cancelled, Err) and cancelled.error.kind == "cancelled"
        assert native.calls == 2
        assert not _pools
        task = asyncio.create_task(_flush_async(native))
        await asyncio.sleep(0)
        native.operations[-1].done = True
        assert isinstance(await task, Ok)
        assert native.calls == 3 and not _pools
        before_start = asyncio.create_task(_flush_async(native))
        before_start.cancel()
        with pytest.raises(asyncio.CancelledError):
            await before_start
        assert native.calls == 3
    asyncio.run(run(), debug=True)


def test_64_completed_native_calls_still_reserve_until_observed() -> None:
    async def run() -> None:
        native = Native()
        tasks = [asyncio.create_task(_flush_async(native)) for _ in range(64)]
        await asyncio.sleep(0)
        assert len(_pools[native.observer_key()].observers) == 64
        for operation in native.operations:
            operation.done = True
        # Native completion alone does not release Python observation capacity.
        overflow = await _flush_async(native)
        assert isinstance(overflow, Err)
        assert overflow.error.code == generated.SC_OBSERVABILITY_BINDING_WAITERS_FULL
        assert native.calls == 64
        assert all(isinstance(result, Ok) for result in await asyncio.gather(*tasks))
        assert not _pools
    asyncio.run(run(), debug=True)


def test_closed_loop_reservations_are_reclaimed_without_retaining_loop() -> None:
    native = Native()
    loop = asyncio.new_event_loop()
    waits = [_flush_async(native) for _ in range(64)]
    for wait in waits:
        loop.call_soon(wait.send, None)
    loop.run_until_complete(asyncio.sleep(0))
    pool = _pools[native.observer_key()]
    assert len(pool.observers) == 64
    loop.close()
    assert isinstance(asyncio.run(_flush_async(native, 0)), Err)
    assert not pool.observers
    for wait in waits:
        wait.close()
    reference = weakref.ref(loop)
    del waits, wait, loop
    gc.collect()
    assert reference() is None
    _teardown()
