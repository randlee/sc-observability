from dataclasses import dataclass, field
from os import PathLike
from typing import Literal, Mapping, TypeAlias
from . import Ok

AttributeValue: TypeAlias = str | bool | int | float
Severity: TypeAlias = Literal['trace', 'debug', 'info', 'warn', 'error', 'fatal']
SpanKind: TypeAlias = Literal['internal', 'server', 'client', 'producer', 'consumer']
MetricKind: TypeAlias = Literal['counter', 'up_down_counter', 'gauge', 'histogram']

@dataclass(frozen=True)
class TelemetryFailure:
    kind: Literal['validation', 'export', 'internal']
    code: str
    message: str

@dataclass(frozen=True)
class TelemetryErr:
    error: TelemetryFailure
    kind: Literal['error'] = field(default='error', init=False)

TelemetryResult: TypeAlias = Ok[None] | TelemetryErr

class Telemetry:
    def __init__(self, endpoint: str | None = ..., *, headers: Mapping[str, str] | None = ..., timeout_s: float | None = ..., root_certificate: str | PathLike[str] | None = ..., service_name: str | None = ...) -> None: ...
    def log(self, body: str, *, severity: Severity = ..., trace_id: str | None = ..., span_id: str | None = ..., attributes: Mapping[str, AttributeValue] | None = ...) -> TelemetryResult: ...
    def span(self, name: str, *, trace_id: str | None = ..., span_id: str | None = ..., parent_span_id: str | None = ..., kind: SpanKind = ..., start_time_unix_nano: int | None = ..., end_time_unix_nano: int | None = ..., ok: bool = ..., error: str | None = ..., attributes: Mapping[str, AttributeValue] | None = ...) -> TelemetryResult: ...
    def metric(self, name: str, kind: MetricKind, value: float, *, unit: str | None = ..., description: str | None = ..., attributes: Mapping[str, AttributeValue] | None = ...) -> TelemetryResult: ...
