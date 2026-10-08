"""Rust native paths against the official Collector, through the example binary.

``otlp-native-example`` runs one scenario per process: the synchronous client,
the Tokio path built from the official SDK providers and OTLP exporters, and
the three core-logger compositions (OTel only, file and OTel, file only).
"""
from __future__ import annotations

import json
from pathlib import Path

import pytest

from conftest import PROCESS_TIMEOUT_SECONDS, OfficialCollector, attributes, run_process

SECRET = "hunter2-secret"
MACRO_MESSAGE = "compose macro event"
BRIDGE_MESSAGE = "compose bridge event"
COMPOSE_SERVICE = "e2e-compose"
BARRIER_SERVICE = "e2e-sync-client"


def run_example(example: Path, *args: str) -> None:
    completed = run_process([str(example), *args], timeout=PROCESS_TIMEOUT_SECONDS,
                            text=True, capture_output=True, check=False)
    assert completed.returncode == 0, completed.stdout + completed.stderr


def test_sync_client_exports_all_signals(native_example: Path, collector: OfficialCollector) -> None:
    run_example(native_example, "sync-client", collector.endpoint)
    collector.wait_for_service("e2e-sync-client", logs=1, spans=1, metrics=1)
    exported = collector.for_service("e2e-sync-client")
    (log,), (span,), (metric,) = exported["logs"], exported["spans"], exported["metrics"]

    assert log["scope"]["name"] == "e2e.native" and log["scope"]["version"] == "1.2.3"
    assert log["record"]["body"] == {"stringValue": "sync client log"}
    assert log["record"]["severityText"] == "WARN"
    assert log["record"]["timeUnixNano"] == "1700000000000000000"
    assert (log["record"]["traceId"], log["record"]["spanId"]) == (
        "4bf92f3577b34da6a3ce929d0e0e4736", "00f067aa0ba902b7")
    assert attributes(log["record"]["attributes"]) == {"job": "build", "attempt": 2}

    assert span["scope"]["name"] == "e2e.native"
    assert span["span"]["name"] == "sync client span"
    assert span["span"]["kind"] == 3  # OTLP SPAN_KIND_CLIENT
    assert (span["span"]["traceId"], span["span"]["spanId"]) == (
        "4bf92f3577b34da6a3ce929d0e0e4736", "00f067aa0ba902b7")
    assert span["span"]["startTimeUnixNano"] == "1700000000000000000"
    assert span["span"]["endTimeUnixNano"] == "1700000005000000000"
    assert span["span"]["status"] == {"code": 1}  # STATUS_CODE_OK
    assert attributes(span["span"]["attributes"]) == {"region": "west"}

    assert metric["scope"]["name"] == "e2e.native"
    assert (metric["metric"]["name"], metric["metric"]["unit"]) == ("sync.client.jobs", "1")
    (point,) = metric["metric"]["sum"]["dataPoints"]
    assert point["asDouble"] == 3.0
    assert attributes(point["attributes"]) == {"queue": "default"}


def test_tokio_path_exports_all_signals(native_example: Path, collector: OfficialCollector) -> None:
    run_example(native_example, "tokio", collector.endpoint)
    collector.wait_for_service("e2e-tokio", logs=1, spans=1, metrics=1)
    exported = collector.for_service("e2e-tokio")
    (log,), (span,), (metric,) = exported["logs"], exported["spans"], exported["metrics"]

    assert log["scope"]["name"] == span["scope"]["name"] == metric["scope"]["name"] == "e2e.native"
    assert log["record"]["body"] == {"stringValue": "tokio log"}
    assert log["record"]["severityText"] == "ERROR"
    assert attributes(log["record"]["attributes"]) == {"job": "deploy"}
    assert span["span"]["name"] == "tokio span"
    assert span["span"]["kind"] == 2  # OTLP SPAN_KIND_SERVER
    assert attributes(span["span"]["attributes"]) == {"region": "east"}
    # The log was emitted inside the span's context, so it is correlated to it.
    assert (log["record"]["traceId"], log["record"]["spanId"]) == (span["span"]["traceId"], span["span"]["spanId"])
    assert len(span["span"]["traceId"]) == 32 and span["span"]["traceId"] != "0" * 32
    assert metric["metric"]["name"] == "tokio.jobs"
    (point,) = metric["metric"]["sum"]["dataPoints"]
    assert point["asDouble"] == 5.0
    assert attributes(point["attributes"]) == {"queue": "tokio"}


def read_file_events(root: Path) -> list[dict[str, object]]:
    path = root / "logs" / f"{COMPOSE_SERVICE}.log.jsonl"
    return [json.loads(line) for line in path.read_text(encoding="utf-8").splitlines() if line]


def compose_exports(native_example: Path, collector: OfficialCollector, mode: str, root: Path) -> list[dict[str, object]]:
    """Run one composition, then prove with a barrier that the Collector has seen everything.

    The composition process has exited, so every export it made was
    acknowledged before exit.  A later request through the same receiver and
    file exporter is written after them; once the barrier is visible, whatever
    the composition exported is visible too, and what is absent was never sent.
    """
    run_example(native_example, f"compose-{mode}", collector.endpoint, str(root))
    run_example(native_example, "sync-client", collector.endpoint)
    collector.wait_for_service(BARRIER_SERVICE, logs=1)
    return collector.for_service(COMPOSE_SERVICE)["logs"]


def assert_no_secret(collector: OfficialCollector, root: Path) -> None:
    for path in [*collector.files.values(), *root.rglob("*")]:
        if path.is_file():
            assert SECRET not in path.read_text(encoding="utf-8"), f"redacted value leaked into {path}"


@pytest.mark.parametrize("mode", ("otel-only", "both", "file-only"))
def test_logger_composition_routes_each_event_once(
    native_example: Path, collector: OfficialCollector, tmp_path: Path, mode: str,
) -> None:
    root = tmp_path / "log-root"
    exported = compose_exports(native_example, collector, mode, root)
    want_file = mode in {"both", "file-only"}
    want_otel = mode in {"both", "otel-only"}

    if want_file:
        file_events = read_file_events(root)
        # One line per event: the macro event and the `log` facade event, no duplicates.
        assert sorted(str(event["message"]) for event in file_events) == [BRIDGE_MESSAGE, MACRO_MESSAGE]
        macro = next(event for event in file_events if event["message"] == MACRO_MESSAGE)
        assert macro["fields"]["marker"] == "macro-event"
        assert macro["fields"]["password"] == "[REDACTED]"
    else:
        assert not root.exists() or not any(path.is_file() for path in root.rglob("*")), \
            f"OTel-only composition created files under {root}"

    if want_otel:
        assert sorted(entry["record"]["body"]["stringValue"] for entry in exported) == [BRIDGE_MESSAGE, MACRO_MESSAGE]
        macro_record = next(e for e in exported if e["record"]["body"]["stringValue"] == MACRO_MESSAGE)
        # The sink maps the event target to the OTLP scope name.
        assert macro_record["scope"]["name"] == "e2e.compose"
        assert macro_record["resource"]["service.name"] == COMPOSE_SERVICE
        assert macro_record["record"]["severityText"] == "INFO"
        fields = attributes(macro_record["record"]["attributes"])
        assert fields["marker"] == "macro-event"
        assert fields["password"] == "[REDACTED]"
    else:
        assert exported == [], "file-only composition must create no OTel export"
        assert collector.for_service(COMPOSE_SERVICE) == {"logs": [], "spans": [], "metrics": []}

    assert_no_secret(collector, root)
