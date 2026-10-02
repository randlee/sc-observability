from __future__ import annotations

import json
from pathlib import Path

from sc_observability import Ok
from sc_observability.telemetry import Telemetry

from import_sanity import Importer, Source, _checkpoint_key


def _config(root: Path, source: Path) -> Path:
    config = root / "telemetry.yaml"
    config.write_text("service: test\nteam: team\ngithub: {pr_url_template: 'https://example/{pr_number}'}\nsources: []\n")
    return config


def _double(root: Path, script: str | None = None) -> Telemetry:
    result = Telemetry._with_test_double(store_path=root / "store", endpoint="http://127.0.0.1:4318", service_name="d35-test", script_json=script)
    assert isinstance(result, Ok)
    return result.value


def _row(number: int = 1, **extra: object) -> str:
    value: dict[str, object] = {
        "run_id": f"run-{number}", "reviewer": "sanity-llm",
        "completed_at": "2026-10-01T12:00:00Z", "started_at": "2026-10-01T11:59:00Z", "verdict": "PASS",
    }
    value.update(extra)
    return json.dumps(value)


def _importer(tmp_path: Path, script: str | None = None, contents: str | None = None) -> tuple[Importer, Path, Source]:
    source = tmp_path / "events.jsonl"
    source.write_text(contents if contents is not None else _row() + "\n")
    config = _config(tmp_path, source)
    return Importer(config, _double(tmp_path, script), tmp_path / "checkpoints.json"), source, Source("events.jsonl", "sanity", "phase-d", "sanity-llm")


def test_partial_line_not_consumed_until_newline(tmp_path: Path) -> None:
    importer, source, spec = _importer(tmp_path, contents=_row())
    assert importer.import_source(spec).admitted == 0
    source.write_text(_row() + "\n")
    assert importer.import_source(spec).admitted == 1


def test_truncation_restarts_without_loss(tmp_path: Path) -> None:
    importer, source, spec = _importer(tmp_path, contents=_row(1) + "\n" + _row(2) + "\n")
    assert importer.import_source(spec).admitted == 2
    source.write_text(_row(1) + "\n")
    truncated = importer.import_source(spec)
    assert truncated.admitted == 0 and truncated.duplicates == 1
    source.write_text(_row(1) + "\n" + _row(3) + "\n")
    resumed = importer.import_source(spec)
    assert resumed.admitted == 1 and resumed.duplicates == 0


def test_rotation_restarts_without_loss(tmp_path: Path) -> None:
    contents = _row(1) + "\n" + _row(2) + "\n"
    importer, source, spec = _importer(tmp_path, contents=contents)
    assert importer.import_source(spec).admitted == 2
    replacement = tmp_path / "replacement.jsonl"
    replacement.write_text(contents)
    replacement.replace(source)
    rotated = importer.import_source(spec)
    assert rotated.admitted == 0 and rotated.duplicates == 2


def test_repeated_import_admits_nothing_new(tmp_path: Path) -> None:
    importer, _, spec = _importer(tmp_path)
    first = importer.import_source(spec)
    repeated_importer = Importer(
        importer.config_path, importer.telemetry, tmp_path / "independent-checkpoints.json", config=importer.config
    )
    repeated = repeated_importer.import_source(spec)
    assert first.admitted == 1 and first.duplicates == 0
    assert repeated.admitted == 0 and repeated.duplicates == 1


def test_checkpoint_written_only_after_receipt(tmp_path: Path) -> None:
    script = '{"admissions":[{"outcome":"admit"},{"outcome":"reject","kind":"disk_bound_exceeded"}]}'
    importer, source, spec = _importer(tmp_path, script=script, contents=_row(1) + "\n")
    assert importer.import_source(spec).admitted == 1
    checkpoint_before = importer.checkpoint_path.read_bytes()
    with source.open("a") as handle:
        handle.write(_row(2) + "\n")
    report = importer.import_source(spec)
    assert report.failures and report.failures[0][0] == "admission"
    assert importer.checkpoint_path.read_bytes() == checkpoint_before


def test_invalid_payload_poison_row_is_counted_and_following_row_is_admitted(tmp_path: Path) -> None:
    importer, source, spec = _importer(
        tmp_path,
        contents=_row(1, run_id="poison-" + "x" * 2048) + "\n" + _row(2) + "\n",
    )
    report = importer.import_source(spec)
    assert report.skipped_invalid == 1
    assert report.admitted == 1
    assert report.failures == []
    checkpoint = json.loads(importer.checkpoint_path.read_text())[_checkpoint_key(spec)]
    assert checkpoint["offset"] == source.stat().st_size


def test_same_path_sources_keep_kind_and_reviewer_checkpoints_distinct(tmp_path: Path) -> None:
    importer, source_path, _ = _importer(tmp_path)
    sanity = Source("events.jsonl", "sanity", reviewer="sanity-llm")
    qa = Source("events.jsonl", "qa", reviewer=None)
    assert importer.import_source(sanity).admitted == 1
    assert importer.import_source(qa).admitted == 1
    state = json.loads(importer.checkpoint_path.read_text())
    assert _checkpoint_key(sanity) in state and _checkpoint_key(qa) in state
    assert state[_checkpoint_key(sanity)]["offset"] == source_path.stat().st_size
    assert state[_checkpoint_key(qa)]["offset"] == source_path.stat().st_size


def test_checkpoint_write_failure_keeps_previous_checkpoint(tmp_path: Path, monkeypatch) -> None:
    importer, _, spec = _importer(tmp_path)
    assert importer.import_source(spec).admitted == 1
    before = importer.checkpoint_path.read_bytes()

    def fail_replace(source: str, target: Path) -> None:
        raise OSError("no replace")

    monkeypatch.setattr("import_sanity.os.replace", fail_replace)
    (tmp_path / "events.jsonl").write_text(_row(2) + "\n")
    report = importer.import_source(spec)
    assert report.failures == [("checkpoint", "CHECKPOINT_WRITE")]
    assert importer.checkpoint_path.read_bytes() == before


def test_malformed_config_and_checkpoint_are_counted(tmp_path: Path) -> None:
    telemetry = _double(tmp_path)
    invalid_config = tmp_path / "invalid.yaml"
    invalid_config.write_text("sources: [\n")
    invalid_importer = Importer(invalid_config, telemetry, tmp_path / "bad-checkpoint.json")
    assert invalid_importer.run_once().failures == [("config", "CONFIG_LOAD")]

    importer, _, spec = _importer(tmp_path)
    importer.checkpoint_path.write_text('{"broken": []}')
    assert importer.import_source(spec).failures == [("checkpoint", "CHECKPOINT_INVALID")]


def test_source_fstat_failure_is_counted_without_raising(tmp_path: Path, monkeypatch) -> None:
    import import_sanity

    importer, _, spec = _importer(tmp_path)
    monkeypatch.setattr(import_sanity.os, "fstat", lambda _fd: (_ for _ in ()).throw(OSError("stat failed")))
    assert importer.import_source(spec).failures == [("source", "SOURCE_READ")]


def test_invalid_utf8_line_is_counted_and_later_rows_continue(tmp_path: Path) -> None:
    importer, source, spec = _importer(tmp_path)
    source.write_bytes(b"\xff\n" + (_row(2) + "\n").encode())
    report = importer.import_source(spec)
    assert report.skipped_invalid == 1 and report.admitted == 1 and report.failures == []


def test_committed_sc_config_resolves_state_and_sources_once(tmp_path: Path) -> None:
    config_directory = tmp_path / ".sc"
    source = config_directory / "sanity-log" / "events.jsonl"
    source.parent.mkdir(parents=True)
    source.write_text(_row() + "\n")
    config = config_directory / "telemetry.yaml"
    config.write_text("service: test\nteam: team\nsources: []\n")
    importer = Importer(config, _double(tmp_path))
    spec = Source(".sc/sanity-log/events.jsonl", "sanity", "phase-d", "sanity-llm")

    assert importer.checkpoint_path == config_directory / "telemetry-state/checkpoints.json"
    assert importer.import_source(spec).admitted == 1
    assert importer.checkpoint_path.exists()
    assert not (config_directory / ".sc").exists()


def test_main_import_status_and_final_flush(tmp_path: Path, monkeypatch) -> None:
    import import_sanity

    config = _config(tmp_path, tmp_path / "events.jsonl")

    class FakeTelemetry:
        flushed = False

        def flush(self):
            self.flushed = True
            return Ok(None)

    telemetry = FakeTelemetry()

    class FakeImporter:
        def __init__(self, config_path, opened, **kwargs):
            assert config_path == config and opened is telemetry

        def run_once(self):
            return import_sanity.Report()

    monkeypatch.setattr(import_sanity.Telemetry, "open", classmethod(lambda cls, config: Ok(telemetry)))
    monkeypatch.setattr(import_sanity, "Importer", FakeImporter)
    assert import_sanity.main(["import", "--config", str(config)]) == 0
    assert telemetry.flushed


def test_main_reports_counted_failure_with_status_one(tmp_path: Path, monkeypatch, capsys) -> None:
    import import_sanity

    config = _config(tmp_path, tmp_path / "events.jsonl")
    telemetry = type("FakeTelemetry", (), {"flush": lambda self: Ok(None)})()

    class FakeImporter:
        def __init__(self, config_path, opened, **kwargs):
            pass

        def run_once(self):
            return import_sanity.Report(failures=[("source", "SOURCE_READ")])

    monkeypatch.setattr(import_sanity.Telemetry, "open", classmethod(lambda cls, config: Ok(telemetry)))
    monkeypatch.setattr(import_sanity, "Importer", FakeImporter)
    assert import_sanity.main(["import", "--config", str(config)]) == 1
    assert json.loads(capsys.readouterr().out)["failures"] == [["source", "SOURCE_READ"]]


def test_main_config_and_usage_failures_return_status_two(tmp_path: Path, monkeypatch, capsys) -> None:
    import import_sanity

    missing = tmp_path / "missing.yaml"
    assert import_sanity.main(["import", "--config", str(missing)]) == 2
    assert json.loads(capsys.readouterr().out)["failures"] == [["config", "CONFIG_LOAD"]]
    try:
        import_sanity.main(["invalid-mode"])
    except SystemExit as error:
        assert error.code == 2
    else:
        raise AssertionError("argparse accepted an invalid mode")


def test_main_follow_repeats_without_real_sleep(tmp_path: Path, monkeypatch, capsys) -> None:
    import import_sanity

    config = _config(tmp_path, tmp_path / "events.jsonl")
    telemetry = type("FakeTelemetry", (), {})()
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

    monkeypatch.setattr(import_sanity.Telemetry, "open", classmethod(lambda cls, config: Ok(telemetry)))
    monkeypatch.setattr(import_sanity, "Importer", FakeImporter)
    monkeypatch.setattr(import_sanity, "sleep", interrupt_after_two_polls)
    assert import_sanity.main(["follow", "--config", str(config)]) == 0
    assert len(calls) == 2 and sleeps == [1.0, 1.0]
    assert len(capsys.readouterr().out.splitlines()) == 2
