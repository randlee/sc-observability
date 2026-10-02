from __future__ import annotations

import json
import pytest
from pathlib import Path

from telemetry_test_support import open_test_double
from sc_observability import Ok
from sc_observability.telemetry import Telemetry, TelemetryErr


GOLDENS = Path(__file__).parents[4] / "crates/sc-observability-types/tests/fixtures/otlp_submission/golden"


def _input(name: str) -> dict[str, object]:
    return json.loads((GOLDENS / name / "input.json").read_text())


def _double(tmp_path: Path, script: dict[str, object] | None = None) -> Telemetry:
    result = open_test_double(
        store_path=tmp_path / "store", endpoint="http://127.0.0.1:4318", service_name="d30-test",
        script_json=None if script is None else json.dumps(script),
    )
    assert isinstance(result, Ok)
    return result.value


def test_test_double_admits_each_signal_and_combined_submission(tmp_path: Path) -> None:
    telemetry = _double(tmp_path)
    for name in ("logs", "traces", "metric_gauge", "metric_sum", "metric_histogram", "metric_exponential_histogram", "metric_summary", "profiles", "paired_log_span_generated_ids"):
        result = telemetry.emit(_input(name))
        assert isinstance(result, Ok), (name, result)


def test_delivery_failure_is_returned_and_retained_at_context_exit(tmp_path: Path) -> None:
    script = {"deliveries": [{"signal": "logs", "outcome": "fail"}]}
    telemetry = _double(tmp_path, script)
    with telemetry:
        assert isinstance(telemetry.emit(_input("logs")), Ok)
    assert isinstance(telemetry.last_shutdown, TelemetryErr)
    assert telemetry.last_shutdown.error.kind == "delivery"
    assert telemetry.last_shutdown.error.variant == "terminal_failure"
    assert telemetry.last_shutdown.error.report is not None


def test_scripted_delivery_failure_is_returned_from_flush(tmp_path: Path) -> None:
    telemetry = _double(tmp_path, {"deliveries": [{"signal": "logs", "outcome": "fail"}]})
    assert isinstance(telemetry.emit(_input("logs")), Ok)
    result = telemetry.flush()
    assert isinstance(result, TelemetryErr)
    assert result.error.kind == "delivery"
    assert result.error.variant == "terminal_failure"
    assert result.error.report is not None


def test_duplicate_key_returns_the_original_admission(tmp_path: Path) -> None:
    telemetry = _double(tmp_path)
    document = _input("logs")
    document["record_key"] = "repeat"
    first = telemetry.emit(document)
    second = telemetry.emit(document)
    assert isinstance(first, Ok) and isinstance(second, Ok)
    assert second.value.duplicate
    assert second.value.submission_id == first.value.submission_id


def test_stalled_delivery_returns_deadline_report(tmp_path: Path) -> None:
    telemetry = _double(tmp_path, {"deliveries": [{"signal": "logs", "outcome": "stall"}]})
    receipt = telemetry.emit(_input("logs"))
    assert isinstance(receipt, Ok)
    result = telemetry.flush_submission(receipt.value.submission_id, 0)
    assert isinstance(result, TelemetryErr)
    assert (result.error.kind, result.error.variant, result.error.code) == ("delivery", "deadline_exceeded", "SC_OBSERVABILITY_DELIVERY_DEADLINE")
    assert result.error.report.still_pending.logs == 1
    assert result.error.remediation


def test_scripted_admission_rejection_keeps_kind_and_code(tmp_path: Path) -> None:
    telemetry = _double(tmp_path, {"admissions": [{"outcome": "reject", "kind": "disk_bound_exceeded"}]})
    result = telemetry.emit(_input("logs"))
    assert isinstance(result, TelemetryErr)
    assert (result.error.kind, result.error.code) == ("admission", "SC_OBSERVABILITY_ADMIT_DISK_BOUND")

@pytest.mark.parametrize("document", [{"bad": object()}, {"bad": float("nan")}])
def test_unserializable_mapping_uses_rust_invalid_json_code(tmp_path: Path, document: dict) -> None:
    from sc_observability.telemetry import build_envelope
    for result in (_double(tmp_path).emit(document), build_envelope(document)):
        assert isinstance(result, TelemetryErr)
        assert (result.error.kind, result.error.variant, result.error.code) == ("submission", "invalid_json", "SC_OBSERVABILITY_SUBMIT_INVALID_JSON")
        assert result.error.cause and result.error.remediation

@pytest.mark.parametrize("timeout", [float("nan"), float("inf"), -1.0, 2**64, 1e308])
@pytest.mark.parametrize("method", ["flush", "flush_submission", "shutdown"])
def test_invalid_timeout_is_config_error(tmp_path: Path, timeout: float, method: str) -> None:
    telemetry = _double(tmp_path)
    args = ("018f01aa-0000-7000-8000-000000000001", timeout) if method == "flush_submission" else (timeout,)
    result = getattr(telemetry, method)(*args)
    assert isinstance(result, TelemetryErr)
    assert (result.error.kind, result.error.code) == ("config", "SC_OBSERVABILITY_TELEMETRY_CONFIG_INVALID")


def test_native_query_and_submission_id_parse_errors_are_tagged(tmp_path: Path) -> None:
    telemetry = _double(tmp_path)
    result = telemetry.flush_submission("not-an-id")
    assert isinstance(result, TelemetryErr) and result.error.kind == "submission"
    for query in ("{", '"unsupported"'):
        raw = json.loads(telemetry._native.status(query))
        assert raw["error"]["code"] == "SC_OBSERVABILITY_TELEMETRY_CONFIG_INVALID"
        assert raw["error"]["cause"]
    from sc_observability.telemetry import _result, _flush_report
    result = _result(json.loads(telemetry._native.flush(0)), _flush_report)
    assert isinstance(result, Ok)


def test_python_programmer_errors_remain_raised(tmp_path: Path) -> None:
    telemetry = _double(tmp_path)
    with pytest.raises(TypeError):
        telemetry.emit([])
    with pytest.raises(ValueError):
        telemetry.status(submissions=[], record_keys=[])


def test_internal_failure_has_registered_code(tmp_path: Path) -> None:
    from sc_observability import generated
    from unittest.mock import patch
    with patch("sc_observability.telemetry.importlib.import_module", side_effect=RuntimeError("load failed")):
        result = Telemetry.open()
    assert isinstance(result, TelemetryErr)
    assert result.error.kind == "internal"
    assert result.error.code == generated.SC_OBSERVABILITY_BINDING_INTERNAL
    assert result.error.cause == "load failed"


def test_native_selected_status_decodes_delivery_status(tmp_path: Path) -> None:
    from sc_observability import DeliveryStatus
    telemetry = _double(tmp_path)
    receipt = telemetry.emit(_input("logs"))
    assert isinstance(receipt, Ok)
    status = telemetry.status(submissions=[receipt.value.submission_id])
    assert isinstance(status, Ok), status
    assert isinstance(status.value.submissions[0], DeliveryStatus)
    assert status.value.submissions[0].signals[0][0] == "logs"
    assert status.value.submissions[0].signals[0][1]["state"] == "pending"
