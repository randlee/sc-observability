"""Opt-in standard logging adapter borrowing an owned or host-attached logger."""
from __future__ import annotations

import copy
from dataclasses import dataclass, field
from enum import Enum
import logging
import threading
from types import MappingProxyType
from typing import Literal, Mapping, Union

from . import AttachedLogger, Err, Level, LogEvent, Logger, Ok, Result, _at, _failure, _internal, generated
from .context import _validate_event


class HandlerDropCause(str, Enum):
    VALIDATION = "validation"
    QUEUE_FULL = "queue_full"
    BELOW_BASELINE = "below_baseline"
    UNSUPPORTED_LEVEL = "unsupported_level"
    PERMISSION_DENIED = "permission_denied"
    CLOSED = "closed"
    UNAVAILABLE = "unavailable"
    IO = "io"
    TIMEOUT = "timeout"
    CANCELLED = "cancelled"
    UNSUPPORTED_VERSION = "unsupported_version"
    INTERNAL = "internal"
    UNKNOWN_REMOTE = "unknown_remote"
    REENTRANT = "reentrant"


@dataclass(frozen=True)
class HandlerIdle:
    kind: Literal["idle"] = field(default="idle", init=False)


@dataclass(frozen=True)
class HandlerEmitted:
    admission: generated.Admission
    kind: Literal["emitted"] = field(default="emitted", init=False)


@dataclass(frozen=True)
class HandlerFlushed:
    kind: Literal["flushed"] = field(default="flushed", init=False)


@dataclass(frozen=True)
class HandlerClosed:
    kind: Literal["closed"] = field(default="closed", init=False)


HandlerOutcome = Union[HandlerIdle, HandlerEmitted, HandlerFlushed, HandlerClosed]


@dataclass(frozen=True)
class HandlerHealth:
    dropped_by_cause: Mapping[HandlerDropCause, int]
    last_result: Result[HandlerOutcome]


@dataclass
class _Emission:
    reentered: bool = False


_CAUSES = {cause.value: cause for cause in HandlerDropCause if cause != HandlerDropCause.REENTRANT}
_MAX_COUNT = (1 << 64) - 1


def _closed() -> Err:
    return Err(generated.OutputFailureClosed(
        at=_at(), code=generated.SC_OBSERVABILITY_BINDING_CLOSED,
        message="Python logging handler is closed",
        remediation=generated.OutputRemediationRecoverable(
            steps=("Stop submitting through the closed backend and inspect its retained health",)
        ),
    ))


def _reentrant() -> Err:
    return Err(generated.OutputFailureInternal(
        at=_at(), code=generated.SC_OBSERVABILITY_PY_HANDLER_REENTRANT,
        message="Recursive Python logging handler invocation",
        remediation=generated.OutputRemediationRecoverable(
            steps=("Remove logging calls from handler formatting and error callbacks",)
        ),
    ))


class ObservabilityHandler(logging.Handler):
    """Create with create_handler. emit/flush/close contain foreign failures."""

    def __init__(self, logger: Logger | AttachedLogger, level: int, extra_fields: tuple[str, ...]) -> None:
        self._logger = logger
        self._extra_fields = extra_fields
        self._state_lock = threading.RLock()
        self._active = threading.local()
        self._counts = {cause: 0 for cause in HandlerDropCause}
        self._last: Result[HandlerOutcome] = Ok(HandlerIdle())
        self._handler_closed = False
        super().__init__(level)

    def _record(self, result: Result[HandlerOutcome], *, event: bool = False,
                cause: HandlerDropCause | None = None, preserve_reentrant: bool = False) -> None:
        with self._state_lock:
            if isinstance(result, Err) and event:
                selected = cause if cause is not None else _CAUSES.get(result.error.kind, HandlerDropCause.INTERNAL)
                try:
                    self._counts[selected] = min(_MAX_COUNT, self._counts[selected] + 1)
                except Exception:  # accounting must never replace the original failure
                    pass
            if not preserve_reentrant or isinstance(result, Err):
                self._last = result

    def emit(self, record: logging.LogRecord) -> None:
        active = getattr(self._active, "emission", None)
        if active is not None:
            active.reentered = True
            self._record(_reentrant(), event=True, cause=HandlerDropCause.REENTRANT)
            return
        emission = _Emission()
        self._active.emission = emission
        try:
            with self._state_lock:
                closed = self._handler_closed
            if closed:
                self._record(_closed(), event=True)
                return
            if type(record.levelno) is not int:
                self._record(Err(_failure("level", "LogRecord level must be an integer")), event=True)
                return
            level: Level = ("trace" if record.levelno < logging.DEBUG else
                     "debug" if record.levelno < logging.INFO else
                     "info" if record.levelno < logging.WARNING else
                     "warn" if record.levelno < logging.ERROR else "error")
            message = record.getMessage()  # exactly once, inside recursion containment
            formatter = self.formatter
            if formatter is not None:
                formatted = copy.copy(record)
                formatted.message = message
                if formatter.usesTime():
                    formatted.asctime = formatter.formatTime(formatted, formatter.datefmt)
                message = formatter.formatMessage(formatted)
            fields = {name: getattr(record, name) for name in self._extra_fields if hasattr(record, name)}
            if record.exc_info:
                fields["exception"] = (formatter or logging.Formatter()).formatException(record.exc_info)
            elif record.exc_text:
                fields["exception"] = record.exc_text
            if record.stack_info:
                fields["stack"] = (formatter or logging.Formatter()).formatStack(record.stack_info)
            result = self._logger.log(LogEvent(level=level, target=record.name, action="python.log",
                                              message=message, fields=fields))
            if isinstance(result, Err):
                self._record(result, event=True)
            elif isinstance(result, Ok) and getattr(result.value, "kind", None) in ("accepted", "filtered"):
                self._record(Ok(HandlerEmitted(result.value)), preserve_reentrant=emission.reentered)
            else:
                self._record(Err(_internal("Backend returned an invalid admission result")), event=True)
        except Exception:  # foreign formatter, LogRecord, selected extra or custom backend
            self._record(Err(_internal("Python logging formatting or submission failed")), event=True)
        finally:
            self._active.emission = None

    def flush(self) -> None:
        try:
            with self._state_lock:
                closed = self._handler_closed
            if closed:
                self._record(_closed())
                return
            result = self._logger.flush(timeout_ms=2000)
            if isinstance(result, Err):
                self._record(result)
            elif isinstance(result, Ok) and getattr(result.value, "kind", None) == "completed":
                self._record(Ok(HandlerFlushed()))
            else:
                self._record(Err(_internal("Backend returned an invalid flush result")))
        except Exception:  # foreign backend; no event drop is counted for flush
            self._record(Err(_internal("Python logging flush failed")))

    def close(self) -> None:
        try:
            with self._state_lock:
                self._handler_closed = True
            super().close()
            self._record(Ok(HandlerClosed()))
        except Exception:  # logging framework cleanup; borrowed logger is never closed
            self._record(Err(_internal("Python logging handler close failed")))

    def health(self) -> Result[HandlerHealth]:
        with self._state_lock:
            return Ok(HandlerHealth(MappingProxyType(dict(self._counts)), self._last))

    def last_result(self) -> Result[HandlerOutcome]:
        with self._state_lock:
            return self._last


def create_handler(logger: Logger | AttachedLogger, *, level: int = logging.NOTSET,
                   extra_fields: tuple[str, ...] = ()) -> Result[ObservabilityHandler]:
    """Construct explicitly; never attach to or change the Python root logger."""
    if type(level) is not int:
        return Err(_failure("level", "expected a Python logging integer level"))
    if not isinstance(extra_fields, tuple) or any(not isinstance(name, str) or not name for name in extra_fields):
        return Err(_failure("extra_fields", "expected a tuple of nonempty field names"))
    try:
        if not callable(getattr(logger, "log", None)) or not callable(getattr(logger, "flush", None)):
            return Err(_failure("logger", "expected a readable logging backend"))
        checked = _validate_event(LogEvent(level="info", target="python.handler", action="handler.validate",
                                         fields={name: None for name in extra_fields}))
        if isinstance(checked, Err):
            return checked
        return Ok(ObservabilityHandler(logger, level, extra_fields))
    except Exception:  # foreign backend or standard logging initialization
        return Err(_internal("Python logging handler creation failed"))


__all__ = ["HandlerDropCause", "HandlerIdle", "HandlerEmitted", "HandlerFlushed", "HandlerClosed",
           "HandlerOutcome", "HandlerHealth", "ObservabilityHandler", "create_handler"]
