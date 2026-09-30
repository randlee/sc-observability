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
from urllib.request import Request, urlopen


SERVICE_NAME = "sc-observability-d9"
MAX_DEADLINE_SECONDS = 120
SAFE_ID = re.compile(r"^[A-Za-z0-9][A-Za-z0-9._-]{0,127}$")
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
        if protocol not in {"http/protobuf", "grpc"}:
            raise ConfigurationUnavailable("OTEL_EXPORTER_OTLP_PROTOCOL must be http/protobuf or grpc")
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


def queries(run_id: str, trace_id: str, start: int, end: int) -> dict[str, dict[str, str]]:
    """Return account-neutral query requests using current neutral attributes.

    Attribute dots are normalized to Prometheus label underscores only in the
    PromQL query, which is the documented OTLP-to-Prometheus convention.
    """
    run_id = require_safe_id(run_id, "run_id")
    trace_id = require_safe_id(trace_id, "trace_id")
    selector = f'{{service_name="{SERVICE_NAME}",test_run_id="{run_id}"}}'
    return {
        "logs": {"query": f'{selector} |= "{run_id}"', "start": str(start), "end": str(end), "limit": "100"},
        "traces": {"q": f'{{ resource.service.name = "{SERVICE_NAME}" && .test.run_id = "{run_id}" }}', "start": str(start), "end": str(end)},
        "metrics": {"query": f"sc_observability_d9_probe_total{selector}", "start": str(start), "end": str(end), "step": "15s"},
    }


def request_json(url: str, params: Mapping[str, str], authorization: str, timeout_seconds: float, opener: Callable = urlopen) -> object:
    separator = "&" if "?" in url else "?"
    request = Request(f"{url}{separator}{urlencode(params)}", headers={"Authorization": authorization, "Accept": "application/json"})
    with opener(request, timeout=timeout_seconds) as response:
        return json.loads(response.read().decode("utf-8"))


def contains_exact_log(payload: object, run_id: str) -> bool:
    return run_id in json.dumps(payload, sort_keys=True)


def contains_exact_trace(payload: object, trace_id: str) -> bool:
    return trace_id.lower() in json.dumps(payload, sort_keys=True).lower()


def contains_exact_metric(payload: object, run_id: str) -> bool:
    text = json.dumps(payload, sort_keys=True)
    return run_id in text and ("test_run_id" in text or "test.run_id" in text)


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
        except (OSError, ValueError, json.JSONDecodeError):
            return {"status": "FAIL", "reason": f"{signal} query did not return a usable response", "deadline_seconds": MAX_DEADLINE_SECONDS, "signals": results}
        exact = matcher(payload, trace_id if signal == "traces" else run_id)
        results[signal] = {"exact_match": exact, "query": query_set[signal]}
    presentation = {
        "logs": query_set["logs"]["query"],
        "traces": query_set["traces"]["q"],
        "metrics": query_set["metrics"]["query"],
    }
    passed = all(item["exact_match"] for item in results.values())
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
        return 0 if result["status"] == "PASS" else 2
    except (ConfigurationUnavailable, ValueError) as error:
        print(json.dumps({"status": "BLOCKED", "reason": str(error)}))
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
