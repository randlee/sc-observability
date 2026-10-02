from __future__ import annotations

import json
from pathlib import Path

from sc_observability import Ok
from sc_observability.telemetry import Telemetry, TelemetryErr

from import_sanity import Importer, Source


def _config(root: Path, source: Path) -> Path:
    config = root / "telemetry.yaml"
    config.write_text("service: test\nteam: team\ngithub: {pr_url_template: 'https://example/{pr_number}'}\nsources: []\n")
    return config


def _double(root: Path, script: str | None = None) -> Telemetry:
    result = Telemetry._with_test_double(store_path=root / "store", endpoint="http://127.0.0.1:4318", service_name="d35-test", script_json=script)
    assert isinstance(result, Ok)
    return result.value


def _row(number: int = 1) -> str:
    return json.dumps({"run_id": f"run-{number}", "reviewer": "sanity-llm", "completed_at": "2026-10-01T12:00:00Z", "started_at": "2026-10-01T11:59:00Z", "verdict": "PASS"})


def _importer(tmp_path: Path, script: str | None = None) -> tuple[Importer, Path, Source]:
    source = tmp_path / "events.jsonl"; source.write_text(_row() + "\n")
    return Importer(_config(tmp_path, source), _double(tmp_path, script), tmp_path / "checkpoints.json"), source, Source("events.jsonl", "sanity", "phase-d", "sanity-llm")


def test_partial_line_not_consumed_until_newline(tmp_path: Path) -> None:
    importer, source, spec = _importer(tmp_path); source.write_text(_row())
    assert importer.import_source(spec).admitted == 0
    source.write_text(_row() + "\n")
    assert importer.import_source(spec).admitted == 1


def test_truncation_and_rotation_restart_without_loss(tmp_path: Path) -> None:
    importer, source, spec = _importer(tmp_path); assert importer.import_source(spec).admitted == 1
    source.write_text(_row(2) + "\n")  # truncation resets the byte offset
    assert importer.import_source(spec).admitted == 1
    replacement = tmp_path / "replacement.jsonl"; replacement.write_text(_row(3) + "\n")
    replacement.replace(source)  # inode replacement resets the byte offset
    assert importer.import_source(spec).admitted == 1


def test_repeated_import_consumes_nothing_new(tmp_path: Path) -> None:
    importer, _, spec = _importer(tmp_path)
    assert importer.import_source(spec).admitted == 1
    assert importer.import_source(spec).admitted == 0


def test_checkpoint_written_only_after_admission_receipt(tmp_path: Path) -> None:
    importer, _, spec = _importer(tmp_path, '{"admissions":[{"outcome":"reject","kind":"closed"}]}')
    report = importer.import_source(spec)
    assert report.failures and report.failures[0][0] == "admission"
    assert not importer.checkpoint_path.exists()
