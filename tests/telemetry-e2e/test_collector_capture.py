"""Collector capture, deliberately separate from pinned-viewer assertions."""
from __future__ import annotations

import json
from pathlib import Path

from conftest import GOLDENS, run_cli, run_installed_python


def _python_emit_script(config: Path) -> str:
    return f"""\
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


def _valid(name: str) -> str:
    return (GOLDENS / name / "input.json").read_text(encoding="utf-8")


def _config_with_store(config: Path, store: str) -> Path:
    text = config.read_text(encoding="utf-8").replace("path: store.sqlite", f"path: {store}")
    result = config.parent / f"{store}.yaml"
    result.write_text(text, encoding="utf-8")
    return result


def test_installed_frontends_export_viewer_unsupported_representations(
    installed_artifacts: dict[str, Path], telemetry_config: Path, collector: object, tmp_path: Path,
) -> None:
    """Capture is the authority for profiles, summary, exponential histogram, and exemplars.

    This intentionally makes no viewer claim: pinned viewer v0.5.0 cannot query
    every representation.  The test checks the real HTTP paths and nonempty
    OTLP/JSON records emitted by the installed wheel and CLI.
    """
    from conftest import CaptureCollector
    assert isinstance(collector, CaptureCollector)
    cases = (
        ("metric_exponential_histogram", "/v1/metrics"),
        ("metric_summary", "/v1/metrics"),
        ("profiles", "/v1development/profiles"),
    )
    for fixture, path in cases:
        payload = _valid(fixture)
        collector.clear()
        python = run_installed_python(installed_artifacts, _python_emit_script(telemetry_config), cwd=tmp_path, input=payload)
        assert python.returncode == 0, python.stderr
        python_records = collector.wait_for(path)
        assert all(record for record in python_records), fixture

        collector.clear()
        # Independent installed front ends need independent stores: the same
        # canonical records intentionally retain the same record keys.
        cli_config = _config_with_store(telemetry_config, f"cli-{fixture}.sqlite")
        cli = run_cli(installed_artifacts, "--config", str(cli_config), "emit", "--stdin", cwd=tmp_path, input=payload)
        assert cli.returncode == 0, cli.stdout + cli.stderr
        cli_records = collector.wait_for(path)
        assert all(record for record in cli_records), fixture
