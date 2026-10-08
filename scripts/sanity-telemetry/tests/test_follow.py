"""Importer logic tests.

``RecordingTelemetry`` stands in for ``Telemetry`` so these tests exercise the
importer's checkpoint and failure handling; they do not prove export.  The live
loopback test at the end builds the real client.
"""
from __future__ import annotations

import json
import threading
from http.server import BaseHTTPRequestHandler, HTTPServer
from pathlib import Path
from typing import Iterator

import pytest

from sc_observability import Ok
from sc_observability.telemetry import Telemetry, TelemetryErr, TelemetryFailure

from import_sanity import Importer, Source, _checkpoint_key, _trace_id

EXPORT_FAILED = "SC_OBSERVABILITY_OTLP_EXPORT_FAILED"
INVALID_RECORD = "SC_OBSERVABILITY_OTLP_SYNC_INVALID_RECORD"


def _err(kind: str, code: str) -> TelemetryErr:
    return TelemetryErr(TelemetryFailure(kind, code, f"{kind} failure"))


class RecordingTelemetry:
    """Records every call; answers from ``script`` in order, then ``Ok(None)``."""

    def __init__(self, script: list[object] | None = None) -> None:
        self.calls: list[tuple[str, tuple[object, ...], dict[str, object]]] = []
        self.script = list(script or [])

    def _answer(self, method: str, args: tuple[object, ...], kwargs: dict[str, object]) -> object:
        self.calls.append((method, args, kwargs))
        return self.script.pop(0) if self.script else Ok(None)

    def log(self, *args: object, **kwargs: object) -> object:
        return self._answer("log", args, kwargs)

    def span(self, *args: object, **kwargs: object) -> object:
        return self._answer("span", args, kwargs)

    def metric(self, *args: object, **kwargs: object) -> object:
        return self._answer("metric", args, kwargs)

    def span_traces(self) -> list[object]:
        return [kwargs.get("trace_id") for method, _, kwargs in self.calls if method == "span"]


def _config(root: Path) -> Path:
    config = root / "telemetry.yaml"
    config.write_text(
        "service: test\nteam: team\notlp: {endpoint: 'http://127.0.0.1:4318'}\n"
        "github: {pr_url_template: 'https://example/{pr_number}'}\nsources: []\n"
    )
    return config


def _row(number: int = 1, **extra: object) -> str:
    value: dict[str, object] = {
        "run_id": f"run-{number}", "reviewer": "sanity-llm",
        "completed_at": "2026-10-01T12:00:00Z", "started_at": "2026-10-01T11:59:00Z", "verdict": "PASS",
    }
    value.update(extra)
    return json.dumps(value)


def _importer(
    tmp_path: Path, script: list[object] | None = None, contents: str | None = None
) -> tuple[Importer, Path, Source, RecordingTelemetry]:
    source = tmp_path / "events.jsonl"
    source.write_text(contents if contents is not None else _row() + "\n")
    telemetry = RecordingTelemetry(script)
    importer = Importer(_config(tmp_path), telemetry, tmp_path / "checkpoints.json")  # type: ignore[arg-type]
    return importer, source, Source("events.jsonl", "sanity", "phase-d", "sanity-llm"), telemetry


def test_sanity_row_makes_log_then_span_calls(tmp_path: Path) -> None:
    importer, _, spec, telemetry = _importer(tmp_path)
    assert importer.import_source(spec).exported == 1
    assert [method for method, _, _ in telemetry.calls] == ["log", "span"]
    assert telemetry.calls[0][1] == ("PASS",)
    assert telemetry.span_traces() == [_trace_id("run-1")]


def test_partial_line_not_consumed_until_newline(tmp_path: Path) -> None:
    importer, source, spec, telemetry = _importer(tmp_path, contents=_row())
    assert importer.import_source(spec).exported == 0
    assert telemetry.calls == []
    source.write_text(_row() + "\n")
    assert importer.import_source(spec).exported == 1
    assert telemetry.span_traces() == [_trace_id("run-1")]


def test_repeated_import_with_same_checkpoint_exports_nothing_new(tmp_path: Path) -> None:
    importer, _, spec, telemetry = _importer(tmp_path)
    assert importer.import_source(spec).exported == 1
    calls = len(telemetry.calls)
    repeated = importer.import_source(spec)
    assert repeated.exported == 0 and repeated.skipped_invalid == 0 and repeated.failures == []
    assert len(telemetry.calls) == calls


def test_truncation_restarts_without_loss(tmp_path: Path) -> None:
    importer, source, spec, telemetry = _importer(tmp_path, contents=_row(1) + "\n" + _row(2) + "\n")
    assert importer.import_source(spec).exported == 2
    source.write_text(_row(1) + "\n")
    truncated = importer.import_source(spec)
    assert truncated.exported == 1 and truncated.failures == []
    source.write_text(_row(1) + "\n" + _row(3) + "\n")
    resumed = importer.import_source(spec)
    assert resumed.exported == 1 and resumed.failures == []
    assert telemetry.span_traces() == [_trace_id(f"run-{n}") for n in (1, 2, 1, 3)]


def test_rotation_restarts_without_loss(tmp_path: Path) -> None:
    contents = _row(1) + "\n" + _row(2) + "\n"
    importer, source, spec, telemetry = _importer(tmp_path, contents=contents)
    assert importer.import_source(spec).exported == 2
    replacement = tmp_path / "replacement.jsonl"
    replacement.write_text(contents)
    replacement.replace(source)
    rotated = importer.import_source(spec)
    assert rotated.exported == 2 and rotated.failures == []
    assert telemetry.span_traces() == [_trace_id(f"run-{n}") for n in (1, 2, 1, 2)]
    checkpoint = json.loads(importer.checkpoint_path.read_text())[_checkpoint_key(spec)]
    assert checkpoint["inode"] == source.stat().st_ino and checkpoint["offset"] == source.stat().st_size


@pytest.mark.parametrize("kind", ["export", "internal"])
def test_checkpoint_not_advanced_on_export_or_internal_failure(tmp_path: Path, kind: str) -> None:
    code = EXPORT_FAILED if kind == "export" else "SC_OBSERVABILITY_BINDING_INTERNAL"
    importer, source, spec, telemetry = _importer(tmp_path, contents=_row(1) + "\n")
    assert importer.import_source(spec).exported == 1
    checkpoint_before = importer.checkpoint_path.read_bytes()
    with source.open("a") as handle:
        handle.write(_row(2) + "\n" + _row(3) + "\n")
    telemetry.script = [Ok(None), _err(kind, code)]
    report = importer.import_source(spec)
    assert report.failures == [(kind, code)]
    assert report.exported == 0 and report.skipped_invalid == 0
    assert importer.checkpoint_path.read_bytes() == checkpoint_before
    # The source stopped at row 2: row 3 was never sent.
    assert telemetry.span_traces() == [_trace_id("run-1"), _trace_id("run-2")]
    retried = importer.import_source(spec)
    assert retried.exported == 2 and retried.failures == []
    assert telemetry.span_traces()[-2:] == [_trace_id("run-2"), _trace_id("run-3")]


def test_telemetry_exception_and_unexpected_result_stop_without_checkpoint(tmp_path: Path) -> None:
    importer, _, spec, telemetry = _importer(tmp_path)

    def explode(*args: object, **kwargs: object) -> object:
        raise RuntimeError("boom")

    telemetry.log = explode  # type: ignore[method-assign]
    assert importer.import_source(spec).failures == [("telemetry", "EXPORT_EXCEPTION")]
    assert not importer.checkpoint_path.exists()

    importer, _, spec, telemetry = _importer(tmp_path, script=["not a result"])
    assert importer.import_source(spec).failures == [("telemetry", "UNEXPECTED_RESULT")]
    assert not importer.checkpoint_path.exists()


def test_invalid_payload_poison_row_is_counted_and_following_row_is_exported(tmp_path: Path) -> None:
    importer, source, spec, telemetry = _importer(
        tmp_path,
        script=[_err("validation", INVALID_RECORD)],
        contents=_row(1, task=["not", "a", "scalar"]) + "\n" + _row(2) + "\n",
    )
    report = importer.import_source(spec)
    assert report.skipped_invalid == 1
    assert report.exported == 1
    assert report.failures == []
    assert telemetry.span_traces() == [_trace_id("run-2")]
    checkpoint = json.loads(importer.checkpoint_path.read_text())[_checkpoint_key(spec)]
    assert checkpoint["offset"] == source.stat().st_size


def test_same_path_sources_keep_kind_and_reviewer_checkpoints_distinct(tmp_path: Path) -> None:
    importer, source_path, _, _ = _importer(tmp_path)
    sanity = Source("events.jsonl", "sanity", reviewer="sanity-llm")
    qa = Source("events.jsonl", "qa", reviewer=None)
    assert importer.import_source(sanity).exported == 1
    assert importer.import_source(qa).exported == 1
    state = json.loads(importer.checkpoint_path.read_text())
    assert _checkpoint_key(sanity) in state and _checkpoint_key(qa) in state
    assert state[_checkpoint_key(sanity)]["offset"] == source_path.stat().st_size
    assert state[_checkpoint_key(qa)]["offset"] == source_path.stat().st_size


def test_checkpoint_write_failure_keeps_previous_checkpoint(tmp_path: Path, monkeypatch) -> None:
    importer, _, spec, _ = _importer(tmp_path)
    assert importer.import_source(spec).exported == 1
    before = importer.checkpoint_path.read_bytes()

    def fail_replace(source: str, target: Path) -> None:
        raise OSError("no replace")

    monkeypatch.setattr("import_sanity.os.replace", fail_replace)
    (tmp_path / "events.jsonl").write_text(_row(2) + "\n")
    report = importer.import_source(spec)
    assert report.failures == [("checkpoint", "CHECKPOINT_WRITE")]
    assert importer.checkpoint_path.read_bytes() == before


def test_malformed_config_and_checkpoint_are_counted(tmp_path: Path) -> None:
    invalid_config = tmp_path / "invalid.yaml"
    invalid_config.write_text("sources: [\n")
    invalid_importer = Importer(invalid_config, RecordingTelemetry(), tmp_path / "bad-checkpoint.json")  # type: ignore[arg-type]
    assert invalid_importer.run_once().failures == [("config", "CONFIG_LOAD")]

    importer, _, spec, telemetry = _importer(tmp_path)
    importer.checkpoint_path.write_text('{"broken": []}')
    assert importer.import_source(spec).failures == [("checkpoint", "CHECKPOINT_INVALID")]
    assert telemetry.calls == []


def test_source_fstat_failure_is_counted_without_raising(tmp_path: Path, monkeypatch) -> None:
    import import_sanity

    importer, _, spec, _ = _importer(tmp_path)
    monkeypatch.setattr(import_sanity.os, "fstat", lambda _fd: (_ for _ in ()).throw(OSError("stat failed")))
    assert importer.import_source(spec).failures == [("source", "SOURCE_READ")]


def test_invalid_utf8_line_is_counted_and_later_rows_continue(tmp_path: Path) -> None:
    importer, source, spec, _ = _importer(tmp_path)
    source.write_bytes(b"\xff\n" + (_row(2) + "\n").encode())
    report = importer.import_source(spec)
    assert report.skipped_invalid == 1 and report.exported == 1 and report.failures == []


def test_committed_sc_config_resolves_state_and_sources_once(tmp_path: Path) -> None:
    config_directory = tmp_path / ".sc"
    source = config_directory / "sanity-log" / "events.jsonl"
    source.parent.mkdir(parents=True)
    source.write_text(_row() + "\n")
    config = config_directory / "telemetry.yaml"
    config.write_text("service: test\nteam: team\nsources: []\n")
    importer = Importer(config, RecordingTelemetry())  # type: ignore[arg-type]
    spec = Source(".sc/sanity-log/events.jsonl", "sanity", "phase-d", "sanity-llm")

    assert importer.checkpoint_path == config_directory / "telemetry-state/checkpoints.json"
    assert importer.import_source(spec).exported == 1
    assert importer.checkpoint_path.exists()
    assert not (config_directory / ".sc").exists()


def test_main_import_builds_telemetry_from_config(tmp_path: Path, monkeypatch, capsys) -> None:
    import import_sanity

    config = _config(tmp_path)
    built: list[tuple[object, dict[str, object]]] = []
    telemetry = RecordingTelemetry()

    def fake_telemetry(endpoint: object = None, **kwargs: object) -> RecordingTelemetry:
        built.append((endpoint, kwargs))
        return telemetry

    class FakeImporter:
        def __init__(self, config_path, opened, **kwargs):
            assert config_path == config and opened is telemetry

        def run_once(self):
            return import_sanity.Report(exported=3, skipped_invalid=1)

    monkeypatch.setattr(import_sanity, "Telemetry", fake_telemetry)
    monkeypatch.setattr(import_sanity, "Importer", FakeImporter)
    assert import_sanity.main(["import", "--config", str(config)]) == 0
    assert built == [("http://127.0.0.1:4318", {"service_name": "test"})]
    assert json.loads(capsys.readouterr().out) == {"exported": 3, "skipped_invalid": 1, "failures": []}


def test_main_reports_counted_failure_with_status_one(tmp_path: Path, monkeypatch, capsys) -> None:
    import import_sanity

    config = _config(tmp_path)

    class FakeImporter:
        def __init__(self, config_path, opened, **kwargs):
            pass

        def run_once(self):
            return import_sanity.Report(failures=[("export", EXPORT_FAILED)])

    monkeypatch.setattr(import_sanity, "Telemetry", lambda *args, **kwargs: RecordingTelemetry())
    monkeypatch.setattr(import_sanity, "Importer", FakeImporter)
    assert import_sanity.main(["import", "--config", str(config)]) == 1
    assert json.loads(capsys.readouterr().out) == {
        "exported": 0, "skipped_invalid": 0, "failures": [["export", EXPORT_FAILED]],
    }


def test_main_config_and_usage_failures_return_status_two(tmp_path: Path, monkeypatch, capsys) -> None:
    import import_sanity

    missing = tmp_path / "missing.yaml"
    assert import_sanity.main(["import", "--config", str(missing)]) == 2
    assert json.loads(capsys.readouterr().out) == {
        "exported": 0, "skipped_invalid": 0, "failures": [["config", "CONFIG_LOAD"]],
    }
    bad_endpoint = tmp_path / "bad-endpoint.yaml"
    bad_endpoint.write_text("otlp: {endpoint: 4318}\nsources: []\n")
    assert import_sanity.main(["import", "--config", str(bad_endpoint)]) == 2
    assert json.loads(capsys.readouterr().out)["failures"] == [["config", "CONFIG_LOAD"]]
    with pytest.raises(SystemExit) as error:
        import_sanity.main(["invalid-mode"])
    assert error.value.code == 2


def test_main_follow_repeats_without_real_sleep(tmp_path: Path, monkeypatch, capsys) -> None:
    import import_sanity

    config = _config(tmp_path)
    calls = []

    class FakeImporter:
        def __init__(self, config_path, opened, **kwargs):
            pass

        def run_once(self):
            calls.append("run")
            return import_sanity.Report()

    sleeps = []

    def interrupt_after_two_polls(interval):
        sleeps.append(interval)
        if len(sleeps) == 2:
            raise KeyboardInterrupt

    monkeypatch.setattr(import_sanity, "Telemetry", lambda *args, **kwargs: RecordingTelemetry())
    monkeypatch.setattr(import_sanity, "Importer", FakeImporter)
    monkeypatch.setattr(import_sanity, "sleep", interrupt_after_two_polls)
    assert import_sanity.main(["follow", "--config", str(config)]) == 0
    assert len(calls) == 2 and sleeps == [1.0, 1.0]
    assert len(capsys.readouterr().out.splitlines()) == 2


class _Collector(BaseHTTPRequestHandler):
    def do_POST(self) -> None:  # noqa: N802 - http.server naming
        length = int(self.headers.get("Content-Length", "0"))
        self.server.requests.append((self.path, self.rfile.read(length)))  # type: ignore[attr-defined]
        self.send_response(200)
        self.send_header("Content-Type", "application/x-protobuf")
        self.send_header("Content-Length", "0")
        self.end_headers()

    def log_message(self, format: str, *args: object) -> None:
        pass


@pytest.fixture
def collector() -> Iterator[HTTPServer]:
    server = HTTPServer(("127.0.0.1", 0), _Collector)
    server.requests = []  # type: ignore[attr-defined]
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    try:
        yield server
    finally:
        server.shutdown()
        thread.join()
        server.server_close()


def test_live_sanity_row_exports_one_log_and_one_span(tmp_path: Path, collector: HTTPServer) -> None:
    host, port = collector.server_address[:2]
    telemetry = Telemetry(endpoint=f"http://{host}:{port}", timeout_s=10.0, service_name="sanity-telemetry-test")
    source = tmp_path / "events.jsonl"
    source.write_text(_row(1) + "\n")
    importer = Importer(_config(tmp_path), telemetry, tmp_path / "checkpoints.json")
    report = importer.import_source(Source("events.jsonl", "sanity", "phase-d", "sanity-llm"))
    assert report.failures == [] and report.exported == 1 and report.skipped_invalid == 0
    paths = [path for path, _ in collector.requests]  # type: ignore[attr-defined]
    assert sorted(paths) == ["/v1/logs", "/v1/traces"]
    assert all(body for _, body in collector.requests)  # type: ignore[attr-defined]
