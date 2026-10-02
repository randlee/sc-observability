"""Collector capture, deliberately separate from pinned-viewer assertions."""
from __future__ import annotations

import json
from pathlib import Path

from conftest import GOLDENS, run_cli, run_installed_python


def _python_emit_script(config: Path) -> str:
    return f"""\
import json
import sys
from sc_observability import Ok
from sc_observability.telemetry import Telemetry

opened = Telemetry.open(config={str(config)!r})
assert isinstance(opened, Ok), opened
with opened.value as telemetry:
    emitted = telemetry.emit(json.load(sys.stdin))
    assert isinstance(emitted, Ok), emitted
    flushed = telemetry.flush_submission(emitted.value.submission_id, timeout_s=10)
    assert isinstance(flushed, Ok), flushed
print("ok")
"""


def _valid(name: str) -> str:
    return (GOLDENS / name / "input.json").read_text(encoding="utf-8")


def _exponential_histogram_with_exemplar() -> str:
    """Return a valid histogram fixture with a concrete exemplar to capture."""
    payload = json.loads(_valid("metric_exponential_histogram"))
    point = payload["metrics"][0]["data"]["data"]["points"][0]
    point["exemplars"] = [{
        "filtered_attributes": {"exemplar.source": "collector-capture"},
        "time": "1970-01-01T00:00:01.000000000Z",
        "value": {"kind": "double", "data": 2.5},
        "trace_id": "00112233445566778899aabbccddeeff",
        "span_id": "0123456789abcdef",
    }]
    return json.dumps(payload)


def _config_with_store(config: Path, store: str) -> Path:
    text = config.read_text(encoding="utf-8").replace("path: store.sqlite", f"path: {store}")
    result = config.parent / f"{store}.yaml"
    result.write_text(text, encoding="utf-8")
    return result


def _assert_wire_form(fixture: str, request: dict[str, object]) -> None:
    """Assert concrete OTLP/JSON fields, rather than merely a successful POST."""
    if fixture not in {
        "metric_exponential_histogram",
        "metric_summary",
        "metric_exponential_histogram_with_exemplar",
        "profiles",
    }:
        raise AssertionError(f"unknown collector capture fixture: {fixture}")

    if fixture == "profiles":
        dictionary = request["dictionary"]
        profile = request["resourceProfiles"][0]["scopeProfiles"][0]["profiles"][0]
        assert dictionary["stringTable"] == [""]
        assert dictionary["functionTable"] == [{
            "nameStrindex": 0,
            "systemNameStrindex": 0,
            "filenameStrindex": 0,
            "startLine": "0",
        }]
        assert dictionary["locationTable"] == [{
            "mappingIndex": 0,
            "address": "0",
            "lines": [],
            "attributeIndices": [],
        }]
        assert dictionary["linkTable"][0]["traceId"] == "AAAAAAAAAAAAAAAAAAAAAA=="
        assert profile["profileId"] == "EREREREREREREREREREREQ=="
        return

    metric = request["resourceMetrics"][0]["scopeMetrics"][0]["metrics"][0]
    if fixture == "metric_exponential_histogram":
        point = metric["exponentialHistogram"]["dataPoints"][0]
        assert point["scale"] == 0
        assert point["zeroCount"] == "1"
        assert point["positive"]["bucketCounts"] == ["1"]
        assert point["negative"]["bucketCounts"] == ["1"]
    elif fixture == "metric_summary":
        point = metric["summary"]["dataPoints"][0]
        assert point["count"] == "2"
        assert point["sum"] == 4.0
        assert point["quantileValues"] == [{"quantile": 0.5, "value": "NaN"}]
    elif fixture == "metric_exponential_histogram_with_exemplar":
        point = metric["exponentialHistogram"]["dataPoints"][0]
        assert point["exemplars"] == [{
            "filteredAttributes": [{
                "key": "exemplar.source",
                "value": {"stringValue": "collector-capture"},
            }],
            "timeUnixNano": "1000000000",
            "asDouble": 2.5,
            "traceId": "00112233445566778899aabbccddeeff",
            "spanId": "0123456789abcdef",
        }]


def test_wire_form_rejects_unknown_fixture() -> None:
    """An unsupported fixture must not silently skip all wire assertions."""
    try:
        _assert_wire_form("unknown", {})
    except AssertionError as error:
        assert "unknown collector capture fixture" in str(error)
    else:
        raise AssertionError("unknown collector capture fixture was accepted")


def test_installed_frontends_export_viewer_unsupported_representations(
    installed_artifacts: dict[str, Path], telemetry_config: Path, collector: object, tmp_path: Path,
) -> None:
    """Capture is the authority for profiles, summary, exponential histogram, and exemplars.

    This intentionally makes no viewer claim: pinned viewer v0.5.0 cannot query
    every representation.  The test checks the real HTTP paths and nonempty
    OTLP/JSON records emitted by the installed wheel and CLI.
    """
    from conftest import CaptureCollector
    assert isinstance(collector, CaptureCollector)
    cases = (
        ("metric_exponential_histogram", "/v1/metrics", _valid("metric_exponential_histogram")),
        ("metric_summary", "/v1/metrics", _valid("metric_summary")),
        ("metric_exponential_histogram_with_exemplar", "/v1/metrics", _exponential_histogram_with_exemplar()),
        ("profiles", "/v1development/profiles", _valid("profiles")),
    )
    for fixture, path, payload in cases:
        collector.clear()
        python = run_installed_python(installed_artifacts, _python_emit_script(telemetry_config), cwd=tmp_path, input=payload)
        assert python.returncode == 0, python.stderr
        python_records = collector.wait_for(path)
        assert len(python_records) == 1, fixture
        _assert_wire_form(fixture, python_records[0])

        collector.clear()
        # Independent installed front ends need independent stores: the same
        # canonical records intentionally retain the same record keys.
        cli_config = _config_with_store(telemetry_config, f"cli-{fixture}.sqlite")
        cli = run_cli(installed_artifacts, "--config", str(cli_config), "emit", "--stdin", cwd=tmp_path, input=payload)
        assert cli.returncode == 0, cli.stdout + cli.stderr
        cli_records = collector.wait_for(path)
        assert len(cli_records) == 1, fixture
        _assert_wire_form(fixture, cli_records[0])
