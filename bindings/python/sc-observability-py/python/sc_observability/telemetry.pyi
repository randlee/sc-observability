from dataclasses import dataclass, field
from pathlib import Path
from typing import Any, Literal, Mapping, Sequence, TypeAlias, TypeVar
from . import Ok

T = TypeVar("T")

@dataclass(frozen=True)
class TelemetryFailure:
    kind: Literal['submission', 'admission', 'delivery', 'config', 'internal']
    variant: str
    code: str
    message: str
    path: str | None = ...
    report: FlushReport | None = ...

@dataclass(frozen=True)
class TelemetryErr:
    error: TelemetryFailure
    kind: Literal['error'] = field(default='error', init=False)

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

class Telemetry:
    last_shutdown: TelemetryResult[FlushReport] | None
    @classmethod
    def open(cls, config: str | Path | None = ..., *, store_path: str | Path | None = ..., endpoint: str | None = ..., service_name: str | None = ...) -> TelemetryResult[Telemetry]: ...
    def emit(self, input: Mapping[str, Any]) -> TelemetryResult[AdmissionReceipt]: ...
    def flush(self, timeout_s: float | None = ...) -> TelemetryResult[FlushReport]: ...
    def flush_submission(self, submission_id: str, timeout_s: float | None = ...) -> TelemetryResult[FlushReport]: ...
    def shutdown(self, timeout_s: float | None = ...) -> TelemetryResult[FlushReport]: ...
    def status(self, *, submissions: Sequence[str] | None = ..., record_keys: Sequence[str] | None = ...) -> TelemetryResult[StoreStatus]: ...
    @classmethod
    def _with_test_double(cls, script_json: str | None = ..., **kwargs: Any) -> TelemetryResult[Telemetry]: ...
    def __enter__(self) -> Telemetry: ...
    def __exit__(self, _type: object, _value: object, _traceback: object) -> bool: ...

def build_envelope(input: Mapping[str, Any]) -> TelemetryResult[str]: ...
