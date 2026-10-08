from __future__ import annotations

import json
from pathlib import Path

from import_sanity import Call, Source, _trace_id, map_row


FIXTURES = Path(__file__).parent / "fixtures"
CONFIG = {
    "service": "sc-observability",
    "team": "sc-obs",
    "github": {"pr_url_template": "https://github.com/randlee/sc-observability/pull/{pr_number}"},
}
PR = {"vcs.pr.number": 12, "pr.url": "https://github.com/randlee/sc-observability/pull/12"}


def _log(calls: list[Call]) -> Call:
    assert calls[0].method == "log"
    return calls[0]


def test_real_format_fixtures_map_to_exact_calls() -> None:
    cases = (
        ("sanity-current", "sanity", "2026-10-01T12:00:00.123456789Z", {
            "team": "sc-obs", "phase": "phase-d", "review.reviewer": "sanity-llm", "review.verdict": "PASS",
            "vcs.commit.sha": "a" * 40, "review.task": "obs-d-35", "review.sprint": "d-35",
            "review.iteration": 1, "review.duration_s": 60, "review.findings": 0, **PR,
        }),
        ("sanity-legacy", "sanity", "2026-10-01T12:00:00.000000000Z", {
            "team": "sc-obs", "phase": "phase-d", "review.reviewer": "sanity-llm", "review.verdict": "FAIL",
            "review.task": "obs-d-35", "review.sprint": "d-35", "review.iteration": 1,
            "review.duration": "1m", "review.findings": 1, **PR,
        }),
        ("qa", "qa", "2026-10-01T12:00:00.000000000Z", {
            "team": "sc-obs", "phase": "phase-d", "review.verdict": "FAIL", "review.task": "obs-d-35",
            "review.sprint": "d-35", "review.iteration": 1, "review.duration": "1m",
            "review.findings": 6, "qa.blocking": 1, "qa.important": 2, "qa.minor": 3,
            "qa.tested": "all", **PR,
        }),
        ("qa-correction", "qa", "2026-10-01T12:01:00.000000000Z", {
            "team": "sc-obs", "phase": "phase-d", "review.verdict": "PASS", "review.task": "obs-d-35",
            "review.sprint": "d-35", "review.iteration": 2, "review.findings": 0,
            "qa.blocking": 0, "qa.important": 0, "qa.minor": 0, "qa.tested": "all",
            "qa.correction": True, "qa.corrects": "row-1", **PR,
        }),
        ("finding-counts", "finding-counts", "2026-10-01T12:00:00.000000000Z", {
            "team": "sc-obs", "phase": "phase-d", "qa.blocking": 1, "qa.important": 2, "qa.minor": 3,
        }),
    )
    for name, kind, time, expected_attrs in cases:
        row = json.loads((FIXTURES / f"{name}.json").read_text())
        reviewer = "sanity-llm" if kind == "sanity" else None
        calls = map_row(Source(f".sc/{name}.jsonl", kind, "phase-d", reviewer), row, CONFIG)
        assert calls is not None
        body = "finding-counts" if kind == "finding-counts" else row["verdict"]
        assert _log(calls) == Call("log", (body,), {"attributes": {**expected_attrs, "event.time": time}})
        if name == "sanity-current":
            assert calls[1:] == [Call("span", ("sanity.review",), {
                "start_time_unix_nano": 1_790_855_940_000_000_000,
                "end_time_unix_nano": 1_790_856_000_123_456_789,
                "attributes": expected_attrs,
                "trace_id": _trace_id("run-pair"),
            })]
            assert len(calls[1].kwargs["trace_id"]) == 32
        elif name == "finding-counts":
            assert calls[1:] == [
                Call("metric", ("sc.qa.findings.open", "gauge", 6.0), {"attributes": expected_attrs}),
                Call("metric", ("sc.qa.findings.total", "gauge", 9.0), {"attributes": expected_attrs}),
            ]
        else:
            assert calls[1:] == []
            if name == "sanity-legacy":
                assert "vcs.commit.sha" not in expected_attrs


def test_current_format_nine_digit_timestamp_preserves_nanoseconds() -> None:
    row = json.loads((FIXTURES / "sanity-current.json").read_text())
    calls = map_row(Source(".sc/sanity.jsonl", "sanity", "phase-d", "sanity-llm"), row, CONFIG)
    assert calls is not None
    assert calls[0].kwargs["attributes"]["event.time"] == "2026-10-01T12:00:00.123456789Z"
    assert calls[1].kwargs["end_time_unix_nano"] % 1_000_000_000 == 123_456_789


def test_llm_and_jev_calls_share_trace_and_keep_exact_error_fields() -> None:
    row = json.loads((FIXTURES / "sanity-current.json").read_text())
    row.update({"verdict": "CANNOT_RUN", "findings": None, "error": {"code": "SC_SANITY_TOOL_MISSING", "message": "missing tool"}})
    source = Source(".sc/sanity.jsonl", "sanity", "phase-d")
    llm = map_row(source, row, CONFIG)
    jev = map_row(source, dict(row, reviewer="sanity-jev"), CONFIG)
    assert llm is not None and jev is not None
    expected = {
        "team": "sc-obs", "phase": "phase-d", "review.reviewer": "sanity-llm", "review.verdict": "CANNOT_RUN",
        "vcs.commit.sha": "a" * 40, "review.task": "obs-d-35", "review.sprint": "d-35",
        "review.iteration": 1, "review.duration_s": 60, **PR,
        "review.error.code": "SC_SANITY_TOOL_MISSING", "review.error.message": "missing tool",
    }
    assert llm[0].kwargs["attributes"] == {**expected, "event.time": "2026-10-01T12:00:00.123456789Z"}
    assert llm[1].method == "span"
    assert llm[1].kwargs["ok"] is False and llm[1].kwargs["error"] == "missing tool"
    assert llm[1].kwargs["attributes"] == expected
    assert jev[1].kwargs["attributes"]["review.reviewer"] == "sanity-jev"
    assert llm[1].kwargs["trace_id"] == jev[1].kwargs["trace_id"] == _trace_id("run-pair")


def test_span_without_run_id_or_error_leaves_trace_and_status_to_client() -> None:
    row = json.loads((FIXTURES / "sanity-current.json").read_text())
    del row["run_id"]
    calls = map_row(Source(".sc/sanity.jsonl", "sanity", "phase-d"), row, CONFIG)
    assert calls is not None and calls[1].method == "span"
    assert not {"trace_id", "ok", "error"} & set(calls[1].kwargs)
    assert _trace_id("") is None and _trace_id(7) is None


def test_missing_fields_and_absent_pr_template_are_omitted() -> None:
    source = Source(".sc/qa.jsonl", "qa")
    row = {"completed_at": "2026-10-01T12:00:00Z", "verdict": "PASS", "findings": None, "pr_number": 7}
    calls = map_row(source, row, {"service": "svc"})
    assert calls == [Call("log", ("PASS",), {"attributes": {
        "review.verdict": "PASS", "vcs.pr.number": 7, "event.time": "2026-10-01T12:00:00.000000000Z",
    }})]
    assert map_row(source, {"verdict": "PASS"}, CONFIG) is None
    assert map_row(source, {"completed_at": "2026-10-01T12:00:00Z"}, CONFIG) is None
    assert map_row(source, {"completed_at": "2026-02-30T12:00:00Z", "verdict": "PASS"}, CONFIG) is None
