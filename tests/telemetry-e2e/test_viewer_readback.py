"""Pinned desktop-viewer readback through the installed CLI and installed Python wheel.

Both frontends send a correlated log, span and one metric of each viewer-supported
form (gauge, sum, histogram) to the pinned viewer's OTLP/HTTP port; the viewer's
RPC API must return the sent values.  This is the viewer coverage for the
synchronous client.  It runs only with the pinned viewer provided.
"""
from __future__ import annotations

import json
import time
from pathlib import Path
from typing import Any, Callable

import pytest

from conftest import rpc, run_cli, run_python


_TRACE_IDS = {
    "python": "0123456789abcdef0123456789abcdef",
    "cli": "fedcba9876543210fedcba9876543210",
}
_SPAN_IDS = {"python": "0123456789abcdef", "cli": "fedcba9876543210"}
_START, _END = 1_700_000_000_000_000_000, 1_700_000_002_000_000_000
_LOW, _HIGH = "-1000000000", "1893456000000000000"
_METRICS = (  # (kind, value, viewer metricType)
    ("gauge", 5.5, "Gauge"),
    ("counter", 7.25, "Sum"),
    ("histogram", 20.0, "Histogram"),
)


def _wait_for(
    viewer: dict[str, str], method: str, params: list[object], predicate: Callable[[object], bool],
) -> object:
    """Poll only for ingestion; the harness has already proven the RPC endpoint ready.

    An RPC failure is a real failure and propagates at once rather than being retried.
    """
    deadline = time.monotonic() + 15
    last: object = None
    while time.monotonic() < deadline:
        last = rpc(viewer["rpc"], method, params)
        if predicate(last):
            return last
        time.sleep(0.25)
    raise AssertionError(f"viewer ingestion via {method} timed out; last result: {last!r}")


def _attributes(subject: object, expected: dict[str, object], context: str) -> None:
    assert isinstance(subject, dict), f"{context} was not an object: {subject!r}"
    attributes = subject.get("attributes")
    assert isinstance(attributes, list), f"{context} omitted decoded attributes: {subject!r}"
    actual = {
        attribute.get("key"): attribute.get("value")
        for attribute in attributes
        if isinstance(attribute, dict)
    }
    for key, value in expected.items():
        assert actual.get(key) == value, f"{context} {key} was {actual.get(key)!r}, expected {value!r}"


def _row(rows: object, expected_name: str) -> dict[str, object] | None:
    if not isinstance(rows, list):
        return None
    return next((row for row in rows if isinstance(row, dict)
                 and row.get("name", row.get("bodyPreview")) == expected_name
                 and isinstance(row.get("id") or row.get("streamID") or row.get("streamId"), str)), None)


def _send(artifacts: dict[str, Path], frontend: str, endpoint: str, cwd: Path) -> None:
    """Send the log, the span and the three metrics through one installed frontend."""
    service = f"telemetry-e2e-viewer-{frontend}"
    trace_id, span_id = _TRACE_IDS[frontend], _SPAN_IDS[frontend]
    attrs = {"test.frontend": frontend}
    metric_args = [(f"viewer.{kind}.{frontend}", kind, value) for kind, value, _ in _METRICS]
    if frontend == "python":
        def call(operation: str, args: list[Any], kwargs: dict[str, Any]) -> None:
            result = run_python(artifacts, cwd, {"endpoint": endpoint, "service": service, "timeout_s": 10,
                                                 "operation": operation, "args": args, "kwargs": kwargs})
            assert result["ok"], result
        call("log", [f"viewer-log-{frontend}"], {"trace_id": trace_id, "span_id": span_id, "attributes": attrs})
        call("span", [f"viewer-span-{frontend}"], {
            "trace_id": trace_id, "span_id": span_id, "start_time_unix_nano": _START,
            "end_time_unix_nano": _END, "attributes": attrs})
        for name, kind, value in metric_args:
            call("metric", [name, kind, value], {"attributes": attrs})
        return
    base = ["--endpoint", endpoint, "--service", service, "--timeout", "10"]
    commands = [
        ["log", "--body", f"viewer-log-{frontend}", "--trace-id", trace_id, "--span-id", span_id,
         "--attributes", json.dumps(attrs)],
        ["span", "--name", f"viewer-span-{frontend}", "--trace-id", trace_id, "--span-id", span_id,
         "--start-time-unix-nano", str(_START), "--end-time-unix-nano", str(_END),
         "--attributes", json.dumps(attrs)],
        *(["metric", "--name", name, "--kind", kind, "--value", str(value), "--attributes", json.dumps(attrs)]
          for name, kind, value in metric_args),
    ]
    for command in commands:
        done = run_cli(artifacts, *base, *command, cwd=cwd)
        assert done.returncode == 0, done.stdout + done.stderr


def _assert_frontend_readback(viewer: dict[str, str], frontend: str) -> None:
    service = f"telemetry-e2e-viewer-{frontend}"
    trace_id, span_id = _TRACE_IDS[frontend], _SPAN_IDS[frontend]
    body = f"viewer-log-{frontend}"
    span_name = f"viewer-span-{frontend}"
    logs = _wait_for(viewer, "searchLogs", [_LOW, _HIGH], lambda result: _row(result, body) is not None)
    log_row = _row(logs, body)
    assert log_row is not None
    detail = _wait_for(viewer, "getLog", [log_row["id"]],
                       lambda result: isinstance(result, dict) and result.get("body") == body)
    assert isinstance(detail, dict)
    assert detail["body"] == body
    assert int(detail["timestamp"]) > 0
    assert detail["traceID"] == trace_id
    assert detail["spanID"] == span_id
    _attributes(detail["resource"], {"service.name": service}, "log resource")
    _attributes(detail, {"test.frontend": frontend}, "log")

    spans = _wait_for(
        viewer, "searchSpans", [trace_id],
        lambda result: isinstance(result, dict) and any(
            isinstance(row, dict) and isinstance(row.get("spanData"), dict)
            and row["spanData"].get("name") == span_name for row in result.get("spans", [])
        ),
    )
    assert isinstance(spans, dict)
    assert spans["traceID"] == trace_id
    assert spans["traceStart"] == str(_START)
    span = next(row["spanData"] for row in spans["spans"]
                if row.get("spanData", {}).get("name") == span_name)
    assert span["spanID"] == span_id
    assert span["start"] == 0
    assert span["dur"] == _END - _START
    _attributes(spans["resources"][str(span["r"])],
                {"service.name": service}, "span resource")
    _attributes(span, {"test.frontend": frontend}, "span")

    expected = {
        f"viewer.gauge.{frontend}": ("Gauge", {"doubleValue": 5.5}),
        f"viewer.counter.{frontend}": ("Sum", {"doubleValue": 7.25}),
        f"viewer.histogram.{frontend}": ("Histogram", {"count": 1, "sum": 20.0}),
    }
    summaries = _wait_for(
        viewer, "searchMetricSummaries", [_LOW, _HIGH],
        lambda result: isinstance(result, list) and all(_row(result, name) is not None for name in expected),
    )
    assert isinstance(summaries, list)
    for name, (metric_type, fields) in expected.items():
        summary = _row(summaries, name)
        assert summary is not None
        assert summary["metricType"] == metric_type
        stream = summary.get("id") or summary.get("streamID") or summary.get("streamId")
        assert isinstance(stream, str), f"metric summary {name!r} omitted its stream id: {summary!r}"
        metric = _wait_for(
            viewer, "getMetric", [stream, _LOW, _HIGH],
            lambda result: isinstance(result, dict) and bool(result.get("timeseries")),
        )
        assert isinstance(metric, dict)
        point = metric["timeseries"][0]["datapoints"][0]
        assert int(point["timestamp"]) > 0
        for field, value in fields.items():
            assert point[field] == value, f"{name} {field} was {point[field]!r}, expected {value!r}"


@pytest.mark.parametrize("frontend", ("python", "cli"))
def test_installed_frontends_read_back_supported_viewer_signal_forms(
    installed_artifacts: dict[str, Path], pinned_viewer: dict[str, str], tmp_path: Path, frontend: str,
) -> None:
    """Viewer reads logs, spans, Gauge, Sum and explicit Histogram sent by the installed frontend."""
    _send(installed_artifacts, frontend, pinned_viewer["otlp"], tmp_path)
    _assert_frontend_readback(pinned_viewer, frontend)
