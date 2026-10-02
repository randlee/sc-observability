"""Durable-store failure and recovery cases through installed `sc-otel`."""
from __future__ import annotations

import json
import time
from pathlib import Path

from conftest import CaptureCollector, GOLDENS, free_port, run_cli


def _config(path: Path, endpoint: str) -> Path:
    path.write_text(
        "\n".join((
            "service: telemetry-e2e-recovery",
            "otlp:", f"  endpoint: {endpoint}", "  timeout_ms: 100",
            "store:", "  path: shared.sqlite", "  max_bytes: 10485760", "",
        )),
        encoding="utf-8",
    )
    return path


def test_offline_cli_admission_is_delivered_by_later_flush(
    installed_artifacts: dict[str, Path], tmp_path: Path,
) -> None:
    """A failed delivery remains durable and a later process can take the lease."""
    port = free_port()
    config = _config(tmp_path / "telemetry.yaml", f"http://127.0.0.1:{port}")
    payload = (GOLDENS / "logs/input.json").read_text(encoding="utf-8")
    offline = run_cli(installed_artifacts, "--config", str(config), "emit", "--stdin", cwd=tmp_path, input=payload)
    assert offline.returncode == 6, offline.stdout + offline.stderr
    admitted = json.loads(offline.stdout)
    assert admitted["receipt"] is not None, admitted

    collector = CaptureCollector(port)
    collector.start()
    try:
        # The durable worker's retry schedule is intentional.  A later process
        # takes the lease only when the first retry becomes eligible.
        recovered = None
        deadline = time.monotonic() + 10
        while time.monotonic() < deadline:
            recovered = run_cli(installed_artifacts, "--config", str(config), "flush", "--timeout", "1", cwd=tmp_path)
            if recovered.returncode == 0:
                break
            assert recovered.returncode == 6, recovered.stdout + recovered.stderr
            time.sleep(0.25)
        assert recovered is not None and recovered.returncode == 0, recovered.stdout + recovered.stderr
        collector.wait_for("/v1/logs")
    finally:
        collector.stop()


def test_partial_signal_failure_retains_the_failed_signal(
    installed_artifacts: dict[str, Path], telemetry_config: Path, collector: CaptureCollector, tmp_path: Path,
) -> None:
    """A profile 503 must not erase already-delivered log records or hide status."""
    collector.statuses["/v1development/profiles"] = 503
    payload = {
        "version": 1,
        "logs": json.loads((GOLDENS / "logs/input.json").read_text())["logs"],
        "profiles": json.loads((GOLDENS / "profiles/input.json").read_text())["profiles"],
    }
    emitted = run_cli(installed_artifacts, "--config", str(telemetry_config), "emit", "--stdin",
                      cwd=tmp_path, input=json.dumps(payload))
    assert emitted.returncode == 6, emitted.stdout + emitted.stderr
    collector.wait_for("/v1/logs")
    collector.wait_for("/v1development/profiles")
    status = run_cli(installed_artifacts, "--config", str(telemetry_config), "status", cwd=tmp_path)
    assert status.returncode == 6, status.stdout + status.stderr
    report = json.loads(status.stdout)["status"]
    assert report is not None
