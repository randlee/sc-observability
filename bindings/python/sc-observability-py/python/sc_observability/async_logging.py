"""Immediate admission receipts and bounded, loop-local flush observation.

Native work owns its completion. These observers never cancel or resubmit it,
and native threads never retain Python objects or call into an interpreter.
"""
from __future__ import annotations

import asyncio
import atexit
from dataclasses import dataclass, field
from typing import Literal, Mapping, Protocol, TypeAlias, cast
import weakref

from . import Err, LogEvent, Ok, Result, _at, _decode, _decode_control, _internal, _timeout, _typed, generated


@dataclass(frozen=True)
class Resolved:
    admission: generated.Admission
    kind: Literal["resolved"] = field(default="resolved", init=False)


ReceiptState: TypeAlias = Resolved


@dataclass(frozen=True)
class LogReceipt:
    """Caller-owned saved admission; acceptance is not sink durability."""

    _state: Resolved
    _result: Ok[generated.Admission]

    def state(self) -> ReceiptState:
        return self._state

    async def wait(self, timeout_ms: int = 2000) -> Result[generated.Admission]:
        timeout = _timeout(timeout_ms)
        if isinstance(timeout, Err):
            return timeout
        # Deliberately no await: zero/repeated waits inspect the same result.
        return self._result


class _Logger(Protocol):
    def log(self, event: LogEvent) -> Result[generated.Admission]: ...


class _Operation(Protocol):
    def state(self) -> str | None: ...


class _Native(Protocol):
    def observer_key(self) -> object: ...
    def start_flush(self, timeout: str) -> tuple[_Operation | None, str]: ...


def _submit(logger: _Logger, event: LogEvent) -> Result[LogReceipt]:
    try:
        # All receipt storage is allocated before calling admission. The private
        # provisional value cannot escape; existing slots are filled afterward.
        provisional = generated.OutputAdmissionFiltered()
        state = Resolved(provisional)
        saved: Ok[generated.Admission] = Ok(provisional)
        receipt = LogReceipt(state, saved)
        result = Ok(receipt)
    except Exception:
        return Err(_internal("could not allocate admission receipt"))
    try:
        admitted = logger.log(event)
    except Exception:
        return Err(_internal("could not submit event through the logging boundary"))
    if isinstance(admitted, Err):
        return admitted
    object.__setattr__(state, "admission", admitted.value)
    object.__setattr__(saved, "value", admitted.value)
    return result


def _boundary(kind: Literal["timeout", "cancelled", "queue_full"]) -> Err:
    code = {
        "timeout": generated.SC_OBSERVABILITY_BINDING_TIMEOUT,
        "cancelled": generated.SC_OBSERVABILITY_BINDING_CANCELLED,
        "queue_full": generated.SC_OBSERVABILITY_BINDING_WAITERS_FULL,
    }[kind]
    registry = cast(tuple[Mapping[str, str], ...], getattr(generated, "ERROR_REGISTRY"))
    entry = next(item for item in registry if item["code"] == code)
    remediation = generated.OutputRemediationRecoverable(steps=(entry["remediation"],))
    if kind == "queue_full":
        return Err(generated.OutputFailureQueueFull(
            at=_at(), code=code, message="All 64 Python flush observers are occupied", remediation=remediation,
        ))
    cls = generated.OutputFailureTimeout if kind == "timeout" else generated.OutputFailureCancelled
    return Err(cls(at=_at(), code=code, operation="flush", message=f"Flush observation {kind}", remediation=remediation))


class _Pool:
    def __init__(self) -> None:
        self.observers: dict[int, weakref.ReferenceType[_Observer]] = {}

    def reclaim(self) -> None:
        for key, reference in tuple(self.observers.items()):
            observer = reference()
            if observer is None:
                self.observers.pop(key, None)
            else:
                loop = observer.loop()
                if loop is None or loop.is_closed():
                    observer.release()

    def reserve(self, observer: _Observer) -> bool:
        self.reclaim()
        if len(self.observers) >= 64:
            return False
        self.observers[id(observer)] = weakref.ref(observer)
        return True


# Weak opaque identity keys do not retain backend wrappers or event loops. CPython's supported
# GIL builds serialize these bounded, non-awaiting bookkeeping sections.
_pools: weakref.WeakKeyDictionary[object, _Pool] = weakref.WeakKeyDictionary()


class _Observer:
    def __init__(self, pool: _Pool, loop: asyncio.AbstractEventLoop, deadline: float) -> None:
        self.pool = pool
        self.loop = weakref.ref(loop)
        self.deadline = deadline
        self.future: asyncio.Future[Result[generated.Completion]] = loop.create_future()
        self.operation: _Operation | None = None
        self.timer: asyncio.TimerHandle | None = None
        self.released = False

    def release(self) -> None:
        self.released = True
        self.pool.observers.pop(id(self), None)
        if self.timer is not None:
            self.timer.cancel()
            self.timer = None
        self.operation = None

    def finish(self, result: Result[generated.Completion]) -> None:
        self.release()
        loop = self.loop()
        if loop is not None and not loop.is_closed() and not self.future.done():
            self.future.set_result(result)

    def poll(self, initial: bool = False) -> None:
        self.timer = None  # The previously scheduled callback has been consumed.
        if self.released:
            return
        loop = self.loop()
        if loop is None or loop.is_closed():
            self.release()
            return
        try:
            # Initial zero-timeout observation still inspects saved completion.
            if not initial and loop.time() >= self.deadline:
                self.finish(_boundary("timeout"))
                return
            if self.operation is None:
                self.finish(Err(_internal("flush operation was not provided")))
                return
            payload = self.operation.state()
            if payload is not None:
                self.finish(_typed(_decode("OutputResultDtoCompletionDto", payload)))
            elif loop.time() >= self.deadline:
                self.finish(_boundary("timeout"))
            else:
                self.timer = loop.call_later(0.001, self.poll)
        except Exception:
            self.finish(Err(_internal("could not observe native flush completion")))


async def _flush_async(native: object, timeout_ms: int = 2000) -> Result[generated.Completion]:
    timeout = _timeout(timeout_ms)
    if isinstance(timeout, Err):
        return timeout
    observer: _Observer | None = None
    try:
        loop = asyncio.get_running_loop()
        deadline = loop.time() + timeout_ms / 1000
        backend = cast(_Native, native)
        key = backend.observer_key()
        pool = _pools.get(key)
        if pool is None:
            pool = _Pool()
            _pools[key] = pool
        observer = _Observer(pool, loop, deadline)
        if not pool.reserve(observer):
            return _boundary("queue_full")
        operation, payload = backend.start_flush(timeout.value)
        started = _decode_control(payload)
        if isinstance(started, Err):
            return started
        observer.operation = operation
        observer.poll(initial=True)
        # Shield prevents task cancellation from cancelling the result Future.
        # Only set_result resolves our Future, including cancellation outcomes.
        return await asyncio.shield(observer.future)
    except asyncio.CancelledError:
        result = _boundary("cancelled")
        if observer is not None:
            observer.finish(result)
        return result
    except Exception:
        return Err(_internal("could not start or observe asynchronous flush"))
    finally:
        if observer is not None:
            observer.release()


def _teardown() -> None:
    for pool in tuple(_pools.values()):
        for reference in tuple(pool.observers.values()):
            observer = reference()
            if observer is not None:
                observer.release()
    _pools.clear()


atexit.register(_teardown)

__all__ = ["LogReceipt", "ReceiptState", "Resolved"]
