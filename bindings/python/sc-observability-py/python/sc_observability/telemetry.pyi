from dataclasses import dataclass, field
from os import PathLike
from typing import Any, Literal, Mapping, Sequence, TypeAlias, TypeVar
from . import Ok

T = TypeVar("T")

@dataclass(frozen=True)
class TelemetryFailure:
    kind: Literal['submission', 'admission', 'delivery', 'config', 'internal', 'unknown']
    variant: str
    code: str
    message: str
    path: str | None = ...
    report: FlushReport | None = ...
    remediation: Mapping[str, object] | None = ...
    cause: str | None = ...

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

class Telemetry:
    def __init__(self, native: object) -> None: ...
    last_shutdown: TelemetryResult[FlushReport] | None
    @classmethod
    def open(cls, config: str | PathLike[str] | None = ..., *, store_path: str | PathLike[str] | None = ..., endpoint: str | None = ..., service_name: str | None = ...) -> TelemetryResult[Telemetry]: ...
    @classmethod
    def _with_test_double(cls, script_json: str | None = ..., **kwargs: Any) -> TelemetryResult[Telemetry]: ...
    def emit(self, input: Mapping[str, Any]) -> TelemetryResult[AdmissionReceipt]: ...
    def flush(self, timeout_s: float | None = ...) -> TelemetryResult[FlushReport]: ...
    def flush_submission(self, submission_id: str, timeout_s: float | None = ...) -> TelemetryResult[FlushReport]: ...
    def shutdown(self, timeout_s: float | None = ...) -> TelemetryResult[FlushReport]: ...
    def status(self, *, submissions: Sequence[str] | None = ..., record_keys: Sequence[str] | None = ...) -> TelemetryResult[StoreStatus]: ...
    def __enter__(self) -> Telemetry: ...
    def __exit__(self, *exc: object) -> Literal[False]: ...

def build_envelope(input: Mapping[str, Any]) -> TelemetryResult[str]: ...
