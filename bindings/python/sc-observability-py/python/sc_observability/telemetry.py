"""Telemetry submission facade backed by the shared Rust submission client."""
from __future__ import annotations

from dataclasses import dataclass, field
import importlib
import json
import math
from pathlib import Path
from typing import Any, Literal, Mapping, Sequence, TypeAlias, TypeVar, cast

from . import Ok

T = TypeVar("T")


@dataclass(frozen=True)
class TelemetryFailure:
    kind: Literal["submission", "admission", "delivery", "config", "internal"]
    variant: str
    code: str
    message: str
    path: str | None = None
    report: "FlushReport | None" = None


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
    lease: Mapping[str, object] | None
    submissions: tuple[Mapping[str, object], ...]


def _counts(value: object) -> SignalCounts:
    source = cast(Mapping[str, object], value) if isinstance(value, Mapping) else {}
    return SignalCounts(*(int(source.get(name, 0)) for name in ("logs", "traces", "metrics", "profiles")))


def _flush_report(value: object) -> FlushReport:
    source = cast(Mapping[str, object], value) if isinstance(value, Mapping) else {}
    return FlushReport(*(_counts(source.get(name)) for name in ("delivered", "still_pending", "failed", "evicted")))


def _admission_receipt(value: object) -> AdmissionReceipt:
    source = cast(Mapping[str, object], value) if isinstance(value, Mapping) else {}
    signals = source.get("signals", ())
    return AdmissionReceipt(str(source.get("submission_id", "")), cast(str | None, source.get("record_key")), str(source.get("admitted_at", "")), tuple(map(str, signals if isinstance(signals, list) else ())), bool(source.get("duplicate", False)))


def _store_status(value: object) -> StoreStatus:
    source = cast(Mapping[str, object], value) if isinstance(value, Mapping) else {}
    submissions = source.get("submissions", ())
    return StoreStatus(
        *(int(source.get(name, 0)) for name in ("schema_version", "store_bytes", "max_store_bytes")),
        *(_counts(source.get(name)) for name in ("pending", "retry_scheduled", "delivered_retained", "failed")),
        *(int(source.get(name, 0)) for name in ("evicted_by_disk_bound", "rejected_by_disk_bound", "unreadable_newer_envelopes")),
        cast(Mapping[str, object] | None, source.get("lease")),
        tuple(cast(Mapping[str, object], item) for item in submissions if isinstance(item, Mapping)) if isinstance(submissions, list) else (),
    )


def _result(payload: object, decode: Any = lambda value: value) -> TelemetryResult[object]:
    if not isinstance(payload, Mapping):
        return TelemetryErr(TelemetryFailure("internal", "decode", "SC_OBSERVABILITY_BINDING_INTERNAL", "native telemetry returned an invalid result"))
    if payload.get("kind") == "ok":
        return Ok(decode(payload.get("value")))
    error = payload.get("error")
    if not isinstance(error, Mapping):
        return TelemetryErr(TelemetryFailure("internal", "decode", "SC_OBSERVABILITY_BINDING_INTERNAL", "native telemetry returned an invalid failure"))
    kind = str(error.get("kind", "internal"))
    if kind not in {"submission", "admission", "delivery", "config", "internal"}:
        kind = "internal"
    return TelemetryErr(TelemetryFailure(
        kind=cast(Literal["submission", "admission", "delivery", "config", "internal"], kind),
        variant=str(error.get("variant", "unknown")),
        code=str(error.get("code", "SC_OBSERVABILITY_BINDING_INTERNAL")),
        message=str(error.get("message", "unknown telemetry failure")),
        path=cast(str | None, error.get("path")),
        report=_flush_report(error["report"]) if error.get("report") is not None else None,
    ))


def _call(function: str, *args: object) -> TelemetryResult[object]:
    try:
        native = importlib.import_module("sc_observability._native")
        raw = getattr(native, function)(*args)
        if isinstance(raw, tuple):
            raw = raw[-1]
        return _result(json.loads(cast(str, raw)))
    except BaseException as error:
        return TelemetryErr(TelemetryFailure("internal", "native", "SC_OBSERVABILITY_BINDING_INTERNAL", f"native telemetry unavailable: {error}"))


def _milliseconds(timeout_s: float | None) -> int | None:
    if timeout_s is None:
        return None
    return max(0, math.ceil(timeout_s * 1000))


class Telemetry:
    """Owned admission and delivery handle for canonical telemetry submissions."""

    def __init__(self, native: object) -> None:
        self._native = native
        self.last_shutdown: TelemetryResult[object] | None = None

    @classmethod
    def open(
        cls,
        config: str | Path | None = None,
        *,
        store_path: str | Path | None = None,
        endpoint: str | None = None,
        service_name: str | None = None,
    ) -> TelemetryResult["Telemetry"]:
        args = {"config": str(config) if config is not None else None, "store_path": str(store_path) if store_path is not None else None, "endpoint": endpoint, "service_name": service_name}
        try:
            native = importlib.import_module("sc_observability._native")
            handle, raw = native.open(json.dumps(args, separators=(",", ":")))
            result = _result(json.loads(raw))
        except BaseException as error:
            return TelemetryErr(TelemetryFailure("internal", "native", "SC_OBSERVABILITY_BINDING_INTERNAL", f"native telemetry unavailable: {error}"))
        if isinstance(result, TelemetryErr):
            return result
        if handle is None:
            return TelemetryErr(TelemetryFailure("internal", "factory", "SC_OBSERVABILITY_BINDING_INTERNAL", "native telemetry factory returned no handle"))
        return Ok(cls(handle))

    def emit(self, input: Mapping[str, Any]) -> TelemetryResult[AdmissionReceipt]:
        if not isinstance(input, Mapping):
            raise TypeError("input must be a Mapping")
        try:
            payload = json.dumps(input, separators=(",", ":"))
        except (TypeError, ValueError) as error:
            return TelemetryErr(TelemetryFailure("submission", "invalid_json", "SC_OBSERVABILITY_SUBMISSION_INVALID_JSON", str(error)))
        return cast(TelemetryResult[AdmissionReceipt], _result(json.loads(self._native.emit(payload)), _admission_receipt))

    def flush(self, timeout_s: float | None = None) -> TelemetryResult[FlushReport]:
        return cast(TelemetryResult[FlushReport], _result(json.loads(self._native.flush(_milliseconds(timeout_s))), _flush_report))

    def flush_submission(self, submission_id: str, timeout_s: float | None = None) -> TelemetryResult[FlushReport]:
        return cast(TelemetryResult[FlushReport], _result(json.loads(self._native.flush_submission(submission_id, _milliseconds(timeout_s))), _flush_report))

    def shutdown(self, timeout_s: float | None = None) -> TelemetryResult[FlushReport]:
        result = cast(TelemetryResult[FlushReport], _result(json.loads(self._native.shutdown(_milliseconds(timeout_s))), _flush_report))
        self.last_shutdown = result
        return result

    def status(
        self, *, submissions: Sequence[str] | None = None, record_keys: Sequence[str] | None = None
    ) -> TelemetryResult[StoreStatus]:
        if submissions is not None and record_keys is not None:
            raise ValueError("status accepts submissions or record_keys, not both")
        query: object = "summary"
        if submissions is not None:
            query = {"submissions": list(submissions)}
        elif record_keys is not None:
            query = {"record_keys": list(record_keys)}
        return cast(TelemetryResult[StoreStatus], _result(json.loads(self._native.status(json.dumps(query, separators=(",", ":")))), _store_status))

    def __enter__(self) -> "Telemetry":
        return self

    def __exit__(self, _type: object, _value: object, _traceback: object) -> bool:
        self.shutdown()
        return False

    @classmethod
    def _with_test_double(cls, script_json: str | None = None, **kwargs: Any) -> TelemetryResult["Telemetry"]:
        return _open_test_double(script_json=script_json, **kwargs)


def _open_test_double(
    *, store_path: str | Path, endpoint: str, service_name: str, script_json: str | None = None
) -> TelemetryResult[Telemetry]:
    """Create the test-hooks-only in-memory client; this is not a release API."""
    try:
        native = importlib.import_module("sc_observability._native")
        args = json.dumps({"config": None, "store_path": str(store_path), "endpoint": endpoint, "service_name": service_name}, separators=(",", ":"))
        handle, raw = native._open_test_double(args, script_json)
        result = _result(json.loads(raw))
    except BaseException as error:
        return TelemetryErr(TelemetryFailure("internal", "native", "SC_OBSERVABILITY_BINDING_INTERNAL", f"native telemetry unavailable: {error}"))
    if isinstance(result, TelemetryErr):
        return result
    if handle is None:
        return TelemetryErr(TelemetryFailure("internal", "factory", "SC_OBSERVABILITY_BINDING_INTERNAL", "native telemetry factory returned no handle"))
    return Ok(Telemetry(handle))


def build_envelope(input: Mapping[str, Any]) -> TelemetryResult[str]:
    """Validate one canonical submission document without opening a store."""
    if not isinstance(input, Mapping):
        raise TypeError("input must be a Mapping")
    try:
        payload = json.dumps(input, separators=(",", ":"))
    except (TypeError, ValueError) as error:
        return TelemetryErr(TelemetryFailure("submission", "invalid_json", "SC_OBSERVABILITY_SUBMISSION_INVALID_JSON", str(error)))
    return cast(TelemetryResult[str], _call("build_envelope", payload))
