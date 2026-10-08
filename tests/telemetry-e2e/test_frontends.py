"""Installed ``sc-otel`` and installed Python wheel against the official Collector.

Each frontend sends a log, a span and metrics of every instrument kind; the
Collector's exported files must contain the sent values.  A refused endpoint
must surface as an export failure in both frontends.
"""
from __future__ import annotations

import json
import time
from pathlib import Path
from typing import Any, Callable

import pytest

from conftest import OfficialCollector, RefusingListener, attributes, run_cli, run_python

TRACE_ID = "4bf92f3577b34da6a3ce929d0e0e4736"
SPAN_ID = "00f067aa0ba902b7"
PARENT_ID = "b7ad6b7169203331"
START_NANOS = 1_700_000_000_000_000_000
END_NANOS = 1_700_000_005_000_000_000
# Wall-clock sanity window for a timestamp the frontend takes itself: the same
# machine's clock, so a generous bound only rejects an unset or epoch value.
CLOCK_SLACK_NANOS = 120 * 1_000_000_000

LOG_ATTRIBUTES = {"job": "build", "attempt": 2, "retry": True, "ratio": 0.25}
SPAN_ATTRIBUTES = {"region": "west", "shard": 7}
METRIC_KINDS = ("counter", "up_down_counter", "gauge", "histogram")
METRIC_VALUE = {"counter": 2.5, "up_down_counter": -1.5, "gauge": 42.0, "histogram": 12.5}
METRIC_ATTRIBUTES = {"queue": "default", "depth": 3}

SCOPES = {"cli": "sc-otel", "python": "sc_observability"}


class Frontend:
    """Sends the shared cases through one installed frontend."""

    def __init__(self, name: str, artifacts: dict[str, Path], cwd: Path, endpoint: str, service: str) -> None:
        self.name, self.artifacts, self.cwd, self.endpoint, self.service = name, artifacts, cwd, endpoint, service

    def python(self, operation: str, args: list[Any], kwargs: dict[str, Any], timeout_s: float = 10) -> dict[str, Any]:
        return run_python(self.artifacts, self.cwd, {
            "endpoint": self.endpoint, "service": self.service, "timeout_s": timeout_s,
            "operation": operation, "args": args, "kwargs": kwargs,
        })

    def cli(self, *args: str, timeout: int = 10) -> int:
        completed = run_cli(
            self.artifacts, "--endpoint", self.endpoint, "--service", self.service, "--timeout", str(timeout),
            *args, cwd=self.cwd,
        )
        if completed.returncode:
            print(completed.stderr)
        return completed.returncode

    def log(self, **extra: Any) -> bool | str:
        """Returns True on success, else the failure kind (``export`` or the CLI exit code)."""
        if self.name == "python":
            result = self.python("log", ["job failed"], {
                "severity": "error", "trace_id": TRACE_ID, "span_id": SPAN_ID,
                "attributes": LOG_ATTRIBUTES, **extra})
            return True if result["ok"] else result["kind"]
        code = self.cli("log", "--body", "job failed", "--severity", "error", "--trace-id", TRACE_ID,
                         "--span-id", SPAN_ID, "--attributes", json.dumps(LOG_ATTRIBUTES))
        return True if code == 0 else str(code)

    def span(self) -> bool | str:
        if self.name == "python":
            result = self.python("span", ["deploy"], {
                "trace_id": TRACE_ID, "span_id": SPAN_ID, "parent_span_id": PARENT_ID, "kind": "client",
                "start_time_unix_nano": START_NANOS, "end_time_unix_nano": END_NANOS,
                "error": "rollout timed out", "attributes": SPAN_ATTRIBUTES})
            return True if result["ok"] else result["kind"]
        code = self.cli("span", "--name", "deploy", "--trace-id", TRACE_ID, "--span-id", SPAN_ID,
                         "--parent-span-id", PARENT_ID, "--kind", "client",
                         "--start-time-unix-nano", str(START_NANOS), "--end-time-unix-nano", str(END_NANOS),
                         "--error", "rollout timed out", "--attributes", json.dumps(SPAN_ATTRIBUTES))
        return True if code == 0 else str(code)

    def metric(self, kind: str) -> bool | str:
        name = f"e2e.{kind}"
        if self.name == "python":
            result = self.python("metric", [name, kind, METRIC_VALUE[kind]], {
                "unit": "s", "description": f"{kind} description", "attributes": METRIC_ATTRIBUTES})
            return True if result["ok"] else result["kind"]
        code = self.cli("metric", "--name", name, "--kind", kind.replace("_", "-"),
                         "--value", str(METRIC_VALUE[kind]), "--unit", "s",
                         "--description", f"{kind} description", "--attributes", json.dumps(METRIC_ATTRIBUTES))
        return True if code == 0 else str(code)


@pytest.fixture(params=("cli", "python"))
def frontend_factory(request: pytest.FixtureRequest, installed_artifacts: dict[str, Path],
                     tmp_path: Path) -> Callable[[str], Frontend]:
    def make(endpoint: str) -> Frontend:
        return Frontend(request.param, installed_artifacts, tmp_path, endpoint, f"e2e-{request.param}")
    return make


def test_frontend_log_reaches_the_collector(collector: OfficialCollector,
                                            frontend_factory: Callable[[str], Frontend]) -> None:
    frontend = frontend_factory(collector.endpoint)
    before = time.time_ns()
    assert frontend.log() is True
    after = time.time_ns()
    collector.wait_for_service(frontend.service, logs=1)
    records = collector.for_service(frontend.service)["logs"]
    assert len(records) == 1
    entry = records[0]
    record = entry["record"]
    assert entry["resource"]["service.name"] == frontend.service
    assert entry["scope"]["name"] == SCOPES[frontend.name]
    assert record["body"] == {"stringValue": "job failed"}
    assert record["severityText"] == "ERROR"
    assert record["severityNumber"] == 17  # OTLP SEVERITY_NUMBER_ERROR
    assert record["traceId"] == TRACE_ID
    assert record["spanId"] == SPAN_ID
    assert attributes(record["attributes"]) == LOG_ATTRIBUTES
    assert before - CLOCK_SLACK_NANOS <= int(record["timeUnixNano"]) <= after + CLOCK_SLACK_NANOS


def test_frontend_span_reaches_the_collector(collector: OfficialCollector,
                                             frontend_factory: Callable[[str], Frontend]) -> None:
    frontend = frontend_factory(collector.endpoint)
    assert frontend.span() is True
    collector.wait_for_service(frontend.service, spans=1)
    spans = collector.for_service(frontend.service)["spans"]
    assert len(spans) == 1
    entry = spans[0]
    span = entry["span"]
    assert entry["resource"]["service.name"] == frontend.service
    assert entry["scope"]["name"] == SCOPES[frontend.name]
    assert span["name"] == "deploy"
    assert span["kind"] == 3  # OTLP SPAN_KIND_CLIENT
    assert (span["traceId"], span["spanId"], span["parentSpanId"]) == (TRACE_ID, SPAN_ID, PARENT_ID)
    assert (int(span["startTimeUnixNano"]), int(span["endTimeUnixNano"])) == (START_NANOS, END_NANOS)
    assert span["status"] == {"code": 2, "message": "rollout timed out"}  # STATUS_CODE_ERROR
    assert attributes(span["attributes"]) == SPAN_ATTRIBUTES


@pytest.mark.parametrize("kind", METRIC_KINDS)
def test_frontend_metric_reaches_the_collector(collector: OfficialCollector,
                                               frontend_factory: Callable[[str], Frontend], kind: str) -> None:
    frontend = frontend_factory(collector.endpoint)
    assert frontend.metric(kind) is True
    collector.wait_for_service(frontend.service, metrics=1)
    metrics = collector.for_service(frontend.service)["metrics"]
    assert len(metrics) == 1
    entry = metrics[0]
    metric = entry["metric"]
    assert entry["resource"]["service.name"] == frontend.service
    assert entry["scope"]["name"] == SCOPES[frontend.name]
    assert (metric["name"], metric["unit"], metric["description"]) == (
        f"e2e.{kind}", "s", f"{kind} description")
    if kind == "gauge":
        (point,) = metric["gauge"]["dataPoints"]
        assert point["asDouble"] == METRIC_VALUE[kind]
    elif kind == "histogram":
        (point,) = metric["histogram"]["dataPoints"]
        assert (int(point["count"]), point["sum"]) == (1, METRIC_VALUE[kind])
    else:
        (point,) = metric["sum"]["dataPoints"]
        assert point["asDouble"] == METRIC_VALUE[kind]
        # The JSON encoding omits a proto3 default, so an absent flag is false.
        assert metric["sum"].get("isMonotonic", False) is (kind == "counter")
    assert attributes(point["attributes"]) == METRIC_ATTRIBUTES


def test_unavailable_endpoint_fails_in_both_frontends(
    refusing_endpoint: RefusingListener, installed_artifacts: dict[str, Path], tmp_path: Path,
) -> None:
    outcomes = {
        name: Frontend(name, installed_artifacts, tmp_path, refusing_endpoint.endpoint, f"e2e-down-{name}")
        for name in ("cli", "python")
    }
    # CLI exit 7 is an export failure; 3 would be a validation failure.  The
    # Python call reports an ``Err`` whose failure kind is ``unavailable``.
    assert outcomes["cli"].cli("log", "--body", "never delivered", timeout=1) == 7
    refused = outcomes["python"].python("log", ["never delivered"], {}, timeout_s=1)
    assert refused == {"ok": False, "kind": "unavailable", "code": refused["code"]}
    assert refused["code"] == "SC_OBSERVABILITY_OTLP_EXPORT_FAILED"
    assert refusing_endpoint.accepted >= 2, "both frontends must have reached the listener and been cut off"
