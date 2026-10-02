"""Durable-store failure and recovery cases through installed `sc-otel`."""
from __future__ import annotations

import json
import socket
from pathlib import Path

import pytest

from conftest import CaptureCollector, GOLDENS, reserve_loopback_sockets, run_cli, run_installed_python


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


def test_reserved_loopback_sockets_are_distinct_and_held() -> None:
    """The harness must not expose a probe-and-close port allocation race."""
    reserved = reserve_loopback_sockets(3)
    try:
        ports = [int(port.getsockname()[1]) for port in reserved]
        assert len(set(ports)) == 3
        for port in ports:
            with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as contender:
                with pytest.raises(OSError):
                    contender.bind(("127.0.0.1", port))
    finally:
        for port in reserved:
            port.close()


def test_offline_cli_admission_records_terminal_delivery(
    installed_artifacts: dict[str, Path], dead_collector_endpoint: str, tmp_path: Path,
) -> None:
    """Exhausting synchronous HTTP retries records a terminal durable failure."""
    config = _config(tmp_path / "telemetry.yaml", dead_collector_endpoint)
    payload = (GOLDENS / "logs/input.json").read_text(encoding="utf-8")
    offline = run_cli(installed_artifacts, "--config", str(config), "emit", "--stdin",
                      cwd=tmp_path, input=payload)
    assert offline.returncode == 7, offline.stdout + offline.stderr
    terminal = json.loads(offline.stdout)
    assert terminal["state"] == "admitted_failed", terminal
    assert terminal["receipt"] is not None, terminal
    assert terminal["flush"]["failed"]["logs"] == 1, terminal


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
    assert emitted.returncode == 7, emitted.stdout + emitted.stderr
    result = json.loads(emitted.stdout)
    assert result["state"] == "admitted_failed", result
    assert result["flush"]["delivered"]["logs"] == 1, result
    assert result["flush"]["failed"]["profiles"] == 1, result
    collector.wait_for("/v1/logs")
    collector.wait_for("/v1development/profiles")
    status = run_cli(installed_artifacts, "--config", str(telemetry_config), "status", cwd=tmp_path)
    assert status.returncode == 0, status.stdout + status.stderr
    report = json.loads(status.stdout)["status"]
    assert report is not None
    assert report["failed"]["profiles"] == 1, report


def test_context_exit_retains_delivery_failure_as_a_tagged_result(
    installed_artifacts: dict[str, Path], dead_collector_endpoint: str, tmp_path: Path,
) -> None:
    """A down collector is a delivery result, never an exception from ``with``."""
    config = _config(tmp_path / "telemetry.yaml", dead_collector_endpoint)
    source = (GOLDENS / "logs/input.json").read_text(encoding="utf-8")
    script = f"""\
import json
import sys
from sc_observability import Ok
from sc_observability.telemetry import Telemetry, TelemetryErr

opened = Telemetry.open(config={str(config)!r})
assert isinstance(opened, Ok), opened
telemetry = opened.value
with telemetry:
    admitted = telemetry.emit(json.load(sys.stdin))
    assert isinstance(admitted, Ok), admitted
assert isinstance(telemetry.last_shutdown, TelemetryErr), telemetry.last_shutdown
assert telemetry.last_shutdown.error.kind == "delivery"
print(telemetry.last_shutdown.error.code)
"""
    exited = run_installed_python(installed_artifacts, script, cwd=tmp_path, input=source)
    assert exited.returncode == 0, exited.stdout + exited.stderr
    assert "DELIVERY" in exited.stdout


def test_killed_python_admission_is_retained_for_recovery(
    installed_artifacts: dict[str, Path], dead_collector_endpoint: str, tmp_path: Path,
) -> None:
    """A process death preserves the submission identity and its recovery record."""
    config = _config(tmp_path / "telemetry.yaml", dead_collector_endpoint)
    source = (GOLDENS / "logs/input.json").read_text(encoding="utf-8")
    script = f"""\
import json
import os
import sys
from sc_observability import Ok
from sc_observability.telemetry import Telemetry

opened = Telemetry.open(config={str(config)!r})
assert isinstance(opened, Ok), opened
admitted = opened.value.emit(json.load(sys.stdin))
assert isinstance(admitted, Ok), admitted
# Deliberately bypass normal cleanup: the next process must own recovery.
print(admitted.value.submission_id, flush=True)
os._exit(0)
"""
    killed = run_installed_python(installed_artifacts, script, cwd=tmp_path, input=source)
    assert killed.returncode == 0, killed.stdout + killed.stderr
    submission_id = killed.stdout.strip()
    assert submission_id, killed.stderr
    recovery = f"""\
import json
import os
from sc_observability import Ok
from sc_observability.telemetry import Telemetry

opened = Telemetry.open(config={str(config)!r})
assert isinstance(opened, Ok), opened
status = opened.value.status(submissions=[{submission_id!r}])
assert isinstance(status, Ok), status
delivery = status.value.submissions[0]
assert delivery.submission_id == {submission_id!r}, delivery
signal, state = delivery.signals[0]
assert signal == "logs", delivery
# A prior process may have claimed its admission before dying, but neither
# pending nor claimed work has been delivered or discarded.
report = dict(state)
assert report["state"] in {{"pending", "claimed"}}, delivery
print(json.dumps(report), flush=True)
# Avoid a second process's shutdown flush: expired-lease takeover is covered
# deterministically by durable::tests::drain::lease_expiry_takeover.
os._exit(0)
"""
    retained = run_installed_python(installed_artifacts, recovery, cwd=tmp_path)
    assert retained.returncode == 0, retained.stdout + retained.stderr
    assert json.loads(retained.stdout)["state"] in {"pending", "claimed"}
