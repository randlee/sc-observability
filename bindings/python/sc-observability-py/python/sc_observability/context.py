"""Explicit Result-returning request scopes; importing this module changes no logger."""
from __future__ import annotations

import asyncio
from contextvars import ContextVar, Token
from dataclasses import dataclass, field, replace
import importlib
import json
import threading
from typing import Literal, Union
import weakref

from . import Err, LogEvent, Ok, Result, TraceContext, _at, _decode_control, _event, _internal, generated


@dataclass(frozen=True)
class ContextIdle:
    kind: Literal["idle"] = field(default="idle", init=False)


@dataclass(frozen=True)
class ContextEntered:
    kind: Literal["entered"] = field(default="entered", init=False)


@dataclass(frozen=True)
class ContextClosed:
    kind: Literal["closed"] = field(default="closed", init=False)


ContextOutcome = Union[ContextIdle, ContextEntered, ContextClosed]


@dataclass(frozen=True)
class _Values:
    request_id: str | None = None
    correlation_id: str | None = None
    trace: TraceContext | None = None


@dataclass(frozen=True)
class _Frame:
    scope: ContextScope
    values: _Values


_STACK: ContextVar[tuple[_Frame, ...]] = ContextVar("sc_observability_context", default=())


def _task() -> asyncio.Task[object] | None:
    try:
        return asyncio.current_task()
    except RuntimeError:  # asyncio's no-running-loop signal, not library control flow
        return None


def _scope_failure(message: str) -> Err:
    return Err(generated.OutputFailureValidation(
        at=_at(), code=generated.SC_OBSERVABILITY_PY_CONTEXT_SCOPE_INVALID,
        message=message, field="context",
        remediation=generated.OutputRemediationRecoverable(
            steps=("Enter and close each scope once in LIFO order on its originating thread and task",)
        ),
    ))


def _validate_event(event: LogEvent) -> Result[None]:
    """Reuse core constructors through the non-admitting native validation hook."""
    try:
        encoded = _event(event)
        if isinstance(encoded, Err):
            return encoded
        native = importlib.import_module("sc_observability._native")
        return _decode_control(native._validate_event(json.dumps(encoded.value, separators=(",", ":"))))
    except Exception:  # foreign extension/encoder failure only
        return Err(_internal("Native input validation could not complete"))


class ContextScope:
    """Opaque scope returned inactive by bind_context; no context-manager protocol."""

    def __init__(self, values: _Values) -> None:
        self._values = values
        self._state: Literal["inactive", "active", "closed"] = "inactive"
        self._token: Token[tuple[_Frame, ...]] | None = None
        self._thread: threading.Thread | None = None
        self._task_ref: weakref.ReferenceType[asyncio.Task[object]] | None = None
        self._last: Result[ContextOutcome] = Ok(ContextIdle())
        self._lock = threading.RLock()

    def enter(self) -> Result[ContextOutcome]:
        with self._lock:
            if self._state != "inactive":
                self._last = _scope_failure("Scope can be entered only once while inactive")
                return self._last
            try:
                stack = _STACK.get()
            except Exception:
                self._last = Err(_internal("Context state could not be read"))
                return self._last
            if len(stack) >= 64:
                self._last = _scope_failure("At most 64 context scopes may be active")
                return self._last
            previous = stack[-1].values if stack else _Values()
            values = _Values(
                self._values.request_id if self._values.request_id is not None else previous.request_id,
                self._values.correlation_id if self._values.correlation_id is not None else previous.correlation_id,
                self._values.trace if self._values.trace is not None else previous.trace,
            )
            try:
                thread = threading.current_thread()
                task = _task()
                task_ref = weakref.ref(task) if task is not None else None
                token = _STACK.set((*stack, _Frame(self, values)))
            except Exception:  # foreign ContextVar/thread/task implementation
                self._last = Err(_internal("Context activation could not complete"))
                return self._last
            self._token, self._thread, self._task_ref = token, thread, task_ref
            self._state = "active"
            self._last = Ok(ContextEntered())
            return self._last

    def close(self) -> Result[ContextOutcome]:
        with self._lock:
            if self._state == "closed":
                self._last = Ok(ContextClosed())
                return self._last
            try:
                stack = _STACK.get()
                task = _task()
            except Exception:
                self._last = Err(_internal("Context state could not be read"))
                return self._last
            expected_task = self._task_ref() if self._task_ref is not None else None
            if (self._state != "active" or self._thread is not threading.current_thread()
                    or task is not expected_task or not stack or stack[-1].scope is not self
                    or self._token is None):
                self._last = _scope_failure("Scope must close in LIFO order on its originating thread and task")
                return self._last
            try:
                _STACK.reset(self._token)
            except Exception:  # copied-context token or foreign ContextVar failure
                self._last = Err(_internal("Context restoration could not complete"))
                return self._last
            self._state = "closed"
            self._token = self._thread = self._task_ref = None
            self._last = Ok(ContextClosed())
            return self._last

    def last_result(self) -> Result[ContextOutcome]:
        with self._lock:
            return self._last


def bind_context(*, request_id: str | None = None, correlation_id: str | None = None,
                 trace: TraceContext | None = None) -> Result[ContextScope]:
    """Validate without emission; return an inactive scope that must be entered."""
    try:
        # Snapshot foreign subclasses before validation; retain only owned values.
        if trace is not None and not isinstance(trace, TraceContext):
            from . import _failure
            return Err(_failure("trace", "expected TraceContext"))
        owned_trace = None if trace is None else TraceContext(trace.trace_id, trace.span_id, trace.parent_span_id)
        checked = _validate_event(LogEvent(level="info", target="python.context", action="context.validate",
                                         request_id=request_id, correlation_id=correlation_id, trace=owned_trace))
        if isinstance(checked, Err):
            return checked
        return Ok(ContextScope(_Values(request_id, correlation_id, owned_trace)))
    except Exception:
        return Err(_internal("Context input snapshot could not complete"))



def _inherit_event(event: LogEvent) -> Result[LogEvent]:
    """Called by both facade log methods; explicit non-None fields win."""
    if not isinstance(event, LogEvent):
        from . import _failure
        return Err(_failure("event", "expected LogEvent"))
    stack = _STACK.get()
    if not stack:
        return Ok(event)
    values = stack[-1].values
    return Ok(replace(event,
                      request_id=event.request_id if event.request_id is not None else values.request_id,
                      correlation_id=event.correlation_id if event.correlation_id is not None else values.correlation_id,
                      trace=event.trace if event.trace is not None else values.trace))


__all__ = ["ContextIdle", "ContextEntered", "ContextClosed", "ContextOutcome", "ContextScope", "bind_context"]
