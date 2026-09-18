"""Private feature-wheel runtime proofs; never part of production discovery."""
from __future__ import annotations

import json
import os
import sys
import time
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path
from threading import Event
from typing import Callable

import pytest

pytestmark = pytest.mark.skipif(
    os.environ.get("SC_OBSERVABILITY_RUNTIME_TEST") != "1",
    reason="requires the B.4 feature-wheel validation environment",
)

from sc_observability import Err, LogEvent, Logger, LoggerConfig, LogQuery, Ok, create_logger, get_host_logger
from sc_observability import _native



def _wait_for_native(predicate: Callable[[], bool], description: str, timeout: float = 2.0) -> None:
    # Native hooks expose a nonblocking snapshot, not a Python notification API.
    # Retain a bounded poll with a single monotonic deadline and named evidence.
    started = time.monotonic()
    deadline = started + timeout
    pause = Event()
    attempts = 0
    while True:
        attempts += 1
        observed = predicate()
        if observed:
            return
        remaining = deadline - time.monotonic()
        assert remaining > 0, (
            f"{description} did not become ready within {timeout:.3f}s; "
            f"last native state={observed!r}, observations={attempts}"
        )
        pause.wait(min(0.01, remaining))


def test_native_wait_failure_is_bounded_and_names_the_missing_signal() -> None:
    started = time.monotonic()
    with pytest.raises(AssertionError, match="fixture writer.*last native state=False"):
        _wait_for_native(lambda: False, "fixture writer", timeout=0.01)
    assert time.monotonic() - started < 2.0, "native snapshot timeout exceeded its bound"


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
        _wait_for_native(
            lambda: json.loads(native._test_blocked_writer_entered())["value"],
            "owned retained writer entry",
        )
        assert isinstance(logger.health(), Ok)
        assert isinstance(logger.query(LogQuery(action="held-sink")), (Ok, Err))
        blocked_flush = logger.flush(timeout_ms=10)
        assert isinstance(blocked_flush, Err)
        assert blocked_flush.error.kind == "timeout"
        assert json.loads(native._test_release_blocked_writer())["kind"] == "ok"
        assert isinstance(logger.shutdown(timeout_ms=2_000), Ok)
    finally:
        # Release before shutdown even if an assertion above fails.
        native._test_release_blocked_writer()
        assert isinstance(logger.shutdown(timeout_ms=2_000), Ok)


def test_owned_fault_fixture_releases_its_worker_after_assertion(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch,
) -> None:
    original_wait = _wait_for_native

    def fail_after_entry(predicate: Callable[[], bool], description: str) -> None:
        original_wait(predicate, description)
        assert False, "injected assertion after real writer entry"

    monkeypatch.setattr(sys.modules[__name__], "_wait_for_native", fail_after_entry)
    with pytest.raises(AssertionError, match="injected assertion after real writer entry"):
        test_real_retained_sink_blocks_while_python_operations_progress(tmp_path)


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
        try:
            _wait_for_native(_native._test_blocked_host_entered, "attached blocked host entry")
            assert isinstance(attached.value.health(), Ok)
            assert isinstance(attached.value.query(LogQuery()), (Ok, Err))
            assert isinstance(attached.value.flush(), (Ok, Err))
        finally:
            # Executor.__exit__ joins its worker: unblock it before that join,
            # including assertion/timeout unwind paths.
            _native._test_release_blocked_host()
        assert isinstance(blocked_log.result(timeout=2), Ok)
