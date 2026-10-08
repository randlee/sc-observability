"""Installed-wheel export through the native synchronous OTLP client.

A loopback HTTP receiver records each request so field forwarding, header
precedence and failure projection are checked against the bytes on the wire.
"""
from __future__ import annotations

import faulthandler
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import os
from pathlib import Path
import socket
import struct
import threading
from typing import Any, Iterator
from unittest.mock import patch

import pytest

pytestmark = pytest.mark.skipif(
    os.environ.get("SC_OBSERVABILITY_RUNTIME_TEST") != "1",
    reason="requires an installed sc-observability wheel",
)

from sc_observability import Err, Ok, Result, Telemetry

TRACE_ID = "4bf92f3577b34da6a3ce929d0e0e4736"
SPAN_ID = "00f067aa0ba902b7"
PARENT_ID = "b7ad6b7169203331"
SECRET = "header-secret-value"


class Collector:
    def __init__(self, status: int = 200) -> None:
        self.requests: list[tuple[str, dict[str, str], bytes]] = []
        requests = self.requests

        class Handler(BaseHTTPRequestHandler):
            def do_POST(self) -> None:
                body = self.rfile.read(int(self.headers.get("content-length", "0")))
                requests.append((self.path, {k.lower(): v for k, v in self.headers.items()}, body))
                self.send_response(status)
                self.send_header("content-length", "0")
                self.end_headers()

            def log_message(self, *_: Any) -> None:
                pass

        self._server = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
        self._server.daemon_threads = True
        self._thread = threading.Thread(target=self._server.serve_forever, args=(0.05,), daemon=True)
        self._thread.start()
        self.endpoint = f"http://127.0.0.1:{self._server.server_address[1]}"

    def close(self) -> None:
        self._server.shutdown()
        self._server.server_close()
        self._thread.join(timeout=10)

    def only(self, path: str) -> tuple[dict[str, str], bytes]:
        assert [request[0] for request in self.requests] == [path]
        _, headers, body = self.requests[0]
        assert headers["content-type"] == "application/x-protobuf"
        return headers, body


@pytest.fixture
def collector(monkeypatch: pytest.MonkeyPatch) -> Iterator[Collector]:
    for name in list(os.environ):
        if name.startswith("OTEL_"):
            monkeypatch.delenv(name)
    receiver = Collector()
    try:
        yield receiver
    finally:
        receiver.close()


@pytest.fixture
def rejecting(monkeypatch: pytest.MonkeyPatch) -> Iterator[Collector]:
    for name in list(os.environ):
        if name.startswith("OTEL_"):
            monkeypatch.delenv(name)
    receiver = Collector(status=400)
    try:
        yield receiver
    finally:
        receiver.close()


def _ok(result: Result[None]) -> None:
    assert isinstance(result, Ok) and result.value is None, result


def _failure(result: Result[None], kind: str, code: str) -> str:
    assert isinstance(result, Err), result
    assert (result.error.kind, result.error.code) == (kind, code), result
    return result.error.message


def test_log_forwards_fields_and_explicit_headers_win(collector: Collector, monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setenv("OTEL_EXPORTER_OTLP_HEADERS", "authorization=from-env,x-env=kept")
    telemetry = Telemetry(collector.endpoint, headers={"authorization": "explicit"}, service_name="py-log-service")
    _ok(telemetry.log("job failed", severity="error", trace_id=TRACE_ID, span_id=SPAN_ID,
                      attributes={"job": "build", "attempt": 2, "retry": True, "ratio": 0.25}))
    headers, body = collector.only("/v1/logs")
    assert (headers["authorization"], headers["x-env"]) == ("explicit", "kept")
    for expected in (b"job failed", b"ERROR", b"py-log-service", b"sc_observability", b"build",
                     b"attempt", bytes.fromhex(TRACE_ID), bytes.fromhex(SPAN_ID), struct.pack("<d", 0.25)):
        assert expected in body, expected


def test_span_forwards_ids_times_status_and_attributes(collector: Collector) -> None:
    _ok(Telemetry(collector.endpoint).span(
        "deploy", trace_id=TRACE_ID, span_id=SPAN_ID, parent_span_id=PARENT_ID, kind="client",
        start_time_unix_nano=1_700_000_000_000_000_000, end_time_unix_nano=1_700_000_005_000_000_000,
        error="rollout timed out", attributes={"region": "west"}))
    _, body = collector.only("/v1/traces")
    for expected in (b"deploy", b"rollout timed out", b"region", b"west", bytes.fromhex(TRACE_ID),
                     bytes.fromhex(SPAN_ID), bytes.fromhex(PARENT_ID),
                     struct.pack("<Q", 1_700_000_000_000_000_000), struct.pack("<Q", 1_700_000_005_000_000_000)):
        assert expected in body, expected


def test_metric_uses_the_environment_endpoint(collector: Collector, monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setenv("OTEL_EXPORTER_OTLP_ENDPOINT", collector.endpoint)
    _ok(Telemetry(service_name="py-metric-service").metric(
        "job.duration", "histogram", 12.5, unit="s", description="job wall time", attributes={"queue": "default"}))
    _, body = collector.only("/v1/metrics")
    for expected in (b"job.duration", b"job wall time", b"py-metric-service", b"queue", b"default",
                     struct.pack("<d", 12.5)):
        assert expected in body, expected


def test_rejected_export_is_tagged_and_redacted(rejecting: Collector) -> None:
    endpoint = rejecting.endpoint.replace("http://", "http://user:userinfo-password@")
    message = _failure(Telemetry(endpoint, headers={"authorization": SECRET}).log("rejected"),
                       "unavailable", "SC_OBSERVABILITY_OTLP_EXPORT_FAILED")
    assert len(rejecting.requests) == 1
    assert SECRET not in message and "userinfo-password" not in message


INVALID_RECORD = "SC_OBSERVABILITY_OTLP_SYNC_INVALID_RECORD"
INVALID_CONFIG = "SC_OBSERVABILITY_OTLP_SYNC_INVALID_CONFIG"
LIMIT = "SC_OBSERVABILITY_OTLP_SYNC_INPUT_LIMIT_EXCEEDED"


@pytest.mark.parametrize("call,code", [
    (lambda t: t.log("x", severity="loud"), INVALID_RECORD),
    (lambda t: t.log("x", trace_id=TRACE_ID), INVALID_RECORD),
    (lambda t: t.log("x", trace_id=TRACE_ID.upper(), span_id=SPAN_ID), INVALID_RECORD),
    (lambda t: t.log("x", attributes={"nested": [1]}), INVALID_RECORD),
    (lambda t: t.log("x", attributes={"missing": None}), INVALID_RECORD),
    (lambda t: t.log("x", attributes={"huge": 2**63}), INVALID_RECORD),
    (lambda t: t.log("x" * (1024 * 1024 + 1)), LIMIT),
    (lambda t: t.log("x", attributes={f"k{i}": i for i in range(10_001)}), LIMIT),
    (lambda t: t.span("x", parent_span_id=PARENT_ID), INVALID_RECORD),
    (lambda t: t.span("x", ok=True, error="boom"), INVALID_RECORD),
    (lambda t: t.span("x", kind="sideways"), INVALID_RECORD),
    (lambda t: t.span("x", span_id="f067aa0ba902b7"), INVALID_RECORD),
    (lambda t: t.span("x", start_time_unix_nano=-1), INVALID_RECORD),
    (lambda t: t.span("x", end_time_unix_nano=2**64), INVALID_RECORD),
    (lambda t: t.span("x", start_time_unix_nano=2, end_time_unix_nano=1), INVALID_RECORD),
    (lambda t: t.metric("x", "summary", 1), INVALID_RECORD),
], ids=lambda value: value if isinstance(value, str) else "")
def test_invalid_fields_are_tagged_and_send_nothing(collector: Collector, call: Any, code: str) -> None:
    _failure(call(Telemetry(collector.endpoint)), "validation", code)
    assert collector.requests == []


@pytest.mark.parametrize("options", [{"timeout_s": 0}, {"timeout_s": float("nan")}, {"timeout_s": -1.0},
                                     {"root_certificate": Path("/nonexistent/sc-observability-ca.pem")},
                                     {"headers": {"bad header": "x"}}])
def test_invalid_configuration_is_tagged_and_sends_nothing(collector: Collector, options: dict[str, Any]) -> None:
    _failure(Telemetry(collector.endpoint, **options).log("x"), "validation", INVALID_CONFIG)
    assert collector.requests == []


def test_invalid_instrument_name_records_nothing(collector: Collector) -> None:
    result = Telemetry(collector.endpoint).metric("1-not-an-instrument-name", "counter", 1)
    assert isinstance(result, Err) and result.error.kind == "validation", result
    assert collector.requests == []


def test_wrong_argument_types_are_programmer_errors() -> None:
    telemetry = Telemetry("http://127.0.0.1:9")
    with pytest.raises(TypeError, match="attributes must be a Mapping"):
        telemetry.log("x", attributes=[("k", "v")])  # type: ignore[arg-type]
    with pytest.raises(TypeError, match="headers must be a Mapping"):
        Telemetry(headers=[("k", "v")])  # type: ignore[arg-type]
    with pytest.raises(TypeError):
        telemetry.log(42)  # type: ignore[arg-type]


def test_unavailable_native_extension_is_an_internal_failure() -> None:
    with patch("sc_observability.telemetry.importlib.import_module", side_effect=ImportError("missing")):
        result = Telemetry().log("x")
    _failure(result, "internal", "SC_OBSERVABILITY_BINDING_INTERNAL")


def test_missing_native_symbol_is_an_internal_failure() -> None:
    # A build without the otlp-telemetry feature has the module but not the function.
    with patch("sc_observability.telemetry.importlib.import_module", return_value=object()):
        result = Telemetry().log("x")
    message = _failure(result, "internal", "SC_OBSERVABILITY_BINDING_INTERNAL")
    assert "send_log" in message, message


# Bounds each collector wait; a send that fails before connecting or sending
# fails the test here instead of reaching the faulthandler watchdog.
COLLECTOR_WAIT_S = 30


def _read_request(connection: socket.socket) -> bytes:
    """Reads one HTTP request, head and content-length body, so the reply
    never closes a connection with unread request bytes."""
    request = b""
    while b"\r\n\r\n" not in request:
        chunk = connection.recv(65536)
        assert chunk, f"connection closed before the request head: {request!r}"
        request += chunk
    head, _, body = request.partition(b"\r\n\r\n")
    length = next((int(line.split(b":", 1)[1]) for line in head.split(b"\r\n")
                   if line.lower().startswith(b"content-length:")), 0)
    while len(body) < length:
        chunk = connection.recv(65536)
        assert chunk, "connection closed before the request body"
        body += chunk
    return head + b"\r\n\r\n" + body


def test_blocked_export_releases_the_gil_and_returns_a_tagged_failure() -> None:
    # The client timeout outlives the watchdog, so a send holding the GIL can
    # only end by the watchdog killing the process: the main thread below must
    # run Python while the send is blocked for the test to finish. Every
    # socket wait has its own shorter deadline, so the watchdog fires only for
    # that deadlock.
    faulthandler.dump_traceback_later(5 * COLLECTOR_WAIT_S, exit=True)
    try:
        with socket.create_server(("127.0.0.1", 0)) as server:
            server.settimeout(COLLECTOR_WAIT_S)
            endpoint = f"http://127.0.0.1:{server.getsockname()[1]}"
            results: list[Result[None]] = []
            done = threading.Event()

            def send() -> None:
                try:
                    results.append(Telemetry(endpoint, timeout_s=600).log("stalled"))
                finally:
                    done.set()

            worker = threading.Thread(target=send, daemon=True)
            worker.start()
            try:
                connection, _ = server.accept()
            except TimeoutError:
                pytest.fail(f"no export connection within {COLLECTOR_WAIT_S} s; results: {results}")
            with connection:
                connection.settimeout(COLLECTOR_WAIT_S)
                try:
                    request = _read_request(connection)
                except TimeoutError:
                    pytest.fail(f"no complete export request within {COLLECTOR_WAIT_S} s")
                assert request.startswith(b"POST /v1/logs "), request[:64]
                # The collector has the request and has not answered: the send is blocked.
                assert not done.is_set()
                # Answer with a rejection rather than closing the connection: a
                # 4xx is not retried (test_rejected_export_is_tagged_and_redacted
                # sees exactly one request), so the send ends on this reply.
                connection.sendall(b"HTTP/1.1 400 Bad Request\r\ncontent-length: 0\r\nconnection: close\r\n\r\n")
            assert done.wait(COLLECTOR_WAIT_S), "send did not finish after the collector rejected it"
        worker.join(timeout=COLLECTOR_WAIT_S)
        assert len(results) == 1
        _failure(results[0], "unavailable", "SC_OBSERVABILITY_OTLP_EXPORT_FAILED")
    finally:
        faulthandler.cancel_dump_traceback_later()
