"""Pinned desktop-viewer readback through installed telemetry front ends."""
from __future__ import annotations

import json
import time
from pathlib import Path

from conftest import GOLDENS, rpc, run_cli, run_installed_python


def _viewer_config(path: Path, endpoint: str, store_name: str) -> Path:
    path.write_text(
        "\n".join(("service: telemetry-e2e-viewer", "otlp:", f"  endpoint: {endpoint}",
                   "  timeout_ms: 1000", "store:", f"  path: {store_name}",
                   "  max_bytes: 10485760", "")), encoding="utf-8")
    return path


def _wait_for(viewer: dict[str, str], method: str, params: list[object], needle: str) -> object:
    deadline = time.monotonic() + 15
    while time.monotonic() < deadline:
        result = rpc(viewer["rpc"], method, params)
        if needle in json.dumps(result):
            return result
        time.sleep(0.25)
    raise AssertionError(f"{method} did not return {needle!r}")


def _python_emit_script(config: Path) -> str:
    return f"""\
import json
import sys
from sc_observability import Ok
from sc_observability.telemetry import Telemetry

opened = Telemetry.open(config={str(config)!r})
assert isinstance(opened, Ok), opened
with opened.value as telemetry:
    submitted = telemetry.emit(json.load(sys.stdin))
    assert isinstance(submitted, Ok), submitted
    delivered = telemetry.flush_submission(submitted.value.submission_id, timeout_s=10)
    assert isinstance(delivered, Ok), delivered
"""


def test_installed_cli_readback_preserves_log_span_and_metric_values(
    installed_artifacts: dict[str, Path], pinned_viewer: dict[str, str], tmp_path: Path,
) -> None:
    """Viewer evidence is limited to the forms it can query (not profiles)."""
    payload = {"version": 1}
    for signal, fixture in (("logs", "logs"), ("spans", "traces"), ("metrics", "metric_gauge")):
        payload[signal] = json.loads((GOLDENS / fixture / "input.json").read_text())[signal]
    source = json.dumps(payload)
    python_config = _viewer_config(tmp_path / "python-viewer.yaml", pinned_viewer["otlp"], "viewer-python.sqlite")
    python = run_installed_python(installed_artifacts, _python_emit_script(python_config), cwd=tmp_path, input=source)
    assert python.returncode == 0, python.stdout + python.stderr
    config = _viewer_config(tmp_path / "cli-viewer.yaml", pinned_viewer["otlp"], "viewer-cli.sqlite")
    emitted = run_cli(installed_artifacts, "--config", str(config), "emit", "--stdin",
                      cwd=tmp_path, input=source)
    assert emitted.returncode == 0, emitted.stdout + emitted.stderr

    low, high = "-1000000000", "1893456000000000000"
    logs = _wait_for(pinned_viewer, "searchLogs", [low, high], "hello")
    log_id = next(row["id"] for row in logs if row.get("bodyPreview") == "hello")
    detail = rpc(pinned_viewer["rpc"], "getLog", [log_id])
    assert detail["body"] == "hello"
    assert "telemetry-e2e-viewer" in json.dumps(detail)
    spans = _wait_for(pinned_viewer, "searchSpans", ["0123456789abcdef0123456789abcdef"], "check")
    assert "check" in json.dumps(spans)
    summaries = _wait_for(pinned_viewer, "searchMetricSummaries", [low, high], "requests")
    stream = next(row.get("id") or row.get("streamID") or row.get("streamId")
                  for row in summaries if row.get("name") == "requests")
    metric = rpc(pinned_viewer["rpc"], "getMetric", [stream, low, high])
    assert "5" in json.dumps(metric)
