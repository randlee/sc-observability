"""Telemetry submission facade backed by the shared Rust submission client."""
from __future__ import annotations

from dataclasses import dataclass, field
import importlib
import json
import math
from os import PathLike
from types import MappingProxyType
from typing import Any, Callable, Literal, Mapping, Sequence, TypeAlias, TypeVar, cast

from . import Ok, generated

T = TypeVar("T")


@dataclass(frozen=True)
class TelemetryFailure:
    kind: Literal["submission", "admission", "delivery", "config", "internal", "unknown"]
    variant: str
    code: str
    message: str
    path: str | None = None
    report: "FlushReport | None" = None
    remediation: Mapping[str, object] | None = None
    cause: str | None = None


@dataclass(frozen=True)
class TelemetryErr:
    error: TelemetryFailure
    kind: Literal["error"] = field(default="error", init=False)


TelemetryResult: TypeAlias = Ok[T] | TelemetryErr


@dataclass(frozen=True)
class SignalCounts:
    logs: int
    traces: int
    metrics: int
    profiles: int


@dataclass(frozen=True)
class AdmissionReceipt:
    submission_id: str
    record_key: str | None
    admitted_at: str
    signals: tuple[str, ...]
    duplicate: bool


@dataclass(frozen=True)
class FlushReport:
    delivered: SignalCounts
    still_pending: SignalCounts
    failed: SignalCounts
    evicted: SignalCounts


@dataclass(frozen=True)
class LeaseInfo:
    holder: str
    expires_at: str


@dataclass(frozen=True)
class DeliveryStatus:
    submission_id: str
    signals: tuple[tuple[str, Mapping[str, object]], ...]


@dataclass(frozen=True)
class StoreStatus:
    schema_version: int
    store_bytes: int
    max_store_bytes: int
    pending: SignalCounts
    retry_scheduled: SignalCounts
    delivered_retained: SignalCounts
    failed: SignalCounts
    evicted_by_disk_bound: int
    rejected_by_disk_bound: int
    unreadable_newer_envelopes: int
    lease: LeaseInfo | None
    submissions: tuple[DeliveryStatus, ...]



def _object(value: object) -> Mapping[str, Any]:
    if not isinstance(value, dict):
        raise ValueError("expected an object")
    return value


def _string(value: object) -> str:
    if not isinstance(value, str):
        raise ValueError("expected a string")
    return value


def _optional_string(value: object) -> str | None:
    return None if value is None else _string(value)


def _integer(value: object) -> int:
    if type(value) is not int or value < 0:
        raise ValueError("expected a non-negative integer")
    return value


def _boolean(value: object) -> bool:
    if type(value) is not bool:
        raise ValueError("expected a boolean")
    return value


def _list(value: object) -> list[Any]:
    if not isinstance(value, list):
        raise ValueError("expected an array")
    return value


def _signal(value: object) -> str:
    text = _string(value)
    if text not in {"logs", "traces", "metrics", "profiles"}:
        raise ValueError("unknown signal")
    return text


def _counts(value: object) -> SignalCounts:
    source = _object(value)
    return SignalCounts(*(_integer(source[name]) for name in ("logs", "traces", "metrics", "profiles")))


def _flush_report(value: object) -> FlushReport:
    source = _object(value)
    return FlushReport(*(_counts(source[name]) for name in ("delivered", "still_pending", "failed", "evicted")))


def _admission_receipt(value: object) -> AdmissionReceipt:
    source = _object(value)
    return AdmissionReceipt(_string(source["submission_id"]), _optional_string(source["record_key"]),
                            _string(source["admitted_at"]), tuple(_signal(s) for s in _list(source["signals"])),
                            _boolean(source["duplicate"]))


def _delivery_state(value: object) -> Mapping[str, object]:
    source = _object(value)
    state = _string(source["state"])
    fields = {
        "pending": {}, "claimed": {"holder": _string, "attempts": _integer},
        "retry_scheduled": {"attempts": _integer, "next_attempt_at": _string, "last_error": _string},
        "delivered": {"at": _string, "attempts": _integer},
        "failed": {"attempts": _integer, "error": _string}, "evicted_by_disk_bound": {"at": _string},
    }
    if state not in fields:
        raise ValueError("unknown delivery state")
    result: dict[str, object] = {"state": state}
    for name, decode in fields[state].items():
        result[name] = decode(source[name])
    return MappingProxyType(result)


def _delivery_status(value: object) -> DeliveryStatus:
    source = _object(value)
    signals = []
    for value_pair in _list(source["signals"]):
        pair = _list(value_pair)
        if len(pair) != 2:
            raise ValueError("expected signal/state pair")
        signals.append((_signal(pair[0]), _delivery_state(pair[1])))
    return DeliveryStatus(_string(source["submission_id"]), tuple(signals))


def _store_status(value: object) -> StoreStatus:
    source = _object(value)
    lease = source["lease"]
    if lease is not None:
        lease = _object(lease)
        lease = LeaseInfo(_string(lease["holder"]), _string(lease["expires_at"]))
    return StoreStatus(
        *(_integer(source[name]) for name in ("schema_version", "store_bytes", "max_store_bytes")),
        *(_counts(source[name]) for name in ("pending", "retry_scheduled", "delivered_retained", "failed")),
        *(_integer(source[name]) for name in ("evicted_by_disk_bound", "rejected_by_disk_bound", "unreadable_newer_envelopes")),
        lease, tuple(_delivery_status(item) for item in _list(source["submissions"])),
    )


def _remediation(value: object) -> Mapping[str, object] | None:
    if value is None:
        return None
    source = _object(value)
    kind = _string(source["kind"])
    if kind == "not_recoverable":
        return MappingProxyType({"kind": kind, "justification": _string(source["justification"])})
    if kind == "recoverable":
        steps = _list(_object(source["steps"])["steps"])
        return MappingProxyType({"kind": kind, "steps": tuple(_string(step) for step in steps)})
    raise ValueError("unknown remediation kind")


def _internal(variant: str, error: object) -> TelemetryErr:
    return TelemetryErr(TelemetryFailure("internal", variant, generated.SC_OBSERVABILITY_BINDING_INTERNAL,
                        "native telemetry operation failed", cause=str(error)))


def _result(payload: object, decode: Callable[[object], T]) -> TelemetryResult[T]:
    try:
        source = _object(payload)
        if source["kind"] == "ok":
            return Ok(decode(source["value"]))
        if source["kind"] != "error":
            raise ValueError("unknown result tag")
        error = _object(source["error"])
        kind = _string(error["kind"])
        if kind not in {"submission", "admission", "delivery", "config", "internal", "unknown"}:
            raise ValueError("unknown failure kind")
        return TelemetryErr(TelemetryFailure(
            kind=cast(Any, kind), variant=_string(error["variant"]), code=_string(error["code"]),
            message=_string(error["message"]), path=_optional_string(error["path"]),
            report=None if error["report"] is None else _flush_report(error["report"]),
            remediation=_remediation(error["remediation"]), cause=_optional_string(error["cause"]),
        ))
    except Exception as error:
        return _internal("decode", error)


def _invoke(action: Callable[[], str], decode: Callable[[object], T]) -> TelemetryResult[T]:
    try:
        raw = action()
    except Exception as error:
        return _internal("native", error)
    try:
        return _result(json.loads(raw), decode)
    except Exception as error:
        return _internal("decode", error)


def _module() -> Any:
    return importlib.import_module("sc_observability._native")


def _serialized(input: Mapping[str, Any]) -> str | TelemetryErr:
    if not isinstance(input, Mapping):
        raise TypeError("input must be a Mapping")
    try:
        return json.dumps(dict(input), separators=(",", ":"), allow_nan=False)
    except Exception as error:
        # Let the Rust parser own the registered invalid_json code and remediation.
        result = _invoke(lambda: _module().build_envelope(""), _string)
        if isinstance(result, TelemetryErr):
            from dataclasses import replace
            return TelemetryErr(replace(result.error, cause=str(error)))
        return _internal("serialization", error)


def _milliseconds(timeout_s: float | None) -> int | None | TelemetryErr:
    if timeout_s is None:
        return None
    try:
        if not math.isfinite(timeout_s) or timeout_s < 0:
            raise ValueError("timeout_s must be finite and non-negative")
        milliseconds = math.ceil(timeout_s * 1000)
        if milliseconds > 2**64 - 1:
            raise ValueError("timeout_s exceeds the u64 millisecond range")
        return milliseconds
    except (TypeError, ValueError, OverflowError) as error:
        try:
            code = _module().TELEMETRY_CONFIG_INVALID
        except Exception as native_error:
            return _internal("native", native_error)
        return TelemetryErr(TelemetryFailure("config", "invalid_field", code, str(error),
            remediation=MappingProxyType({"kind": "not_recoverable", "justification": "Supply a finite non-negative timeout within the u64 millisecond range."})))


class Telemetry:
    """Owned admission and delivery handle for canonical telemetry submissions."""

    def __init__(self, native: Any) -> None:
        self._native = native
        self.last_shutdown: TelemetryResult[FlushReport] | None = None

    @classmethod
    def open(cls, config: str | PathLike[str] | None = None, *,
             store_path: str | PathLike[str] | None = None, endpoint: str | None = None,
             service_name: str | None = None) -> TelemetryResult["Telemetry"]:
        return _factory(cls, "open", config, store_path, endpoint, service_name)

    def emit(self, input: Mapping[str, Any]) -> TelemetryResult[AdmissionReceipt]:
        payload = _serialized(input)
        if isinstance(payload, TelemetryErr):
            return payload
        return _invoke(lambda: self._native.emit(payload), _admission_receipt)

    def _flush(self, method: str, timeout_s: float | None, *args: object) -> TelemetryResult[FlushReport]:
        timeout = _milliseconds(timeout_s)
        if isinstance(timeout, TelemetryErr):
            return timeout
        return _invoke(lambda: getattr(self._native, method)(*args, timeout), _flush_report)

    def flush(self, timeout_s: float | None = None) -> TelemetryResult[FlushReport]:
        return self._flush("flush", timeout_s)

    def flush_submission(self, submission_id: str, timeout_s: float | None = None) -> TelemetryResult[FlushReport]:
        return self._flush("flush_submission", timeout_s, submission_id)

    def shutdown(self, timeout_s: float | None = None) -> TelemetryResult[FlushReport]:
        result = self._flush("shutdown", timeout_s)
        self.last_shutdown = result
        return result

    def status(self, *, submissions: Sequence[str] | None = None,
               record_keys: Sequence[str] | None = None) -> TelemetryResult[StoreStatus]:
        if submissions is not None and record_keys is not None:
            raise ValueError("status accepts submissions or record_keys, not both")
        def call() -> str:
            query: object = "summary"
            if submissions is not None:
                query = {"submissions": list(submissions)}
            elif record_keys is not None:
                query = {"record_keys": list(record_keys)}
            return cast(str, self._native.status(json.dumps(query, separators=(",", ":"))))
        return _invoke(call, _store_status)

    def __enter__(self) -> "Telemetry":
        return self

    def __exit__(self, _type: object, _value: object, _traceback: object) -> Literal[False]:
        self.shutdown()
        return False


def _factory(cls: type[Telemetry], function: str, config: str | PathLike[str] | None,
             store_path: str | PathLike[str] | None, endpoint: str | None, service_name: str | None,
             *extra: object) -> TelemetryResult[Telemetry]:
    def call() -> str:
        args = {"config": str(config) if config is not None else None,
                "store_path": str(store_path) if store_path is not None else None,
                "endpoint": endpoint, "service_name": service_name}
        handle, raw = getattr(_module(), function)(json.dumps(args, separators=(",", ":")), *extra)
        handles.append(handle)
        return cast(str, raw)
    def decode(value: object) -> Telemetry:
        if value is not None or not handles or handles[0] is None:
            raise ValueError("native telemetry factory returned no handle or invalid value")
        return cls(handles[0])
    handles: list[Any] = []
    return _invoke(call, decode)


def build_envelope(input: Mapping[str, Any]) -> TelemetryResult[str]:
    """Validate one canonical submission document without opening a store."""
    payload = _serialized(input)
    if isinstance(payload, TelemetryErr):
        return payload
    return _invoke(lambda: _module().build_envelope(payload), _string)
