from __future__ import annotations

import json
from pathlib import Path

from sc_observability import Ok
from sc_observability.telemetry import Telemetry, build_envelope

from import_sanity import Source, map_row


FIXTURES = Path(__file__).parent / "fixtures"
CONFIG = {
    "service": "sc-observability",
    "team": "sc-obs",
    "github": {"pr_url_template": "https://github.com/randlee/sc-observability/pull/{pr_number}"},
}


def _double(tmp_path: Path) -> Telemetry:
    result = Telemetry._with_test_double(store_path=tmp_path / "store", endpoint="http://127.0.0.1:4318", service_name="d35-test")
    assert isinstance(result, Ok)
    return result.value


def _attrs(document: dict[str, object]) -> dict[str, object]:
    return document["logs"][0]["attributes"]  # type: ignore[index,return-value]


def _decoded_attributes(attributes: list[list[object]]) -> dict[str, object]:
    return {name: value["data"] for name, value in attributes}  # type: ignore[misc,index]


def test_real_format_fixtures_map_to_exact_envelopes_admitted_by_test_double(tmp_path: Path) -> None:
    cases = (
        ("sanity-current", "sanity", {
            "phase": "phase-d", "review.reviewer": "sanity-llm", "review.verdict": "PASS",
            "vcs.commit.sha": "a" * 40, "review.task": "obs-d-35", "review.sprint": "d-35",
            "review.iteration": 1, "review.duration_s": 60, "review.findings": 0,
            "vcs.pr.number": 12, "pr.url": "https://github.com/randlee/sc-observability/pull/12",
        }),
        ("sanity-legacy", "sanity", {
            "phase": "phase-d", "review.reviewer": "sanity-llm", "review.verdict": "FAIL",
            "review.task": "obs-d-35", "review.sprint": "d-35", "review.iteration": 1,
            "review.duration": "1m", "review.findings": 1, "vcs.pr.number": 12,
            "pr.url": "https://github.com/randlee/sc-observability/pull/12",
        }),
        ("qa", "qa", {
            "phase": "phase-d", "review.verdict": "FAIL", "review.task": "obs-d-35",
            "review.sprint": "d-35", "review.iteration": 1, "review.duration": "1m",
            "review.findings": 6, "qa.blocking": 1, "qa.important": 2, "qa.minor": 3,
            "qa.tested": "all", "vcs.pr.number": 12,
            "pr.url": "https://github.com/randlee/sc-observability/pull/12",
        }),
        ("qa-correction", "qa", {
            "phase": "phase-d", "review.verdict": "PASS", "review.task": "obs-d-35",
            "review.sprint": "d-35", "review.iteration": 2, "review.findings": 0,
            "qa.blocking": 0, "qa.important": 0, "qa.minor": 0, "qa.tested": "all",
            "qa.correction": True, "qa.corrects": "row-1", "vcs.pr.number": 12,
            "pr.url": "https://github.com/randlee/sc-observability/pull/12",
        }),
        ("finding-counts", "finding-counts", {
            "phase": "phase-d", "qa.blocking": 1, "qa.important": 2, "qa.minor": 3,
        }),
    )
    telemetry = _double(tmp_path)
    for name, kind, expected_attrs in cases:
        row = json.loads((FIXTURES / f"{name}.json").read_text())
        reviewer = "sanity-llm" if kind == "sanity" else None
        source = Source(f".sc/{name}.jsonl", kind, "phase-d", reviewer)
        document = map_row(source, row, CONFIG)
        assert document is not None
        assert document["resource"] == {
            "attributes": {"service.name": "sc-observability", "team": "sc-obs"},
            "dropped_attributes_count": 0, "entity_refs": [], "schema_url": None,
        }
        assert document["scope"] == {
            "name": "sc-observability.sanity", "version": None, "attributes": {},
            "dropped_attributes_count": 0, "schema_url": None,
        }
        assert _attrs(document) == expected_attrs
        normalized = build_envelope(document)
        assert isinstance(normalized, Ok), normalized
        envelope = json.loads(normalized.value)
        signal_row = envelope["logs"][0]["record"]
        assert _decoded_attributes(signal_row["attributes"]) == expected_attrs
        assert _decoded_attributes(envelope["logs"][0]["resource"]["attributes"]) == {
            "service.name": "sc-observability", "team": "sc-obs",
        }
        receipt = telemetry.emit(document)
        assert isinstance(receipt, Ok), receipt
        assert receipt.value.record_key == document["record_key"]
        assert receipt.value.signals
        if name == "sanity-current":
            assert document["logs"] == [{"time": "2026-10-01T12:00:00.123456789Z", "body": "PASS", "attributes": expected_attrs}]
            assert document["spans"] == [{
                "name": "sanity.review", "start_time": "2026-10-01T11:59:00.000000000Z",
                "end_time": "2026-10-01T12:00:00.123456789Z", "attributes": expected_attrs,
                "trace_id": document["spans"][0]["trace_id"],
            }]
            assert len(document["spans"][0]["trace_id"]) == 32
        elif name == "sanity-legacy":
            assert document["spans"] == []
            assert "vcs.commit.sha" not in expected_attrs
        elif name == "finding-counts":
            assert [metric["name"] for metric in document["metrics"]] == ["sc.qa.findings.open", "sc.qa.findings.total"]
            assert [metric["data"]["data"]["points"][0]["value"]["data"] for metric in document["metrics"]] == [6, 9]


def test_current_format_nine_digit_timestamp_preserves_nanoseconds() -> None:
    row = json.loads((FIXTURES / "sanity-current.json").read_text())
    document = map_row(Source(".sc/sanity.jsonl", "sanity", "phase-d", "sanity-llm"), row, CONFIG)
    assert document is not None
    assert document["logs"][0]["time"] == "2026-10-01T12:00:00.123456789Z"


def test_llm_and_jev_envelopes_share_trace_and_keep_exact_error_fields(tmp_path: Path) -> None:
    row = json.loads((FIXTURES / "sanity-current.json").read_text())
    row.update({"verdict": "CANNOT_RUN", "findings": None, "error": {"code": "SC_SANITY_TOOL_MISSING", "message": "missing tool"}})
    source = Source(".sc/sanity.jsonl", "sanity", "phase-d")
    llm = map_row(source, row, CONFIG)
    jev = map_row(source, dict(row, reviewer="sanity-jev"), CONFIG)
    assert llm is not None and jev is not None
    assert _attrs(llm) == {
        "phase": "phase-d", "review.reviewer": "sanity-llm", "review.verdict": "CANNOT_RUN",
        "vcs.commit.sha": "a" * 40, "review.task": "obs-d-35", "review.sprint": "d-35",
        "review.iteration": 1, "review.duration_s": 60,
        "vcs.pr.number": 12, "pr.url": "https://github.com/randlee/sc-observability/pull/12",
        "review.error.code": "SC_SANITY_TOOL_MISSING", "review.error.message": "missing tool",
    }
    assert llm["spans"][0]["status"] == {"code": "error", "message": "missing tool"}
    assert llm["spans"][0]["trace_id"] == jev["spans"][0]["trace_id"]
    llm_envelope = build_envelope(llm)
    jev_envelope = build_envelope(jev)
    assert isinstance(llm_envelope, Ok) and isinstance(jev_envelope, Ok)
    llm_trace = json.loads(llm_envelope.value)["spans"][0]["record"]["trace_id"]
    jev_trace = json.loads(jev_envelope.value)["spans"][0]["record"]["trace_id"]
    assert llm_trace == jev_trace
    telemetry = _double(tmp_path)
    for document in (llm, jev):
        normalized = build_envelope(document)
        receipt = telemetry.emit(document)
        assert isinstance(normalized, Ok) and isinstance(receipt, Ok)
        assert receipt.value.record_key == document["record_key"]
    assert llm["spans"][0]["trace_id"] == jev["spans"][0]["trace_id"]


def test_missing_fields_and_absent_pr_template_are_omitted() -> None:
    source = Source(".sc/qa.jsonl", "qa")
    row = {"completed_at": "2026-10-01T12:00:00Z", "verdict": "PASS", "findings": None, "pr_number": 7}
    document = map_row(source, row, {"service": "svc", "team": "team"})
    assert document is not None
    assert document["logs"][0]["attributes"] == {"review.verdict": "PASS", "vcs.pr.number": 7}
    assert map_row(source, {"verdict": "PASS"}, CONFIG) is None
    assert map_row(source, {"completed_at": "2026-10-01T12:00:00Z"}, CONFIG) is None
    assert map_row(source, {"completed_at": "2026-02-30T12:00:00Z", "verdict": "PASS"}, CONFIG) is None
