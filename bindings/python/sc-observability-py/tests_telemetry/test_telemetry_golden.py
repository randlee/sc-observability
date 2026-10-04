from __future__ import annotations

import json
from pathlib import Path

from sc_observability import Ok
from sc_observability.telemetry import TelemetryErr, build_envelope


GOLDENS = Path(__file__).parents[4] / "crates/sc-observability-types/tests/fixtures/otlp_submission/golden"


def _placeholder_equal(actual: object, expected: object) -> bool:
    if isinstance(expected, str) and (expected.startswith("$GENERATED_") or expected == "1970-01-01T00:00:00.000000000Z"):
        return isinstance(actual, str)
    if isinstance(expected, dict) and isinstance(actual, dict):
        return actual.keys() == expected.keys() and all(
            isinstance(actual[key], str) if key in {"trace_id", "span_id"} and expected[key] is not None else _placeholder_equal(actual[key], value)
            for key, value in expected.items()
        )
    if isinstance(expected, list) and isinstance(actual, list):
        return len(actual) == len(expected) and all(_placeholder_equal(left, right) for left, right in zip(actual, expected))
    return actual == expected


def test_every_submission_golden_is_a_tagged_result() -> None:
    for fixture in sorted(GOLDENS.iterdir()):
        result = build_envelope(json.loads((fixture / "input.json").read_text()))
        expected_error = fixture / "expected.error.json"
        if expected_error.exists():
            assert isinstance(result, TelemetryErr), fixture.name
            assert result.error.kind == "submission", fixture.name
            assert result.error.code == json.loads(expected_error.read_text())["code"], fixture.name
        else:
            assert isinstance(result, Ok), fixture.name
            assert _placeholder_equal(json.loads(result.value), json.loads((fixture / "expected.envelope.json").read_text())), fixture.name
