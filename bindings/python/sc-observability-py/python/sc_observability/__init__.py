"""Result-returning owned Python logging API over the shared native backend.

The public facade accepts ergonomic frozen input values.  It encodes them once
to the canonical schema and lets the Rust DTO/runtime crates own validation of
native values, provenance stamping, admission, and lifecycle conversion.
"""
from __future__ import annotations

from dataclasses import dataclass, field
from datetime import datetime, timezone
import importlib
import json
import math
from pathlib import Path
from typing import Any, Generic, Literal, Mapping, Protocol, TypeAlias, TypeVar, cast, TYPE_CHECKING

from . import generated

if TYPE_CHECKING:
    from .async_logging import LogReceipt

T = TypeVar("T")
Level: TypeAlias = Literal["trace", "debug", "info", "warn", "error"]
LevelFilter: TypeAlias = Literal["off", "error", "warn", "info", "debug", "trace"]
LevelChangeSource: TypeAlias = Literal["application", "user_request", "diagnostic_session"]


@dataclass(frozen=True)
class Ok(Generic[T]):
    value: T
    kind: Literal["ok"] = field(default="ok", init=False)


@dataclass(frozen=True)
class Err:
    error: generated.Failure
    kind: Literal["error"] = field(default="error", init=False)


Result: TypeAlias = Ok[T] | Err


class _NativeReadable(Protocol):
    def log(self, payload: str) -> str: ...
    def query(self, payload: str) -> str: ...
    def health(self) -> str: ...
    def flush(self, timeout: str) -> str: ...


class _NativeOwned(_NativeReadable, Protocol):
    def shutdown(self, timeout: str) -> str: ...
    def wait_stopped(self, timeout: str) -> str: ...
    def elevate_level(self, level: str, source: str) -> str: ...
    def reset_level(self, source: str) -> str: ...


@dataclass(frozen=True)
class LoggerConfig:
    service: str
    log_root: str
    level: LevelFilter = "info"
    enable_file_sink: bool = True
    enable_console_sink: bool = False


@dataclass(frozen=True)
class TraceContext:
    trace_id: str
    span_id: str
    parent_span_id: str | None = None


@dataclass(frozen=True)
class FieldMatch:
    field: str
    value: object


@dataclass(frozen=True)
class LogEvent:
    level: Level
    target: str
    action: str
    message: str | None = None
    trace: TraceContext | None = None
    request_id: str | None = None
    correlation_id: str | None = None
    outcome: str | None = None
    fields: Mapping[str, object] = field(default_factory=dict)


@dataclass(frozen=True)
class LogQuery:
    service: str | None = None
    levels: tuple[Level, ...] = ()
    target: str | None = None
    action: str | None = None
    request_id: str | None = None
    correlation_id: str | None = None
    since: str | None = None
    until: str | None = None
    field_matches: tuple[FieldMatch, ...] = ()
    limit: int = 100
    order: Literal["oldest_first", "newest_first"] = "oldest_first"


def _at() -> str:
    return datetime.now(timezone.utc).isoformat().replace("+00:00", "Z")


def _failure(field_name: str, message: str) -> generated.Failure:
    return generated.OutputFailureValidation(
        at=_at(),
        code=generated.SC_OBSERVABILITY_BINDING_INVALID_INPUT,
        field=field_name,
        message=message,
        remediation=generated.OutputRemediationRecoverable(
            steps=("Correct the named input field and submit a new request",)
        ),
    )


def _internal(message: str) -> generated.Failure:
    return generated.OutputFailureInternal(
        at=_at(),
        code=generated.SC_OBSERVABILITY_BINDING_INTERNAL,
        message=message,
        remediation=generated.OutputRemediationRecoverable(
            steps=("Inspect the retained status and restore the affected host or client",)
        ),
    )


def _unavailable(code: str, message: str, step: str) -> generated.Failure:
    return generated.OutputFailureUnavailable(
        at=_at(),
        code=code,
        message=message,
        remediation=generated.OutputRemediationRecoverable(steps=(step,)),
    )


def _normalised_key(key: str) -> str:
    return "".join(
        character if character.isascii() and (character.isalnum() or character in "_.-") else "_"
        for character in key.replace("::", ".")
    )


def _value(value: object, path: str, seen: set[int], depth: int = 0) -> Result[dict[str, object]]:
    if depth >= 32:
        return Err(_failure(path, "maximum container depth is 32"))
    if value is None:
        return Ok({"kind": "null"})
    if type(value) is bool:
        return Ok({"kind": "boolean", "value": value})
    if isinstance(value, str):
        return Ok({"kind": "string", "value": value})
    if type(value) is int:
        if not -(2**63) <= value < 2**64:
            return Err(_failure(path, "integer is outside the shared wire range"))
        return Ok({"kind": "integer", "value": str(value)})
    if type(value) is float:
        if not math.isfinite(value):
            return Err(_failure(path, "float must be finite"))
        return Ok({"kind": "float", "value": value})
    if isinstance(value, (tuple, list)):
        identity = id(value)
        if identity in seen:
            return Err(_failure(path, "cyclic container"))
        seen.add(identity)
        try:
            array_values: list[object] = []
            for index, item in enumerate(value):
                child = _value(item, f"{path}[{index}]", seen, depth + 1)
                if isinstance(child, Err):
                    return child
                array_values.append(child.value)
            return Ok({"kind": "array", "value": array_values})
        finally:
            seen.remove(identity)
    if isinstance(value, Mapping):
        identity = id(value)
        if identity in seen:
            return Err(_failure(path, "cyclic container"))
        seen.add(identity)
        try:
            object_values: dict[str, object] = {}
            try:
                entries = tuple(value.items())
            except Exception as error:  # foreign Mapping implementation
                return Err(_internal(f"could not inspect mapping input: {error}"))
            for key, item in entries:
                if not isinstance(key, str):
                    return Err(_failure(path, "object keys must be strings"))
                if _normalised_key(key).startswith("sc_observability.binding."):
                    return Err(_failure(f"{path}.{key}", "reserved binding provenance field"))
                child = _value(item, f"{path}.{key}", seen, depth + 1)
                if isinstance(child, Err):
                    return child
                object_values[key] = child.value
            return Ok({"kind": "object", "value": object_values})
        finally:
            seen.remove(identity)
    return Err(_failure(path, f"unsupported value type {type(value).__name__}"))


def _event(event: object) -> Result[dict[str, object]]:
    if not isinstance(event, LogEvent):
        return Err(_failure("event", "expected LogEvent"))
    if event.level not in ("trace", "debug", "info", "warn", "error"):
        return Err(_failure("level", "unsupported level"))
    trace = None
    if event.trace is not None:
        if not isinstance(event.trace, TraceContext):
            return Err(_failure("trace", "expected TraceContext"))
        trace = {
            "trace_id": event.trace.trace_id,
            "span_id": event.trace.span_id,
            "parent_span_id": event.trace.parent_span_id,
        }
    fields: dict[str, object] = {}
    if not isinstance(event.fields, Mapping):
        return Err(_failure("fields", "expected a string-keyed mapping"))
    try:
        entries = tuple(event.fields.items())
    except Exception as error:  # foreign Mapping implementation
        return Err(_internal(f"could not inspect event fields: {error}"))
    for key, value in entries:
        if not isinstance(key, str):
            return Err(_failure("fields", "object keys must be strings"))
        if _normalised_key(key).startswith("sc_observability.binding."):
            return Err(_failure(f"fields.{key}", "reserved binding provenance field"))
        encoded = _value(value, f"fields.{key}", set())
        if isinstance(encoded, Err):
            return encoded
        fields[key] = encoded.value
    return Ok({
        "schema_version": 1,
        "level": event.level,
        "target": event.target,
        "action": event.action,
        "message": event.message,
        "trace": trace,
        "request_id": event.request_id,
        "correlation_id": event.correlation_id,
        "outcome": event.outcome,
        "fields": fields,
    })


def _query(query: object) -> Result[dict[str, object]]:
    if not isinstance(query, LogQuery):
        return Err(_failure("query", "expected LogQuery"))
    if type(query.limit) is not int or not 1 <= query.limit <= 1000:
        return Err(_failure("limit", "expected an integer in 1..1000"))
    if query.order not in ("oldest_first", "newest_first"):
        return Err(_failure("order", "unsupported query order"))
    matches: list[dict[str, object]] = []
    for index, match in enumerate(query.field_matches):
        if not isinstance(match, FieldMatch):
            return Err(_failure(f"field_matches[{index}]", "expected FieldMatch"))
        encoded = _value(match.value, f"field_matches[{index}].value", set())
        if isinstance(encoded, Err):
            return encoded
        matches.append({"field": match.field, "value": encoded.value})
    return Ok({
        "schema_version": 1,
        "service": query.service,
        "levels": list(query.levels),
        "target": query.target,
        "action": query.action,
        "request_id": query.request_id,
        "correlation_id": query.correlation_id,
        "since": query.since,
        "until": query.until,
        "field_matches": matches,
        "limit": query.limit,
        "order": query.order,
    })


def _timeout(timeout_ms: object) -> Result[str]:
    if type(timeout_ms) is not int or not 0 <= timeout_ms <= 60_000:
        return Err(_failure("timeout_ms", "expected integer milliseconds in 0..60000"))
    return Ok(json.dumps(timeout_ms))


def _decode(name: str, payload: str) -> Result[object]:
    try:
        wire = json.loads(payload)
        decoded = generated.from_wire(name, wire)
    except (TypeError, ValueError, json.JSONDecodeError) as error:
        return Err(_internal(f"invalid native result: {error}"))
    if getattr(decoded, "kind", None) == "error":
        return Err(cast(generated.Failure, getattr(decoded, "error")))
    return Ok(getattr(decoded, "value"))


def _decode_control(payload: str) -> Result[None]:
    """Decode the factory's `Result[None]` without inventing an untyped schema entrypoint."""
    try:
        wire = json.loads(payload)
        if wire.get("kind") == "ok" and wire.get("value") is None:
            return Ok(None)
        if wire.get("kind") == "error":
            return Err(cast(generated.Failure, generated.from_wire("OutputFailure", wire["error"])))
    except (AttributeError, TypeError, ValueError, json.JSONDecodeError) as error:
        return Err(_internal(f"invalid native control result: {error}"))
    return Err(_internal("invalid native control result shape"))


def _typed(value: Result[object]) -> Result[T]:
    if isinstance(value, Err):
        return value
    return Ok(cast(T, value.value))


class Logger:
    """An independently owned logger; it is never process-global."""

    def __init__(self, native: _NativeOwned) -> None:
        self._native = native

    def log(self, event: LogEvent) -> Result[generated.Admission]:
        from .context import _inherit_event
        inherited = _inherit_event(event)
        if isinstance(inherited, Err):
            return inherited
        encoded = _event(inherited.value)
        if isinstance(encoded, Err):
            return encoded
        return _typed(_decode("OutputResultDtoAdmissionDto", self._native.log(json.dumps(encoded.value, separators=(",", ":")))))

    def submit(self, event: LogEvent) -> Result[LogReceipt]:
        """Admit once now, optionally inspect its resolved receipt later."""
        from .async_logging import _submit
        return _submit(self, event)

    async def flush_async(self, timeout_ms: int = 2000) -> Result[generated.Completion]:
        """Observe one native flush without blocking the calling event loop."""
        from .async_logging import _flush_async
        return await _flush_async(self._native, timeout_ms)

    def query(self, query: LogQuery) -> Result[generated.LogSnapshot]:
        encoded = _query(query)
        if isinstance(encoded, Err):
            return encoded
        return _typed(_decode("OutputResultDtoLogSnapshotDto", self._native.query(json.dumps(encoded.value, separators=(",", ":")))))

    def health(self) -> Result[generated.LogHealth]:
        return _typed(_decode("OutputResultDtoLogHealthDto", self._native.health()))

    def flush(self, timeout_ms: int = 2000) -> Result[generated.Completion]:
        timeout = _timeout(timeout_ms)
        if isinstance(timeout, Err):
            return timeout
        return _typed(_decode("OutputResultDtoCompletionDto", self._native.flush(timeout.value)))

    def shutdown(self, timeout_ms: int = 2000) -> Result[generated.LogHealth]:
        timeout = _timeout(timeout_ms)
        if isinstance(timeout, Err):
            return timeout
        return _typed(_decode("OutputResultDtoLogHealthDto", self._native.shutdown(timeout.value)))

    def wait_stopped(self, timeout_ms: int = 2000) -> Result[generated.LogHealth]:
        timeout = _timeout(timeout_ms)
        if isinstance(timeout, Err):
            return timeout
        return _typed(_decode("OutputResultDtoLogHealthDto", self._native.wait_stopped(timeout.value)))

    def elevate_level(
        self, level: LevelFilter, source: LevelChangeSource = "application"
    ) -> Result[generated.LevelChange]:
        return _typed(_decode("OutputResultDtoLevelChangeDto", self._native.elevate_level(level, source)))

    def reset_level(
        self, source: LevelChangeSource = "application"
    ) -> Result[generated.LevelChange]:
        return _typed(_decode("OutputResultDtoLevelChangeDto", self._native.reset_level(source)))


class AttachedLogger:
    """A non-owning view of a Rust host logger (implemented by the host bridge)."""

    def __init__(self, native: _NativeReadable) -> None:
        self._native = native

    def log(self, event: LogEvent) -> Result[generated.Admission]:
        from .context import _inherit_event
        inherited = _inherit_event(event)
        if isinstance(inherited, Err):
            return inherited
        encoded = _event(inherited.value)
        if isinstance(encoded, Err):
            return encoded
        return _typed(_decode("OutputResultDtoAdmissionDto", self._native.log(json.dumps(encoded.value, separators=(",", ":")))))

    def submit(self, event: LogEvent) -> Result[LogReceipt]:
        """Admit once now, optionally inspect its resolved receipt later."""
        from .async_logging import _submit
        return _submit(self, event)

    async def flush_async(self, timeout_ms: int = 2000) -> Result[generated.Completion]:
        """Observe one native flush without blocking the calling event loop."""
        from .async_logging import _flush_async
        return await _flush_async(self._native, timeout_ms)

    def query(self, query: LogQuery) -> Result[generated.LogSnapshot]:
        encoded = _query(query)
        if isinstance(encoded, Err):
            return encoded
        return _typed(_decode("OutputResultDtoLogSnapshotDto", self._native.query(json.dumps(encoded.value, separators=(",", ":")))))

    def health(self) -> Result[generated.LogHealth]:
        return _typed(_decode("OutputResultDtoLogHealthDto", self._native.health()))

    def flush(self, timeout_ms: int = 2000) -> Result[generated.Completion]:
        timeout = _timeout(timeout_ms)
        if isinstance(timeout, Err):
            return timeout
        return _typed(_decode("OutputResultDtoCompletionDto", self._native.flush(timeout.value)))


def create_logger(config: LoggerConfig) -> Result[Logger]:
    """Create one independent owned logger without installing global logging."""
    if not isinstance(config, LoggerConfig):
        return Err(_failure("config", "expected LoggerConfig"))
    if not config.service:
        return Err(_failure("service", "service must be nonempty"))
    if not config.log_root or "\x00" in config.log_root:
        return Err(_failure("log_root", "log_root must be nonempty and NUL-free"))
    try:
        native_module = cast(Any, importlib.import_module("sc_observability._native"))
        native, result = native_module.create_owned(
            json.dumps(
                {
                    "service": config.service,
                    "log_root": str(Path(config.log_root)),
                    "level": config.level,
                    "enable_file_sink": config.enable_file_sink,
                    "enable_console_sink": config.enable_console_sink,
                },
                separators=(",", ":"),
            )
        )
    except Exception as error:  # foreign extension import/runtime failure only
        return Err(_internal(f"native extension unavailable: {error}"))
    decoded = _decode_control(result)
    if isinstance(decoded, Err):
        return decoded
    if native is None:
        return Err(_internal("native factory reported success without a logger"))
    return Ok(Logger(cast(_NativeOwned, native)))


def get_host_logger() -> Result[AttachedLogger]:
    """Return a host-attached logger after Rust embedding installs one."""
    try:
        native_module = cast(Any, importlib.import_module("sc_observability._native"))
        native, result = native_module.get_installed_host_logger()
    except Exception as error:  # foreign extension import/runtime failure only
        return Err(_internal(f"native extension unavailable: {error}"))
    decoded = _decode_control(result)
    if isinstance(decoded, Err):
        return decoded
    if native is None:
        return Err(_internal("native host factory reported success without a logger"))
    return Ok(AttachedLogger(cast(_NativeReadable, native)))


__all__ = [
    "AttachedLogger", "Err", "FieldMatch", "LogEvent", "Logger", "LoggerConfig",
    "LogQuery", "Ok", "Result", "TraceContext", "create_logger", "get_host_logger",
]
