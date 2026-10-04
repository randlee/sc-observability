#!/usr/bin/env python3
"""Offline-tested Grafana query adapter for the protected D9 probe.

This file deliberately has no default network path.  A remote query is possible
only when SC_OBS_GRAFANA_PROBE=1 is set and every endpoint and secret *reference*
is present.  The script never prints credential values.
"""

from __future__ import annotations

import argparse
import json
import os
import re
import sys
import time
from dataclasses import dataclass
from typing import Callable, Mapping
from urllib.parse import urlencode
from urllib.error import HTTPError, URLError
from urllib.request import Request, urlopen


SERVICE_NAME = "sc-observability-d9"
MAX_DEADLINE_SECONDS = 120
SAFE_ID = re.compile(r"^[A-Za-z0-9][A-Za-z0-9._-]{0,127}$")
TRACE_ID = re.compile(r"^[0-9a-f]{32}$")
ENVIRONMENT_NAME = re.compile(r"^[A-Z][A-Z0-9_]*$")


class ConfigurationUnavailable(RuntimeError):
    """Raised before any network activity when the protected contract is absent."""


@dataclass(frozen=True)
class GrafanaConfig:
    ingest_endpoint: str
    ingest_protocol: str
    logs_query_url: str
    traces_query_url: str
    metrics_query_url: str
    logs_auth_env: str
    traces_auth_env: str
    metrics_auth_env: str

    @classmethod
    def from_environment(cls, env: Mapping[str, str]) -> "GrafanaConfig":
        fields = {
            "ingest_endpoint": "OTEL_EXPORTER_OTLP_ENDPOINT",
            "ingest_protocol": "OTEL_EXPORTER_OTLP_PROTOCOL",
            "logs_query_url": "SC_OBS_GRAFANA_LOGS_QUERY_URL",
            "traces_query_url": "SC_OBS_GRAFANA_TRACES_QUERY_URL",
            "metrics_query_url": "SC_OBS_GRAFANA_METRICS_QUERY_URL",
            "logs_auth_env": "SC_OBS_GRAFANA_LOGS_AUTH_ENV",
            "traces_auth_env": "SC_OBS_GRAFANA_TRACES_AUTH_ENV",
            "metrics_auth_env": "SC_OBS_GRAFANA_METRICS_AUTH_ENV",
        }
        missing = [name for name in fields.values() if not env.get(name)]
        if missing:
            raise ConfigurationUnavailable("missing configuration references: " + ", ".join(missing))
        protocol = env[fields["ingest_protocol"]]
        if protocol != "http/protobuf":
            raise ConfigurationUnavailable("OTEL_EXPORTER_OTLP_PROTOCOL must be http/protobuf")
        config = cls(**{field: env[name] for field, name in fields.items()})
        for reference in (config.logs_auth_env, config.traces_auth_env, config.metrics_auth_env):
            if not ENVIRONMENT_NAME.fullmatch(reference):
                raise ConfigurationUnavailable("authentication reference is not an environment-variable name")
        return config

    def missing_credentials(self, env: Mapping[str, str]) -> list[str]:
        return [reference for reference in (self.logs_auth_env, self.traces_auth_env, self.metrics_auth_env) if not env.get(reference)]

    def redacted_contract(self) -> dict[str, object]:
        return {
            "ingest_endpoint_configured": bool(self.ingest_endpoint),
            "ingest_protocol": self.ingest_protocol,
            "logs_query_url_configured": bool(self.logs_query_url),
            "traces_query_url_configured": bool(self.traces_query_url),
            "metrics_query_url_configured": bool(self.metrics_query_url),
            "credential_references": {
                "logs": self.logs_auth_env,
                "traces": self.traces_auth_env,
                "metrics": self.metrics_auth_env,
            },
        }


def require_safe_id(value: str, name: str) -> str:
    if not SAFE_ID.fullmatch(value):
        raise ValueError(f"{name} must contain only letters, digits, dot, underscore, or dash")
    return value


def require_trace_id(value: str) -> str:
    if not TRACE_ID.fullmatch(value):
        raise ValueError("trace_id must be exactly 32 lowercase hexadecimal characters")
    return value


def queries(run_id: str, trace_id: str, start: int, end: int) -> dict[str, dict[str, str]]:
    """Return account-neutral query requests using current neutral attributes.

    Attribute dots are normalized to Prometheus label underscores only in the
    PromQL query, which is the documented OTLP-to-Prometheus convention.
    """
    run_id = require_safe_id(run_id, "run_id")
    trace_id = require_trace_id(trace_id)
    metric_selector = f'{{service_name="{SERVICE_NAME}",test_run_id="{run_id}"}}'
    return {
        "logs": {"query": f'{{service_name="{SERVICE_NAME}"}} | json | test_run_id="{run_id}"', "start": str(start), "end": str(end), "limit": "100"},
        "traces": {"q": f'{{ resource.service.name = "{SERVICE_NAME}" && .test.run_id = "{run_id}" }}', "start": str(start), "end": str(end)},
        "metrics": {"query": f"sc_observability_d9_probe_total{metric_selector}", "start": str(start), "end": str(end), "step": "15s"},
    }


def request_json(url: str, params: Mapping[str, str], authorization: str, timeout_seconds: float, opener: Callable = urlopen) -> object:
    separator = "&" if "?" in url else "?"
    request = Request(f"{url}{separator}{urlencode(params)}", headers={"Authorization": authorization, "Accept": "application/json"})
    with opener(request, timeout=timeout_seconds) as response:
        return json.loads(response.read().decode("utf-8"))


def contains_exact_log(payload: object, run_id: str) -> bool:
    """Match an exact synthetic record in Loki's successful streams result."""
    if not isinstance(payload, dict) or payload.get("status") != "success":
        return False
    data = payload.get("data")
    if not isinstance(data, dict) or data.get("resultType") != "streams":
        return False
    streams = data.get("result")
    if not isinstance(streams, list):
        return False
    for stream in streams:
        if not isinstance(stream, dict) or not isinstance(stream.get("values"), list):
            continue
        for value in stream["values"]:
            if isinstance(value, list) and len(value) >= 2 and isinstance(value[1], str) and run_id in value[1]:
                return True
    return False


def contains_exact_trace(payload: object, trace_id: str) -> bool:
    if not isinstance(payload, dict):
        return False
    traces = payload.get("traces")
    if not isinstance(traces, list):
        return False
    return any(isinstance(trace, dict) and trace.get("traceID") == trace_id for trace in traces)


def contains_exact_metric(payload: object, run_id: str) -> bool:
    if not isinstance(payload, dict) or payload.get("status") != "success":
        return False
    data = payload.get("data")
    if not isinstance(data, dict) or not isinstance(data.get("result"), list):
        return False
    for series in data["result"]:
        if not isinstance(series, dict) or not isinstance(series.get("metric"), dict):
            continue
        labels = series["metric"]
        samples = series.get("values")
        if labels.get("test_run_id") == run_id and labels.get("service_name") == SERVICE_NAME and isinstance(samples, list) and samples:
            return True
    return False


def run_probe(config: GrafanaConfig, run_id: str, trace_id: str, now: int, env: Mapping[str, str], opener: Callable = urlopen) -> dict[str, object]:
    missing = config.missing_credentials(env)
    if missing:
        return {"status": "BLOCKED", "reason": "missing query credential references", "missing_references": missing}
    query_set = queries(run_id, trace_id, now - MAX_DEADLINE_SECONDS, now)
    endpoints = {
        "logs": (config.logs_query_url, config.logs_auth_env, contains_exact_log),
        "traces": (config.traces_query_url, config.traces_auth_env, contains_exact_trace),
        "metrics": (config.metrics_query_url, config.metrics_auth_env, contains_exact_metric),
    }
    results: dict[str, object] = {}
    deadline = time.monotonic() + MAX_DEADLINE_SECONDS
    for signal, (url, auth_reference, matcher) in endpoints.items():
        remaining = deadline - time.monotonic()
        if remaining <= 0:
            return {"status": "FAIL", "reason": "three-signal query deadline exceeded", "deadline_seconds": MAX_DEADLINE_SECONDS, "signals": results}
        try:
            payload = request_json(url, query_set[signal], env[auth_reference], remaining, opener)
        except HTTPError as error:
            if error.code in {401, 403}:
                return {"status": "BLOCKED", "reason": f"{signal} query access is unavailable", "signals": results}
            return {"status": "FAIL", "reason": f"{signal} query returned an HTTP error", "signals": results}
        except (URLError, TimeoutError, ConnectionError, OSError):
            return {"status": "BLOCKED", "reason": f"{signal} query access is unavailable", "signals": results}
        except (ValueError, json.JSONDecodeError):
            return {"status": "FAIL", "reason": f"{signal} query did not return a usable response", "deadline_seconds": MAX_DEADLINE_SECONDS, "signals": results}
        if time.monotonic() >= deadline:
            return {"status": "FAIL", "reason": "three-signal query deadline exceeded", "deadline_seconds": MAX_DEADLINE_SECONDS, "signals": results}
        exact = matcher(payload, trace_id if signal == "traces" else run_id)
        results[signal] = {"structural_match": exact, "query": query_set[signal]}
    presentation = {
        "logs": query_set["logs"]["query"],
        "traces": query_set["traces"]["q"],
        "metrics": query_set["metrics"]["query"],
    }
    passed = all(item["structural_match"] for item in results.values())
    return {"status": "PASS" if passed else "FAIL", "deadline_seconds": MAX_DEADLINE_SECONDS, "signals": results, "presentation_queries": presentation}


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--run-id", required=True)
    parser.add_argument("--trace-id", required=True)
    parser.add_argument("--print-contract", action="store_true")
    args = parser.parse_args(argv)
    try:
        config = GrafanaConfig.from_environment(os.environ)
        if args.print_contract:
            print(json.dumps(config.redacted_contract(), sort_keys=True))
            return 0
        if os.environ.get("SC_OBS_GRAFANA_PROBE") != "1":
            print(json.dumps({"status": "BLOCKED", "reason": "set SC_OBS_GRAFANA_PROBE=1 for the protected remote probe"}))
            return 2
        result = run_probe(config, args.run_id, args.trace_id, int(time.time()), os.environ)
        print(json.dumps(result, sort_keys=True))
        return {"PASS": 0, "FAIL": 1, "BLOCKED": 2}[result["status"]]
    except (ConfigurationUnavailable, ValueError) as error:
        print(json.dumps({"status": "BLOCKED", "reason": str(error)}))
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
