"""Pinned desktop-viewer readback through installed telemetry front ends."""
from __future__ import annotations

import json
import time
from pathlib import Path
from typing import Any, Callable

from conftest import GOLDENS, rpc, run_cli, run_installed_python


_TIME = "1970-01-01T00:00:01.000000000Z"
_TIME_NANOS = "1000000000"
_TRACE_IDS = {
    "python": "0123456789abcdef0123456789abcdef",
    "cli": "fedcba9876543210fedcba9876543210",
}
_SPAN_IDS = {"python": "0123456789abcdef", "cli": "fedcba9876543210"}
_LOW, _HIGH = "-1000000000", "1893456000000000000"


def _viewer_config(path: Path, endpoint: str, store_name: str, service: str) -> Path:
    path.write_text(
        "\n".join((f"service: {service}", "otlp:", f"  endpoint: {endpoint}",
                   "  timeout_ms: 1000", "store:", f"  path: {store_name}",
                   "  max_bytes: 10485760", "")), encoding="utf-8")
    return path


def _wait_for(
    viewer: dict[str, str], method: str, params: list[object], predicate: Callable[[object], bool],
) -> object:
    """Wait for viewer ingestion while retaining the last useful response."""
    deadline = time.monotonic() + 15
    last: object = None
    while time.monotonic() < deadline:
        try:
            result = rpc(viewer["rpc"], method, params)
            last = result
            if predicate(result):
                return result
        except (AssertionError, OSError, TimeoutError, ValueError) as error:
            # The viewer may briefly reject RPCs while it starts its ingest loop.
            last = f"{type(error).__name__}: {error}"
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


def _payload(frontend: str) -> dict[str, object]:
    """Use validated golden shapes, with one explicit correlated viewer record set."""
    def golden(name: str) -> dict[str, Any]:
        return json.loads((GOLDENS / name / "input.json").read_text())

    service = f"telemetry-e2e-viewer-{frontend}"
    trace_id, span_id = _TRACE_IDS[frontend], _SPAN_IDS[frontend]
    payload = golden("logs")
    payload["resource"] = {
        "attributes": {"service.name": service, "test.frontend": frontend},
        "dropped_attributes_count": 0,
        "entity_refs": [],
        "schema_url": None,
    }
    payload["record_key"] = f"viewer-readback-{frontend}"
    payload["logs"][0].update({
        "body": f"viewer-log-{frontend}", "time": _TIME, "observed_time": _TIME,
        "attributes": {"test.frontend": frontend}, "trace_id": trace_id, "span_id": span_id,
        "correlation_id": f"viewer-{frontend}",
    })
    span = golden("traces")["spans"][0]
    span.update({
        "name": f"viewer-span-{frontend}", "trace_id": trace_id, "span_id": span_id,
        "start_time": _TIME,
        "attributes": {"test.frontend": frontend}, "correlation_id": f"viewer-{frontend}",
    })
    payload["spans"] = [span]
    metrics = []
    for fixture, name, value in (
        ("metric_gauge", f"viewer.gauge.{frontend}", 5.5),
        ("metric_sum", f"viewer.sum.{frontend}", 7.25),
        ("metric_histogram", f"viewer.histogram.{frontend}", None),
    ):
        metric = golden(fixture)["metrics"][0]
        metric["name"] = name
        point = metric["data"]["data"]["points"][0]
        point["attributes"] = {"test.frontend": frontend}
        if value is not None:
            point["value"] = {"kind": "double", "data": value}
        else:
            point.update({"sum": 20.0, "count": 3, "bucket_counts": [1, 2], "explicit_bounds": [5.0]})
        metrics.append(metric)
    payload["metrics"] = metrics
    return payload


def _python_emit_script(config: Path) -> str:
    return f"""\
import json
import sys
from sc_observability import Ok
from sc_observability.telemetry import Telemetry

opened = Telemetry.open(config={str(config)!r})
assert isinstance(opened, Ok), opened
with opened.value as telemetry:
    submitted = telemetry.emit(json.load(sys.stdin))
    assert isinstance(submitted, Ok), submitted
    delivered = telemetry.flush_submission(submitted.value.submission_id, timeout_s=10)
    assert isinstance(delivered, Ok), delivered
"""


def _assert_frontend_readback(viewer: dict[str, str], frontend: str) -> None:
    service = f"telemetry-e2e-viewer-{frontend}"
    trace_id, span_id = _TRACE_IDS[frontend], _SPAN_IDS[frontend]
    body = f"viewer-log-{frontend}"
    span_name = f"viewer-span-{frontend}"
    logs = _wait_for(viewer, "searchLogs", [_LOW, _HIGH], lambda result: _row(result, body) is not None)
    log_row = _row(logs, body)
    assert log_row is not None
    log_id = log_row["id"]
    detail = _wait_for(viewer, "getLog", [log_id], lambda result: isinstance(result, dict) and result.get("body") == body)
    assert isinstance(detail, dict)
    assert detail["body"] == body
    assert detail["timestamp"] == _TIME_NANOS
    assert detail["observedTimestamp"] == _TIME_NANOS
    assert detail["traceID"] == trace_id
    assert detail["spanID"] == span_id
    _attributes(detail["resource"], {"service.name": service, "test.frontend": frontend}, "log resource")
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
    assert spans["traceStart"] == _TIME_NANOS
    span = next(row["spanData"] for row in spans["spans"]
                if row.get("spanData", {}).get("name") == span_name)
    assert span["spanID"] == span_id
    assert span["start"] == 0
    assert span["dur"] == 1_000_000_000
    assert int(spans["traceStart"]) + span["start"] + span["dur"] == 2_000_000_000
    _attributes(spans["resources"][str(span["r"])],
                {"service.name": service, "test.frontend": frontend}, "span resource")
    _attributes(span, {"test.frontend": frontend}, "span")

    expected = {
        f"viewer.gauge.{frontend}": ("Gauge", {"doubleValue": 5.5}),
        f"viewer.sum.{frontend}": ("Sum", {"doubleValue": 7.25}),
        f"viewer.histogram.{frontend}": (
            "Histogram", {"count": 3, "sum": 20.0, "explicitBounds": [5.0], "bucketCounts": [1, 2]},
        ),
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
        assert point["timestamp"] == _TIME_NANOS
        for field, value in fields.items():
            assert point[field] == value, f"{name} {field} was {point[field]!r}, expected {value!r}"


def test_installed_frontends_read_back_supported_viewer_signal_forms(
    installed_artifacts: dict[str, Path], pinned_viewer: dict[str, str], tmp_path: Path,
) -> None:
    """Viewer v0.5.0 reads logs, spans, Gauge, Sum, and explicit Histogram."""
    for frontend in ("python", "cli"):
        payload = json.dumps(_payload(frontend))
        service = f"telemetry-e2e-viewer-{frontend}"
        config = _viewer_config(tmp_path / f"{frontend}-viewer.yaml", pinned_viewer["otlp"],
                                f"viewer-{frontend}.sqlite", service)
        if frontend == "python":
            emitted = run_installed_python(installed_artifacts, _python_emit_script(config), cwd=tmp_path, input=payload)
        else:
            emitted = run_cli(installed_artifacts, "--config", str(config), "emit", "--stdin",
                              cwd=tmp_path, input=payload)
        assert emitted.returncode == 0, emitted.stdout + emitted.stderr
        _assert_frontend_readback(pinned_viewer, frontend)
