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
from typing import Any, Callable, Generic, Literal, Mapping, Protocol, TypeAlias, TypeVar, cast, TYPE_CHECKING

from . import generated

if TYPE_CHECKING:
    from .async_logging import LogReceipt

T = TypeVar("T")
Level: TypeAlias = Literal["trace", "debug", "info", "warn", "error"]
LevelFilter: TypeAlias = Literal["off", "error", "warn", "info", "debug", "trace"]
LevelChangeSource: TypeAlias = Literal["application", "user_request", "diagnostic_session"]

_LEVEL_FILTERS = frozenset(("off", "error", "warn", "info", "debug", "trace"))
_LEVEL_CHANGE_SOURCES = frozenset(("application", "user_request", "diagnostic_session"))


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


def _foreign_message(error: BaseException) -> str:
    """Describe a foreign failure without letting a hostile ``__str__`` escape."""
    try:
        return str(error)
    except BaseException:
        return f"unprintable {type(error).__name__}"


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
    if type(value) in (tuple, list):
        array = cast(tuple[object, ...] | list[object], value)
        identity = id(value)
        if identity in seen:
            return Err(_failure(path, "cyclic container"))
        seen.add(identity)
        try:
            array_values: list[object] = []
            for index, item in enumerate(array):
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
            except BaseException as error:  # foreign Mapping implementation
                return Err(_internal(f"could not inspect mapping input: {_foreign_message(error)}"))
            for entry in entries:
                if type(entry) not in (tuple, list) or len(entry) != 2:
                    return Err(_failure(path, "mapping entries must be key-value pairs"))
                key, item = cast(tuple[object, object] | list[object], entry)
                if type(key) is not str:
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
    try:
        return _event_checked(event)
    except BaseException as error:  # foreign dataclass subclass/accessor
        return Err(_internal(f"could not inspect event input: {_foreign_message(error)}"))


def _event_checked(event: object) -> Result[dict[str, object]]:
    if not isinstance(event, LogEvent):
        return Err(_failure("event", "expected LogEvent"))
    if type(event.level) is not str or event.level not in ("trace", "debug", "info", "warn", "error"):
        return Err(_failure("level", "unsupported level"))
    if type(event.target) is not str or not event.target:
        return Err(_failure("target", "expected a nonempty string"))
    if type(event.action) is not str or not event.action:
        return Err(_failure("action", "expected a nonempty string"))
    for field_name, value in (
        ("message", event.message),
        ("request_id", event.request_id),
        ("correlation_id", event.correlation_id),
        ("outcome", event.outcome),
    ):
        if value is not None and type(value) is not str:
            return Err(_failure(field_name, "expected a string or None"))
    trace = None
    if event.trace is not None:
        if not isinstance(event.trace, TraceContext):
            return Err(_failure("trace", "expected TraceContext"))
        if type(event.trace.trace_id) is not str or type(event.trace.span_id) is not str:
            return Err(_failure("trace", "trace identifiers must be strings"))
        if event.trace.parent_span_id is not None and type(event.trace.parent_span_id) is not str:
            return Err(_failure("trace.parent_span_id", "expected a string or None"))
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
    except BaseException as error:  # foreign Mapping implementation
        return Err(_internal(f"could not inspect event fields: {_foreign_message(error)}"))
    for entry in entries:
        if type(entry) not in (tuple, list) or len(entry) != 2:
            return Err(_failure("fields", "mapping entries must be key-value pairs"))
        key, value = cast(tuple[object, object] | list[object], entry)
        if type(key) is not str:
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
    try:
        return _query_checked(query)
    except BaseException as error:  # foreign dataclass subclass/accessor
        return Err(_internal(f"could not inspect query input: {_foreign_message(error)}"))


def _query_checked(query: object) -> Result[dict[str, object]]:
    if not isinstance(query, LogQuery):
        return Err(_failure("query", "expected LogQuery"))
    if type(query.limit) is not int or not 1 <= query.limit <= 1000:
        return Err(_failure("limit", "expected an integer in 1..1000"))
    if query.order not in ("oldest_first", "newest_first"):
        return Err(_failure("order", "unsupported query order"))
    for field_name, value in (
        ("service", query.service),
        ("target", query.target),
        ("action", query.action),
        ("request_id", query.request_id),
        ("correlation_id", query.correlation_id),
        ("since", query.since),
        ("until", query.until),
    ):
        if value is not None and type(value) is not str:
            return Err(_failure(field_name, "expected a string or None"))
    if type(query.levels) is not tuple or any(
        type(level) is not str or level not in _LEVEL_FILTERS - {"off"}
        for level in query.levels
    ):
        return Err(_failure("levels", "expected a tuple of event levels"))
    if type(query.field_matches) is not tuple:
        return Err(_failure("field_matches", "expected a tuple of FieldMatch values"))
    matches: list[dict[str, object]] = []
    for index, match in enumerate(query.field_matches):
        if not isinstance(match, FieldMatch):
            return Err(_failure(f"field_matches[{index}]", "expected FieldMatch"))
        if type(match.field) is not str or not match.field:
            return Err(_failure(f"field_matches[{index}].field", "expected a nonempty string"))
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


def _level_filter(level: object) -> Result[str]:
    if type(level) is not str or level not in _LEVEL_FILTERS:
        return Err(_failure("level", "unsupported level filter"))
    return Ok(level)


def _level_change_source(source: object) -> Result[str]:
    if type(source) is not str or source not in _LEVEL_CHANGE_SOURCES:
        return Err(_failure("source", "unsupported level change source"))
    return Ok(source)


def _decode(name: str, payload: object) -> Result[object]:
    if type(payload) is not str:
        return Err(_internal("native result was not a JSON string"))
    try:
        wire = json.loads(payload)
        decoded = generated.from_wire(name, wire)
    except BaseException as error:
        return Err(_internal(f"invalid native result: {_foreign_message(error)}"))
    if getattr(decoded, "kind", None) == "error":
        return Err(cast(generated.Failure, getattr(decoded, "error")))
    return Ok(getattr(decoded, "value"))


def _decode_control(payload: object) -> Result[None]:
    """Decode the factory's `Result[None]` without inventing an untyped schema entrypoint."""
    if type(payload) is not str:
        return Err(_internal("native control result was not a JSON string"))
    try:
        wire = json.loads(payload)
        if wire.get("kind") == "ok" and wire.get("value") is None:
            return Ok(None)
        if wire.get("kind") == "error":
            return Err(cast(generated.Failure, generated.from_wire("OutputFailure", wire["error"])))
    except BaseException as error:
        return Err(_internal(f"invalid native control result: {_foreign_message(error)}"))
    return Err(_internal("invalid native control result shape"))


def _typed(value: Result[object]) -> Result[T]:
    if isinstance(value, Err):
        return value
    return Ok(cast(T, value.value))


def _native_call(name: str, call: Callable[[], object]) -> Result[object]:
    """Contain exceptions and malformed values from the foreign PyO3 object."""
    try:
        payload = call()
    except BaseException as error:
        return Err(_internal(f"native {name} failed: {_foreign_message(error)}"))
    return _decode(name, payload)


def _logger_config(config: object) -> Result[dict[str, object]]:
    try:
        return _logger_config_checked(config)
    except BaseException as error:  # foreign dataclass subclass/accessor
        return Err(_internal(f"could not inspect logger configuration: {_foreign_message(error)}"))


def _logger_config_checked(config: object) -> Result[dict[str, object]]:
    if not isinstance(config, LoggerConfig):
        return Err(_failure("config", "expected LoggerConfig"))
    if type(config.service) is not str or not config.service:
        return Err(_failure("service", "service must be a nonempty string"))
    if type(config.log_root) is not str or not config.log_root or "\x00" in config.log_root:
        return Err(_failure("log_root", "log_root must be a nonempty NUL-free string"))
    level = _level_filter(config.level)
    if isinstance(level, Err):
        return level
    if type(config.enable_file_sink) is not bool or type(config.enable_console_sink) is not bool:
        return Err(_failure("sinks", "sink flags must be booleans"))
    return Ok({
        "service": config.service,
        "log_root": str(Path(config.log_root)),
        "level": level.value,
        "enable_file_sink": config.enable_file_sink,
        "enable_console_sink": config.enable_console_sink,
    })


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
        return _typed(
            _native_call(
                "OutputResultDtoAdmissionDto",
                lambda: self._native.log(json.dumps(encoded.value, separators=(",", ":"))),
            )
        )

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
        return _typed(
            _native_call(
                "OutputResultDtoLogSnapshotDto",
                lambda: self._native.query(json.dumps(encoded.value, separators=(",", ":"))),
            )
        )

    def health(self) -> Result[generated.LogHealth]:
        return _typed(_native_call("OutputResultDtoLogHealthDto", lambda: self._native.health()))

    def flush(self, timeout_ms: int = 2000) -> Result[generated.Completion]:
        timeout = _timeout(timeout_ms)
        if isinstance(timeout, Err):
            return timeout
        return _typed(_native_call("OutputResultDtoCompletionDto", lambda: self._native.flush(timeout.value)))

    def shutdown(self, timeout_ms: int = 2000) -> Result[generated.LogHealth]:
        timeout = _timeout(timeout_ms)
        if isinstance(timeout, Err):
            return timeout
        return _typed(_native_call("OutputResultDtoLogHealthDto", lambda: self._native.shutdown(timeout.value)))

    def wait_stopped(self, timeout_ms: int = 2000) -> Result[generated.LogHealth]:
        timeout = _timeout(timeout_ms)
        if isinstance(timeout, Err):
            return timeout
        return _typed(_native_call("OutputResultDtoLogHealthDto", lambda: self._native.wait_stopped(timeout.value)))

    def elevate_level(
        self, level: LevelFilter, source: LevelChangeSource = "application"
    ) -> Result[generated.LevelChange]:
        validated_level = _level_filter(level)
        if isinstance(validated_level, Err):
            return validated_level
        validated_source = _level_change_source(source)
        if isinstance(validated_source, Err):
            return validated_source
        return _typed(_native_call(
            "OutputResultDtoLevelChangeDto",
            lambda: self._native.elevate_level(validated_level.value, validated_source.value),
        ))

    def reset_level(
        self, source: LevelChangeSource = "application"
    ) -> Result[generated.LevelChange]:
        validated_source = _level_change_source(source)
        if isinstance(validated_source, Err):
            return validated_source
        return _typed(_native_call("OutputResultDtoLevelChangeDto", lambda: self._native.reset_level(validated_source.value)))


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
        return _typed(
            _native_call(
                "OutputResultDtoAdmissionDto",
                lambda: self._native.log(json.dumps(encoded.value, separators=(",", ":"))),
            )
        )

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
        return _typed(
            _native_call(
                "OutputResultDtoLogSnapshotDto",
                lambda: self._native.query(json.dumps(encoded.value, separators=(",", ":"))),
            )
        )

    def health(self) -> Result[generated.LogHealth]:
        return _typed(_native_call("OutputResultDtoLogHealthDto", lambda: self._native.health()))

    def flush(self, timeout_ms: int = 2000) -> Result[generated.Completion]:
        timeout = _timeout(timeout_ms)
        if isinstance(timeout, Err):
            return timeout
        return _typed(_native_call("OutputResultDtoCompletionDto", lambda: self._native.flush(timeout.value)))


def create_logger(config: LoggerConfig) -> Result[Logger]:
    """Create one independent owned logger without installing global logging."""
    encoded = _logger_config(config)
    if isinstance(encoded, Err):
        return encoded
    try:
        native_module = cast(Any, importlib.import_module("sc_observability._native"))
        native, result = native_module.create_owned(json.dumps(encoded.value, separators=(",", ":")))
    except BaseException as error:  # foreign extension import/runtime failure only
        return Err(_internal(f"native extension unavailable: {_foreign_message(error)}"))
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
    except BaseException as error:  # foreign extension import/runtime failure only
        return Err(_internal(f"native extension unavailable: {_foreign_message(error)}"))
    decoded = _decode_control(result)
    if isinstance(decoded, Err):
        return decoded
    if native is None:
        return Err(_internal("native host factory reported success without a logger"))
    return Ok(AttachedLogger(cast(_NativeReadable, native)))


from .async_logging import LogReceipt, ReceiptState, Resolved


__all__ = [
    "LogReceipt", "ReceiptState", "Resolved",
    "AttachedLogger", "Err", "FieldMatch", "LogEvent", "Logger", "LoggerConfig",
    "LogQuery", "Ok", "Result", "TraceContext", "create_logger", "get_host_logger",
]
