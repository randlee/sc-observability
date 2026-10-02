"""Equivalence of the two installed Wave 5 submission front ends."""
from __future__ import annotations

import json
from pathlib import Path

from conftest import CaptureCollector, GOLDENS, canonical_json, normalise_system_generated_fields, run_cli, run_installed_python


def _python_build_script() -> str:
    return """\
import json
import sys
from sc_observability import Ok
from sc_observability.telemetry import build_envelope

result = build_envelope(json.load(sys.stdin))
if isinstance(result, Ok):
    print(json.dumps({"kind": "ok", "envelope": json.loads(result.value)}, separators=(",", ":")))
else:
    print(json.dumps({"kind": "error", "code": result.error.code}, separators=(",", ":")))
"""


def _python_emit_script(config: Path) -> str:
    return f"""\\
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


def _config_with_store(config: Path, store: str) -> Path:
    text = config.read_text(encoding="utf-8").replace("path: store.sqlite", f"path: {store}")
    result = config.parent / f"{store}.yaml"
    result.write_text(text, encoding="utf-8")
    return result


def _captured_payloads(collector: CaptureCollector, submitted: dict[str, object]) -> dict[str, list[dict[str, object]]]:
    """Return the real OTLP/JSON requests for every submitted signal family."""
    paths = {
        "logs": "/v1/logs",
        "spans": "/v1/traces",
        "metrics": "/v1/metrics",
        "profiles": "/v1development/profiles",
    }
    return {
        path: collector.wait_for(path)
        for signal, path in paths.items()
        if submitted.get(signal)
    }


def _normalise_generated_capture_fields(
    payloads: dict[str, list[dict[str, object]]], submitted: dict[str, object],
) -> dict[str, list[dict[str, object]]]:
    """Mask only D29's invocation-generated fields in retained OTLP/JSON.

    The submitted contract, rather than a broad wire-field search, decides
    whether a field may differ between the independently installed processes.
    """
    result = json.loads(json.dumps(payloads))
    signal_shapes = {
        "logs": ("/v1/logs", "resourceLogs", "scopeLogs", "logRecords", {
            "observed_time": "observedTimeUnixNano",
            "trace_id": "traceId",
            "span_id": "spanId",
        }),
        "spans": ("/v1/traces", "resourceSpans", "scopeSpans", "spans", {
            "trace_id": "traceId",
            "span_id": "spanId",
        }),
    }
    for signal, (path, resources, scopes, records, generated_fields) in signal_shapes.items():
        source_records = submitted.get(signal, [])
        if not source_records:
            continue
        captured_records = [
            record
            for request in result[path]
            for resource in request[resources]
            for scope in resource[scopes]
            for record in scope[records]
        ]
        assert len(captured_records) == len(source_records), signal
        for captured, source in zip(captured_records, source_records):
            for input_name, wire_name in generated_fields.items():
                if input_name not in source and wire_name in captured:
                    captured[wire_name] = f"$GENERATED_{input_name.upper()}"
    return result


def test_every_d29_golden_matches_installed_python_and_cli(installed_artifacts: dict[str, Path], tmp_path: Path) -> None:
    """Both public front ends accept the single D29 SubmissionInput contract."""
    fixtures = sorted(path for path in GOLDENS.iterdir() if (path / "input.json").is_file())
    assert fixtures, "the D29 golden corpus must not silently disappear"
    for fixture in fixtures:
        source = (fixture / "input.json").read_text(encoding="utf-8")
        python = run_installed_python(installed_artifacts, _python_build_script(), cwd=tmp_path, input=source)
        cli = run_cli(installed_artifacts, "validate", "--stdin", cwd=tmp_path, input=source)
        assert python.returncode == 0, python.stderr
        py_result = json.loads(python.stdout)
        cli_result = json.loads(cli.stdout)
        expected_error = fixture / "expected.error.json"
        if expected_error.exists():
            expected = json.loads(expected_error.read_text(encoding="utf-8"))["code"]
            assert py_result == {"kind": "error", "code": expected}, fixture.name
            assert cli.returncode == 3, fixture.name
            assert cli_result["error"]["code"] == expected, fixture.name
            continue
        assert py_result["kind"] == "ok", fixture.name
        assert cli.returncode == 0, fixture.name
        # The values originate in distinct installed front ends.  Comparing the
        # serialized canonical documents catches ordering and conversion drift.
        # Only D29's three invocation-generated fields may differ.
        submitted = json.loads(source)
        assert canonical_json(normalise_system_generated_fields(py_result["envelope"], submitted)) == canonical_json(
            normalise_system_generated_fields(cli_result["envelope"], submitted)
        ), fixture.name


def test_every_valid_d29_golden_emits_identical_collector_payloads(
    installed_artifacts: dict[str, Path], telemetry_config: Path, collector: CaptureCollector, tmp_path: Path,
) -> None:
    """The two public emit paths must share the canonical encoder boundary."""
    fixtures = sorted(path for path in GOLDENS.iterdir() if (path / "expected.envelope.json").is_file())
    assert fixtures, "the D29 valid golden corpus must not silently disappear"
    for fixture in fixtures:
        source = (fixture / "input.json").read_text(encoding="utf-8")
        submitted = json.loads(source)

        collector.clear()
        python_config = _config_with_store(telemetry_config, f"python-{fixture.name}.sqlite")
        python = run_installed_python(
            installed_artifacts, _python_emit_script(python_config), cwd=tmp_path, input=source,
        )
        assert python.returncode == 0, python.stdout + python.stderr
        python_payloads = _captured_payloads(collector, submitted)

        collector.clear()
        cli_config = _config_with_store(telemetry_config, f"cli-{fixture.name}.sqlite")
        cli = run_cli(
            installed_artifacts, "--config", str(cli_config), "emit", "--stdin", cwd=tmp_path, input=source,
        )
        assert cli.returncode == 0, cli.stdout + cli.stderr
        cli_payloads = _captured_payloads(collector, submitted)

        assert canonical_json(_normalise_generated_capture_fields(python_payloads, submitted)) == canonical_json(
            _normalise_generated_capture_fields(cli_payloads, submitted)
        ), fixture.name
