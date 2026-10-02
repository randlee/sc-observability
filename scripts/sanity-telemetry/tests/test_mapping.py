from __future__ import annotations

import json
from pathlib import Path

from sc_observability import Ok
from sc_observability.telemetry import Telemetry, TelemetryErr, build_envelope

from import_sanity import Source, map_row


FIXTURES = Path(__file__).parent / "fixtures"
CONFIG = {"service": "sc-observability", "team": "sc-obs", "github": {"pr_url_template": "https://github.com/randlee/sc-observability/pull/{pr_number}"}}


def _double(tmp_path: Path) -> Telemetry:
    result = Telemetry._with_test_double(store_path=tmp_path / "store", endpoint="http://127.0.0.1:4318", service_name="d35-test")
    assert isinstance(result, Ok)
    return result.value


def test_real_format_fixtures_map_to_valid_neutral_envelopes_and_test_double(tmp_path: Path) -> None:
    for name, kind in (("sanity-current", "sanity"), ("sanity-legacy", "sanity"), ("qa", "qa"), ("qa-correction", "qa"), ("finding-counts", "finding-counts")):
        row = json.loads((FIXTURES / f"{name}.json").read_text())
        document = map_row(Source(f".sc/{name}.jsonl", kind, "phase-d", "sanity-llm"), row, CONFIG)
        assert document is not None
        assert isinstance(build_envelope(document), Ok)
        assert isinstance(_double(tmp_path / name).emit(document), Ok)


def test_current_pair_preserves_utc_nanos_and_shared_trace() -> None:
    left = json.loads((FIXTURES / "sanity-current.json").read_text())
    right = dict(left, reviewer="sanity-jev")
    source = Source(".sc/sanity.jsonl", "sanity", "phase-d")
    a, b = map_row(source, left, CONFIG), map_row(source, right, CONFIG)
    assert a and b
    assert a["spans"][0]["trace_id"] == b["spans"][0]["trace_id"]
    assert a["logs"][0]["time"] == "2026-10-01T12:00:00.123456789Z"


def test_missing_mandatory_time_or_verdict_is_skipped() -> None:
    source = Source(".sc/sanity.jsonl", "sanity", "phase-d")
    assert map_row(source, {"verdict": "PASS"}, CONFIG) is None
    assert map_row(source, {"completed_at": "2026-10-01T12:00:00Z"}, CONFIG) is None
