#!/usr/bin/env python3
"""Hermetic tests for the Grafana D9 query adapter; no account is contacted."""

from __future__ import annotations

import importlib.util
import io
import json
import pathlib
import sys
import unittest
from unittest import mock
from urllib.error import HTTPError, URLError


MODULE_PATH = pathlib.Path(__file__).with_name("grafana_probe.py")
SPEC = importlib.util.spec_from_file_location("grafana_probe", MODULE_PATH)
assert SPEC and SPEC.loader
probe = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = probe
SPEC.loader.exec_module(probe)


class FakeResponse:
    def __init__(self, payload: object) -> None:
        self.payload = payload

    def __enter__(self) -> "FakeResponse":
        return self

    def __exit__(self, *_: object) -> None:
        return None

    def read(self) -> bytes:
        return json.dumps(self.payload).encode("utf-8")


class GrafanaProbeTests(unittest.TestCase):
    TRACE_ID = "0123456789abcdef0123456789abcdef"

    def setUp(self) -> None:
        self.env = {
            "OTEL_EXPORTER_OTLP_ENDPOINT": "https://ingest.example.invalid/otlp",
            "OTEL_EXPORTER_OTLP_PROTOCOL": "http/protobuf",
            "SC_OBS_GRAFANA_LOGS_QUERY_URL": "https://logs.example.invalid/loki/api/v1/query_range",
            "SC_OBS_GRAFANA_TRACES_QUERY_URL": "https://traces.example.invalid/api/search",
            "SC_OBS_GRAFANA_METRICS_QUERY_URL": "https://metrics.example.invalid/api/v1/query_range",
            "SC_OBS_GRAFANA_LOGS_AUTH_ENV": "D9_LOGS_AUTH",
            "SC_OBS_GRAFANA_TRACES_AUTH_ENV": "D9_TRACES_AUTH",
            "SC_OBS_GRAFANA_METRICS_AUTH_ENV": "D9_METRICS_AUTH",
            "D9_LOGS_AUTH": "Bearer test-only-logs",
            "D9_TRACES_AUTH": "Bearer test-only-traces",
            "D9_METRICS_AUTH": "Bearer test-only-metrics",
        }

    def test_contract_redacts_credential_values(self) -> None:
        contract = probe.GrafanaConfig.from_environment(self.env).redacted_contract()
        self.assertEqual(contract["credential_references"]["logs"], "D9_LOGS_AUTH")
        self.assertNotIn("test-only-logs", json.dumps(contract))

    def test_queries_use_current_schema_normalization(self) -> None:
        result = probe.queries("run-42", self.TRACE_ID, 1, 2)
        self.assertIn('service_name="sc-observability-d9"', result["logs"]["query"])
        self.assertIn("| json | test_run_id", result["logs"]["query"])
        self.assertIn('test_run_id="run-42"', result["metrics"]["query"])
        self.assertIn("resource.service.name", result["traces"]["q"])

    def test_missing_credential_blocks_before_opening_network(self) -> None:
        env = {key: value for key, value in self.env.items() if key != "D9_TRACES_AUTH"}
        called = False

        def opener(_: object, timeout: int) -> FakeResponse:
            nonlocal called
            called = True
            raise AssertionError("must not call a remote endpoint")

        result = probe.run_probe(probe.GrafanaConfig.from_environment(env), "run-42", self.TRACE_ID, 200, env, opener)
        self.assertEqual(result["status"], "BLOCKED")
        self.assertFalse(called)

    def test_mocked_three_signal_queries_pass(self) -> None:
        payloads = iter([
            {"status": "success", "data": {"resultType": "streams", "result": [{"values": [["1", "run-42 exact log"]]}]}},
            {"traces": [{"traceID": self.TRACE_ID}]},
            {"status": "success", "data": {"result": [{"metric": {"service_name": probe.SERVICE_NAME, "test_run_id": "run-42"}, "values": [["1", "1"]]}]}},
        ])
        requested: list[str] = []

        def opener(request: object, timeout: int) -> FakeResponse:
            requested.append(request.full_url)  # type: ignore[attr-defined]
            self.assertGreater(timeout, 0)
            self.assertLessEqual(timeout, probe.MAX_DEADLINE_SECONDS)
            return FakeResponse(next(payloads))

        result = probe.run_probe(probe.GrafanaConfig.from_environment(self.env), "run-42", self.TRACE_ID, 200, self.env, opener)
        self.assertEqual(result["status"], "PASS")
        self.assertEqual(len(requested), 3)
        self.assertTrue(any("loki/api/v1/query_range" in url for url in requested))

    def test_structural_matchers_reject_empty_wrong_and_echoed_payloads(self) -> None:
        self.assertFalse(probe.contains_exact_log({"status": "error", "message": "run-42"}, "run-42"))
        self.assertFalse(probe.contains_exact_log({"status": "success", "data": {"resultType": "streams", "result": []}}, "run-42"))
        self.assertFalse(probe.contains_exact_trace({"traces": [{"traceID": "f" * 32}]}, self.TRACE_ID))
        self.assertFalse(probe.contains_exact_metric({"status": "success", "data": {"result": [{"metric": {"service_name": probe.SERVICE_NAME, "test_run_id": "run-42"}, "values": []}]}}, "run-42"))

    def test_mismatch_is_fail_not_pass(self) -> None:
        payloads = iter([
            {"status": "success", "data": {"resultType": "streams", "result": []}},
            {"traces": [{"traceID": "f" * 32}]},
            {"status": "success", "data": {"result": []}},
        ])
        result = probe.run_probe(probe.GrafanaConfig.from_environment(self.env), "run-42", self.TRACE_ID, 200, self.env, lambda *_args, **_kwargs: FakeResponse(next(payloads)))
        self.assertEqual(result["status"], "FAIL")

    def test_access_failure_is_blocked(self) -> None:
        result = probe.run_probe(probe.GrafanaConfig.from_environment(self.env), "run-42", self.TRACE_ID, 200, self.env, lambda *_args, **_kwargs: (_ for _ in ()).throw(URLError("no route")))
        self.assertEqual(result["status"], "BLOCKED")
        self.assertIn("access is unavailable", result["reason"])

    def test_unauthorized_is_blocked(self) -> None:
        error = HTTPError("https://example.invalid", 401, "unauthorized", {}, None)
        result = probe.run_probe(probe.GrafanaConfig.from_environment(self.env), "run-42", self.TRACE_ID, 200, self.env, lambda *_args, **_kwargs: (_ for _ in ()).throw(error))
        self.assertEqual(result["status"], "BLOCKED")

    def test_deadline_is_checked_after_a_response(self) -> None:
        payload = {"status": "success", "data": {"resultType": "streams", "result": [{"values": [["1", "run-42"]]}]}}
        with mock.patch.object(probe.time, "monotonic", side_effect=[0.0, 0.0, 121.0]):
            result = probe.run_probe(probe.GrafanaConfig.from_environment(self.env), "run-42", self.TRACE_ID, 200, self.env, lambda *_args, **_kwargs: FakeResponse(payload))
        self.assertEqual(result["status"], "FAIL")
        self.assertEqual(result["reason"], "three-signal query deadline exceeded")

    def test_invalid_protocol_and_identifiers_are_rejected(self) -> None:
        missing = dict(self.env)
        del missing["OTEL_EXPORTER_OTLP_ENDPOINT"]
        with self.assertRaises(probe.ConfigurationUnavailable):
            probe.GrafanaConfig.from_environment(missing)
        invalid = dict(self.env, OTEL_EXPORTER_OTLP_PROTOCOL="grpc")
        with self.assertRaises(probe.ConfigurationUnavailable):
            probe.GrafanaConfig.from_environment(invalid)
        with self.assertRaises(ValueError):
            probe.queries("unsafe value", self.TRACE_ID, 1, 2)
        with self.assertRaises(ValueError):
            probe.queries("run-42", "0123456789abcdef", 1, 2)

    def test_main_gate_and_redacted_contract(self) -> None:
        with mock.patch.dict(probe.os.environ, self.env, clear=True), mock.patch("sys.stdout", new_callable=io.StringIO) as output:
            self.assertEqual(probe.main(["--run-id", "run-42", "--trace-id", self.TRACE_ID]), 2)
            self.assertEqual(json.loads(output.getvalue())["status"], "BLOCKED")
        with mock.patch.dict(probe.os.environ, self.env, clear=True), mock.patch("sys.stdout", new_callable=io.StringIO) as output:
            self.assertEqual(probe.main(["--run-id", "run-42", "--trace-id", self.TRACE_ID, "--print-contract"]), 0)
            rendered = output.getvalue()
            self.assertNotIn("test-only-logs", rendered)
            self.assertTrue(json.loads(rendered)["ingest_endpoint_configured"])


if __name__ == "__main__":
    unittest.main()
