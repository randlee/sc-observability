"""Result-returning owned Python logging API over the shared native backend.

The public facade accepts ergonomic frozen input values.  It encodes them once
to the canonical schema and lets the Rust DTO/runtime crates own validation of
native values, provenance stamping, admission, and lifecycle conversion.
"""
from __future__ import annotations

from dataclasses import dataclass, field
from datetime import datetime, timezone
import json
import math
from pathlib import Path
from typing import Generic, Literal, Mapping, TypeAlias, TypeVar

from . import generated

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


def _value(value: object, path: str, seen: set[int], depth: int = 0) -> dict[str, object]:
    if depth >= 32:
        raise ValueError(f"{path}: maximum container depth is 32")
    if value is None:
        return {"kind": "null"}
    if type(value) is bool:
        return {"kind": "boolean", "value": value}
    if isinstance(value, str):
        return {"kind": "string", "value": value}
    if type(value) is int:
        if not -(2**63) <= value < 2**64:
            raise ValueError(f"{path}: integer is outside the shared wire range")
        return {"kind": "integer", "value": str(value)}
    if type(value) is float:
        if not math.isfinite(value):
            raise ValueError(f"{path}: float must be finite")
        return {"kind": "float", "value": value}
    if isinstance(value, (tuple, list)):
        identity = id(value)
        if identity in seen:
            raise ValueError(f"{path}: cyclic container")
        seen.add(identity)
        try:
            return {
                "kind": "array",
                "value": [_value(item, f"{path}[{index}]", seen, depth + 1) for index, item in enumerate(value)],
            }
        finally:
            seen.remove(identity)
    if isinstance(value, Mapping):
        identity = id(value)
        if identity in seen:
            raise ValueError(f"{path}: cyclic container")
        seen.add(identity)
        try:
            encoded: dict[str, object] = {}
            for key, item in value.items():
                if not isinstance(key, str):
                    raise ValueError(f"{path}: object keys must be strings")
                if _normalised_key(key).startswith("sc_observability.binding."):
                    raise ValueError(f"{path}.{key}: reserved binding provenance field")
                encoded[key] = _value(item, f"{path}.{key}", seen, depth + 1)
            return {"kind": "object", "value": encoded}
        finally:
            seen.remove(identity)
    raise ValueError(f"{path}: unsupported value type {type(value).__name__}")


def _event(event: LogEvent) -> dict[str, object]:
    if event.level not in ("trace", "debug", "info", "warn", "error"):
        raise ValueError("level: unsupported level")
    trace = None
    if event.trace is not None:
        trace = {
            "trace_id": event.trace.trace_id,
            "span_id": event.trace.span_id,
            "parent_span_id": event.trace.parent_span_id,
        }
    fields: dict[str, object] = {}
    for key, value in event.fields.items():
        if not isinstance(key, str):
            raise ValueError("fields: object keys must be strings")
        if _normalised_key(key).startswith("sc_observability.binding."):
            raise ValueError(f"fields.{key}: reserved binding provenance field")
        fields[key] = _value(value, f"fields.{key}", set())
    return {
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
    }


def _query(query: LogQuery) -> dict[str, object]:
    if type(query.limit) is not int or not 1 <= query.limit <= 1000:
        raise ValueError("limit: expected an integer in 1..1000")
    if query.order not in ("oldest_first", "newest_first"):
        raise ValueError("order: unsupported query order")
    return {
        "schema_version": 1,
        "service": query.service,
        "levels": list(query.levels),
        "target": query.target,
        "action": query.action,
        "request_id": query.request_id,
        "correlation_id": query.correlation_id,
        "since": query.since,
        "until": query.until,
        "field_matches": [
            {"field": match.field, "value": _value(match.value, f"field_matches[{index}].value", set())}
            for index, match in enumerate(query.field_matches)
        ],
        "limit": query.limit,
        "order": query.order,
    }


def _timeout(timeout_ms: object) -> str:
    if type(timeout_ms) is not int or not 0 <= timeout_ms <= 60_000:
        raise ValueError("timeout_ms: expected integer milliseconds in 0..60000")
    return json.dumps(timeout_ms)


def _decode(name: str, payload: str) -> Result[object]:
    try:
        wire = json.loads(payload)
        decoded = generated.from_wire(name, wire)
    except (TypeError, ValueError, json.JSONDecodeError) as error:
        return Err(_internal(f"invalid native result: {error}"))
    if getattr(decoded, "kind", None) == "error":
        return Err(decoded.error)
    return Ok(decoded.value)


def _decode_control(payload: str) -> Result[None]:
    """Decode the factory's `Result[None]` without inventing an untyped schema entrypoint."""
    try:
        wire = json.loads(payload)
        if wire.get("kind") == "ok" and wire.get("value") is None:
            return Ok(None)
        if wire.get("kind") == "error":
            return Err(generated.from_wire("OutputFailure", wire["error"]))
    except (AttributeError, TypeError, ValueError, json.JSONDecodeError) as error:
        return Err(_internal(f"invalid native control result: {error}"))
    return Err(_internal("invalid native control result shape"))


class Logger:
    """An independently owned logger; it is never process-global."""

    def __init__(self, native: object) -> None:
        self._native = native

    def log(self, event: LogEvent) -> Result[generated.Admission]:
        try:
            payload = json.dumps(_event(event), separators=(",", ":"))
        except (TypeError, ValueError) as error:
            return Err(_failure("event", str(error)))
        return _decode("OutputResultDtoAdmissionDto", self._native.log(payload))

    def query(self, query: LogQuery) -> Result[generated.LogSnapshot]:
        try:
            payload = json.dumps(_query(query), separators=(",", ":"))
        except (TypeError, ValueError) as error:
            return Err(_failure("query", str(error)))
        return _decode("OutputResultDtoLogSnapshotDto", self._native.query(payload))

    def health(self) -> Result[generated.LogHealth]:
        return _decode("OutputResultDtoLogHealthDto", self._native.health())

    def flush(self, timeout_ms: int = 2000) -> Result[generated.Completion]:
        try:
            timeout = _timeout(timeout_ms)
        except ValueError as error:
            return Err(_failure("timeout_ms", str(error)))
        return _decode("OutputResultDtoCompletionDto", self._native.flush(timeout))

    def shutdown(self, timeout_ms: int = 2000) -> Result[generated.LogHealth]:
        try:
            timeout = _timeout(timeout_ms)
        except ValueError as error:
            return Err(_failure("timeout_ms", str(error)))
        return _decode("OutputResultDtoLogHealthDto", self._native.shutdown(timeout))

    def wait_stopped(self, timeout_ms: int = 2000) -> Result[generated.LogHealth]:
        try:
            timeout = _timeout(timeout_ms)
        except ValueError as error:
            return Err(_failure("timeout_ms", str(error)))
        return _decode("OutputResultDtoLogHealthDto", self._native.wait_stopped(timeout))

    def elevate_level(
        self, level: LevelFilter, source: LevelChangeSource = "application"
    ) -> Result[generated.LevelChange]:
        return _decode("OutputResultDtoLevelChangeDto", self._native.elevate_level(level, source))

    def reset_level(
        self, source: LevelChangeSource = "application"
    ) -> Result[generated.LevelChange]:
        return _decode("OutputResultDtoLevelChangeDto", self._native.reset_level(source))


class AttachedLogger:
    """A non-owning view of a Rust host logger (implemented by the host bridge)."""


def create_logger(config: LoggerConfig) -> Result[Logger]:
    """Create one independent owned logger without installing global logging."""
    if not isinstance(config, LoggerConfig):
        return Err(_failure("config", "expected LoggerConfig"))
    if not config.service:
        return Err(_failure("service", "service must be nonempty"))
    if not config.log_root or "\x00" in config.log_root:
        return Err(_failure("log_root", "log_root must be nonempty and NUL-free"))
    try:
        from . import _native
        native, result = _native.create_owned(
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
    return Ok(Logger(native))


def get_host_logger() -> Result[AttachedLogger]:
    """Return a host-attached logger after Rust embedding installs one."""
    return Err(
        _unavailable(
            generated.SC_OBSERVABILITY_BINDING_HOST_NOT_INSTALLED,
            "host logger has not been installed in this module",
            "Install a host backend before requesting an attached logger",
        )
    )


__all__ = [
    "AttachedLogger", "Err", "FieldMatch", "LogEvent", "Logger", "LoggerConfig",
    "LogQuery", "Ok", "Result", "TraceContext", "create_logger", "get_host_logger",
]
