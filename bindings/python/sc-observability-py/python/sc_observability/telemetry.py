"""OpenTelemetry export over OTLP/HTTP through the shared native synchronous client.

Each call builds one native client, exports one record and returns after the
exporter reports its result; nothing is stored or retried after the call.
"""
from __future__ import annotations

import importlib
from os import PathLike, fspath
from typing import Any, Literal, Mapping, TypeAlias

from . import Err, Result, _decode_control, _foreign_message, _internal

AttributeValue: TypeAlias = str | bool | int | float
Severity: TypeAlias = Literal["trace", "debug", "info", "warn", "error", "fatal"]
SpanKind: TypeAlias = Literal["internal", "server", "client", "producer", "consumer"]
MetricKind: TypeAlias = Literal["counter", "up_down_counter", "gauge", "histogram"]


def _items(name: str, mapping: Mapping[str, Any] | None) -> list[tuple[str, Any]]:
    if mapping is None:
        return []
    if not isinstance(mapping, Mapping):
        raise TypeError(f"{name} must be a Mapping")
    return list(mapping.items())


def _attributes(attributes: Mapping[str, AttributeValue] | None) -> list[tuple[str, AttributeValue]]:
    return _items("attributes", attributes)


class Telemetry:
    """Export settings for OpenTelemetry logs, spans and metrics.

    ``endpoint`` is the OTLP/HTTP base URL; it defaults to
    ``OTEL_EXPORTER_OTLP_ENDPOINT``, then ``http://localhost:4318``. Explicit
    ``headers`` replace same-named ``OTEL_EXPORTER_OTLP_HEADERS`` entries.
    ``timeout_s`` bounds connecting, each request and the exporter's retries
    (default 3 seconds). ``service_name`` defaults to ``OTEL_SERVICE_NAME``.
    Blocking export releases the GIL.

    Each call returns ``Ok(None)`` or ``Err`` with a shared ``Failure``:
    ``validation`` for rejected input, ``timeout`` when the exporter deadline
    passed, ``unavailable`` for other export failures, ``internal`` for
    binding failures. Codes come from the OTLP error registry.
    """

    def __init__(self, endpoint: str | None = None, *, headers: Mapping[str, str] | None = None,
                 timeout_s: float | None = None, root_certificate: str | PathLike[str] | None = None,
                 service_name: str | None = None) -> None:
        self._config = {
            "endpoint": endpoint,
            "headers": _items("headers", headers),
            "timeout_s": timeout_s,
            "root_certificate": None if root_certificate is None else fspath(root_certificate),
            "service_name": service_name,
        }

    def log(self, body: str, *, severity: Severity = "info", trace_id: str | None = None,
            span_id: str | None = None,
            attributes: Mapping[str, AttributeValue] | None = None) -> Result[None]:
        """Export one log record; ``trace_id`` and ``span_id`` go together."""
        return self._send("send_log", {
            "body": body, "severity": severity, "trace_id": trace_id, "span_id": span_id,
            "attributes": _attributes(attributes),
        })

    def span(self, name: str, *, trace_id: str | None = None, span_id: str | None = None,
             parent_span_id: str | None = None, kind: SpanKind = "internal",
             start_time_unix_nano: int | None = None, end_time_unix_nano: int | None = None,
             ok: bool = False, error: str | None = None,
             attributes: Mapping[str, AttributeValue] | None = None) -> Result[None]:
        """Export one completed span; missing ids are random and times default to now."""
        return self._send("send_span", {
            "name": name, "trace_id": trace_id, "span_id": span_id, "parent_span_id": parent_span_id,
            "kind": kind, "start_time_unix_nano": start_time_unix_nano,
            "end_time_unix_nano": end_time_unix_nano, "ok": ok, "error": error,
            "attributes": _attributes(attributes),
        })

    def metric(self, name: str, kind: MetricKind, value: float, *, unit: str | None = None,
               description: str | None = None,
               attributes: Mapping[str, AttributeValue] | None = None) -> Result[None]:
        """Export one measurement.

        Counter measurements are expected to be non-negative increments. This
        wrapper does not reject a negative value: the current SDK records and
        exports it. Use ``up_down_counter`` for signed changes. Histograms may
        contain negative observations.
        """
        return self._send("send_metric", {
            "name": name, "kind": kind, "value": value, "unit": unit, "description": description,
            "attributes": _attributes(attributes),
        })

    def _send(self, function: str, fields: Mapping[str, Any]) -> Result[None]:
        try:
            send = getattr(importlib.import_module("sc_observability._native"), function)
        except Exception as error:
            return Err(_internal(f"native extension unavailable: {_foreign_message(error)}"))
        return _decode_control(send(self._config, fields))
