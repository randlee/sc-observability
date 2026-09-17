"""Private feature-wheel runtime proofs; never part of production discovery."""
from __future__ import annotations

import json
import os
import time
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

import pytest

pytestmark = pytest.mark.skipif(
    os.environ.get("SC_OBSERVABILITY_RUNTIME_TEST") != "1",
    reason="requires the B.4 feature-wheel validation environment",
)

from sc_observability import Err, LogEvent, Logger, LoggerConfig, LogQuery, Ok, create_logger, get_host_logger
from sc_observability import _native


def _owned(root: Path, service: str) -> Logger:
    created = create_logger(LoggerConfig(service=service, log_root=str(root)))
    assert isinstance(created, Ok), created
    return created.value


def _event(action: str) -> LogEvent:
    return LogEvent(level="info", target="python.runtime", action=action, fields={"value": 1})


def test_real_revision_exhaustion_retains_the_native_state(tmp_path: Path) -> None:
    logger = _owned(tmp_path / "revision-exhaustion", "python-runtime-revision-exhaustion")
    forced = json.loads(logger._native._test_force_revision_exhaustion())
    assert forced["kind"] == "ok"
    try:
        overflow = logger.elevate_level("debug")
        assert isinstance(overflow, Err)
        assert overflow.error.kind == "unavailable"
        assert overflow.error.code == "SC_OBSERVABILITY_LEVEL_REVISION_EXHAUSTED"
        assert overflow.error.remediation.kind == "not_recoverable"
        retained = logger.health()
        assert isinstance(retained, Ok)
        assert retained.value.level_state.level_revision == 2**64 - 1
        assert retained.value.level_state.effective_level == "info"
    finally:
        assert isinstance(logger.shutdown(), Ok)


def test_real_retained_sink_blocks_while_python_operations_progress(tmp_path: Path) -> None:
    native, created = _native._test_create_blocking_owned(json.dumps({
        "service": "python-runtime-held-writer", "log_root": str(tmp_path / "held-writer"),
        "enable_file_sink": True, "enable_console_sink": False,
    }))
    assert json.loads(created)["kind"] == "ok"
    assert native is not None
    logger = Logger(native)
    try:
        assert isinstance(logger.log(_event("held-sink")), Ok)
        deadline = time.monotonic() + 2
        while not json.loads(native._test_blocked_writer_entered())["value"] and time.monotonic() < deadline:
            time.sleep(0.01)
        assert json.loads(native._test_blocked_writer_entered())["value"]
        assert isinstance(logger.health(), Ok)
        assert isinstance(logger.query(LogQuery(action="held-sink")), (Ok, Err))
        blocked_flush = logger.flush(timeout_ms=10)
        assert isinstance(blocked_flush, Err)
        assert blocked_flush.error.kind == "timeout"
        assert json.loads(native._test_release_blocked_writer())["kind"] == "ok"
        assert isinstance(logger.shutdown(timeout_ms=2_000), Ok)
    finally:
        assert isinstance(logger.shutdown(timeout_ms=2_000), Ok)


def test_private_ci_fault_hook_preserves_tagged_native_results(tmp_path: Path) -> None:
    forced = _native._test_force_failure
    forced("create_owned")
    failed_factory = create_logger(LoggerConfig("python-runtime-forced-factory", str(tmp_path / "factory")))
    assert isinstance(failed_factory, Err)
    assert failed_factory.error.kind == "internal"
    forced("get_installed_host_logger")
    assert isinstance(get_host_logger(), Err)
    forced(None)
    logger = _owned(tmp_path / "operations", "python-runtime-forced-operations")
    operations = (("log", lambda: logger.log(_event("forced-log"))), ("query", lambda: logger.query(LogQuery())), ("health", logger.health), ("flush", logger.flush), ("wait_stopped", logger.wait_stopped), ("elevate_level", lambda: logger.elevate_level("debug")), ("reset_level", logger.reset_level), ("shutdown", logger.shutdown))
    try:
        for operation, call in operations:
            forced(operation)
            result = call()
            assert isinstance(result, Err), operation
            assert result.error.kind == "internal"
    finally:
        forced(None)
        assert isinstance(logger.shutdown(), Ok)


def test_private_ci_fault_hook_covers_attached_native_results(tmp_path: Path) -> None:
    installed = json.loads(_native._test_install_owned_host(json.dumps({"service": "python-runtime-forced-attached", "log_root": str(tmp_path / "attached")}), True))
    assert installed["kind"] == "ok"
    attached = get_host_logger()
    assert isinstance(attached, Ok)
    forced = _native._test_force_failure
    operations = (
        ("log", lambda: attached.value.log(_event("forced-attached-log"))),
        ("query", lambda: attached.value.query(LogQuery())),
        ("health", attached.value.health),
        ("flush", attached.value.flush),
    )
    for operation, call in operations:
        forced(operation)
        result = call()
        assert isinstance(result, Err), operation
        assert result.error.kind == "internal"
    forced(None)
    with ThreadPoolExecutor(max_workers=1) as workers:
        blocked_log = workers.submit(attached.value.log, _event("blocked-attached-log"))
        deadline = time.monotonic() + 2
        while not _native._test_blocked_host_entered():
            assert time.monotonic() < deadline
            time.sleep(0.005)
        assert isinstance(attached.value.health(), Ok)
        assert isinstance(attached.value.query(LogQuery()), (Ok, Err))
        assert isinstance(attached.value.flush(), (Ok, Err))
        _native._test_release_blocked_host()
        assert isinstance(blocked_log.result(timeout=2), Ok)
