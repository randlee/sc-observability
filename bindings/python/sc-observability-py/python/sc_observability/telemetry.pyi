from os import PathLike
from typing import Literal, Mapping, TypeAlias
from . import Result

AttributeValue: TypeAlias = str | bool | int | float
Severity: TypeAlias = Literal['trace', 'debug', 'info', 'warn', 'error', 'fatal']
SpanKind: TypeAlias = Literal['internal', 'server', 'client', 'producer', 'consumer']
MetricKind: TypeAlias = Literal['counter', 'up_down_counter', 'gauge', 'histogram']

class Telemetry:
    def __init__(self, endpoint: str | None = ..., *, headers: Mapping[str, str] | None = ..., timeout_s: float | None = ..., root_certificate: str | PathLike[str] | None = ..., service_name: str | None = ...) -> None: ...
    def log(self, body: str, *, severity: Severity = ..., trace_id: str | None = ..., span_id: str | None = ..., attributes: Mapping[str, AttributeValue] | None = ...) -> Result[None]: ...
    def span(self, name: str, *, trace_id: str | None = ..., span_id: str | None = ..., parent_span_id: str | None = ..., kind: SpanKind = ..., start_time_unix_nano: int | None = ..., end_time_unix_nano: int | None = ..., ok: bool = ..., error: str | None = ..., attributes: Mapping[str, AttributeValue] | None = ...) -> Result[None]: ...
    def metric(self, name: str, kind: MetricKind, value: float, *, unit: str | None = ..., description: str | None = ..., attributes: Mapping[str, AttributeValue] | None = ...) -> Result[None]:
        """Export one measurement.

        The SDK ignores negative counter measurements; if no valid measurement
        is recorded, this returns a tagged validation failure. Histograms may
        contain negative values.
        """
