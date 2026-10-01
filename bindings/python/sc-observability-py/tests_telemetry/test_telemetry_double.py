from __future__ import annotations

import json
from pathlib import Path

from sc_observability import Ok
from sc_observability.telemetry import Telemetry, TelemetryErr


GOLDENS = Path(__file__).parents[4] / "crates/sc-observability-types/tests/fixtures/otlp_submission/golden"


def _input(name: str) -> dict[str, object]:
    return json.loads((GOLDENS / name / "input.json").read_text())


def _double(tmp_path: Path, script: dict[str, object] | None = None) -> Telemetry:
    result = Telemetry._with_test_double(
        store_path=tmp_path / "store", endpoint="http://127.0.0.1:4318", service_name="d30-test",
        script_json=None if script is None else json.dumps(script),
    )
    assert isinstance(result, Ok)
    return result.value


def test_test_double_admits_each_signal_and_combined_submission(tmp_path: Path) -> None:
    telemetry = _double(tmp_path)
    for name in ("logs", "traces", "metric_gauge", "profiles", "combined"):
        if (GOLDENS / name / "input.json").exists():
            assert isinstance(telemetry.emit(_input(name)), Ok)


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
