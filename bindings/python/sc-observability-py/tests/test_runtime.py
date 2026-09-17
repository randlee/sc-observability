"""Runtime fixtures that require a wheel/source installation of ``_native``.

The normal facade unit suite deliberately runs without the extension.  CI sets
``SC_OBSERVABILITY_RUNTIME_TEST=1`` only after installing the freshly built
wheel, so these tests prove the public Python API reaches the real Rust backend.
"""
from __future__ import annotations

import os
from concurrent.futures import ThreadPoolExecutor
from threading import Barrier
from pathlib import Path
import subprocess
import sys

import pytest

pytestmark = pytest.mark.skipif(
    os.environ.get("SC_OBSERVABILITY_RUNTIME_TEST") != "1",
    reason="requires an installed sc-observability wheel",
)

from sc_observability import Err, LogEvent, Logger, LoggerConfig, LogQuery, Ok, create_logger


def _owned(root: Path, service: str) -> Logger:
    created = create_logger(LoggerConfig(service=service, log_root=str(root)))
    assert isinstance(created, Ok), created
    return created.value


def _event(action: str, value: int = 1) -> LogEvent:
    return LogEvent(
        level="info",
        target="python.runtime",
        action=action,
        fields={"value": value},
    )


def _assert_closed(result: object) -> None:
    assert isinstance(result, Err)
    assert result.error.code == "SC_OBSERVABILITY_BINDING_CLOSED"


def test_owned_instances_are_isolated_and_keep_lifecycle_results(tmp_path: Path) -> None:
    first = _owned(tmp_path / "first", "python-runtime-first")
    second = _owned(tmp_path / "second", "python-runtime-second")

    assert isinstance(first.log(_event("first-only")), Ok)
    assert isinstance(second.log(_event("second-only")), Ok)
    assert isinstance(first.flush(), Ok)
    assert isinstance(second.flush(), Ok)

    first_snapshot = first.query(LogQuery(action="first-only"))
    second_snapshot = second.query(LogQuery(action="second-only"))
    assert isinstance(first_snapshot, Ok)
    assert isinstance(second_snapshot, Ok)
    assert [entry.action for entry in first_snapshot.value.events] == ["first-only"]
    assert [entry.action for entry in second_snapshot.value.events] == ["second-only"]

    assert isinstance(first.elevate_level("debug"), Ok)
    assert isinstance(first.reset_level(), Ok)
    assert isinstance(first.shutdown(), Ok)
    assert isinstance(first.shutdown(), Ok)
    assert isinstance(first.wait_stopped(), Ok)
    assert isinstance(first.health(), Ok)
    _assert_closed(first.log(_event("after-close")))
    _assert_closed(first.query(LogQuery()))
    _assert_closed(first.flush())

    assert isinstance(second.health(), Ok)
    assert isinstance(second.shutdown(), Ok)


def test_parallel_owned_producers_do_not_report_dispatch_contention(tmp_path: Path) -> None:
    logger = _owned(tmp_path / "parallel", "python-runtime-parallel")
    try:
        start = Barrier(32)

        def emit(index: int) -> object:
            start.wait(timeout=5)
            return logger.log(_event(f"parallel-{index}", index))

        with ThreadPoolExecutor(max_workers=32) as workers:
            results = list(workers.map(emit, range(32)))
        dispatch_failures = [
            result
            for result in results
            if isinstance(result, Err)
            and result.error.code == "SC_OBSERVABILITY_BINDING_DISPATCH_FULL"
        ]
        assert not dispatch_failures
        assert all(isinstance(result, Ok) for result in results)
        assert isinstance(logger.flush(), Ok)
    finally:
        assert isinstance(logger.shutdown(), Ok)


def test_owned_level_changes_are_revised_reset_and_retained(tmp_path: Path) -> None:
    logger = _owned(tmp_path / "levels", "python-runtime-levels")
    try:
        initial = logger.health()
        assert isinstance(initial, Ok)
        assert initial.value.level_state.effective_level == "info"
        assert initial.value.level_state.level_revision == 0

        for revision, level in enumerate(("debug", "trace"), start=1):
            changed = logger.elevate_level(level)
            assert isinstance(changed, Ok)
            assert changed.value.kind == "changed"
            assert changed.value.current.effective_level == level
            assert changed.value.current.level_revision == revision

        repeated = logger.elevate_level("trace", "user_request")
        assert isinstance(repeated, Ok)
        assert repeated.value.kind == "unchanged"
        assert repeated.value.state.level_revision == 2

        for level in ("warn", "error", "off"):
            below_baseline = logger.elevate_level(level, "diagnostic_session")
            assert isinstance(below_baseline, Err)
            assert below_baseline.error.kind == "below_baseline"
            assert below_baseline.error.requested == level
            assert below_baseline.error.configured == "info"

        reset = logger.reset_level("application")
        assert isinstance(reset, Ok)
        assert reset.value.kind == "changed"
        assert reset.value.current.effective_level == "info"
        assert reset.value.current.level_revision == 3
        assert isinstance(logger.log(_event("accepted-after-reset")), Ok)
        assert isinstance(logger.flush(), Ok)
    finally:
        assert isinstance(logger.shutdown(), Ok)

    off_created = create_logger(
        LoggerConfig(service="python-runtime-levels-off", log_root=str(tmp_path / "levels-off"), level="off")
    )
    assert isinstance(off_created, Ok)
    off_logger = off_created.value
    try:
        off_health = off_logger.health()
        assert isinstance(off_health, Ok)
        assert off_health.value.level_state.effective_level == "off"
        filtered = off_logger.log(_event("filtered-at-off"))
        assert isinstance(filtered, Ok)
        assert filtered.value.kind == "filtered"
        elevated = off_logger.elevate_level("info")
        assert isinstance(elevated, Ok)
        assert elevated.value.kind == "changed"
        assert elevated.value.current.effective_level == "info"
        restored = off_logger.reset_level()
        assert isinstance(restored, Ok)
        assert restored.value.kind == "changed"
        assert restored.value.current.effective_level == "off"
    finally:
        assert isinstance(off_logger.shutdown(), Ok)


def test_public_operation_race_keeps_every_result_tagged(tmp_path: Path) -> None:
    logger = _owned(tmp_path / "race", "python-runtime-race")
    start = Barrier(4)

    def emit() -> object:
        start.wait(timeout=5)
        return logger.log(_event("race-log"))

    def query() -> object:
        start.wait(timeout=5)
        return logger.query(LogQuery(action="race-log"))

    def flush() -> object:
        start.wait(timeout=5)
        return logger.flush(timeout_ms=10)

    def shutdown() -> object:
        start.wait(timeout=5)
        return logger.shutdown(timeout_ms=2_000)

    with ThreadPoolExecutor(max_workers=4) as workers:
        results = list(workers.map(lambda call: call(), (emit, query, flush, shutdown)))

    # Lifecycle races may close admission or expose an in-flight native slot,
    # but the facade must always return a schema-tagged Result rather than an
    # exception or a partially decoded value.
    assert all(isinstance(result, (Ok, Err)) for result in results)
    assert isinstance(logger.wait_stopped(), Ok)
    retained = logger.health()
    assert isinstance(retained, Ok)


def test_owned_gc_and_interpreter_teardown_do_not_hang(tmp_path: Path) -> None:
    """A clean installed interpreter may release an unclosed owned handle."""
    root = repr(str(tmp_path / "gc"))
    program = f"""
import gc
from sc_observability import LoggerConfig, Ok, create_logger
created = create_logger(LoggerConfig(service='python-runtime-gc', log_root={root!s}))
assert isinstance(created, Ok), created
del created
gc.collect()
"""
    completed = subprocess.run(
        [sys.executable, "-c", program],
        check=False,
        capture_output=True,
        text=True,
        timeout=15,
    )
    assert completed.returncode == 0, completed.stderr


def test_zero_deadline_shutdown_retains_the_late_completion(tmp_path: Path) -> None:
    logger = _owned(tmp_path / "late-shutdown", "python-runtime-late-shutdown")
    assert isinstance(logger.log(_event("before-zero-deadline")), Ok)
    first = logger.shutdown(timeout_ms=0)
    # An observation deadline does not cancel the one native shutdown. Fast
    # machines may finish before zero is observed; otherwise its tagged timeout
    # is resolved by the retained wait operation below.
    assert isinstance(first, (Ok, Err))
    if isinstance(first, Err):
        assert first.error.kind == "timeout"
    final = logger.wait_stopped(timeout_ms=2_000)
    assert isinstance(final, Ok)
    repeated = logger.shutdown(timeout_ms=2_000)
    assert isinstance(repeated, Ok)
    assert isinstance(logger.health(), Ok)


def test_real_factory_filesystem_failure_is_tagged(tmp_path: Path) -> None:
    root = tmp_path / "sink-fault"
    service = "python-runtime-sink-fault"
    # The JSONL sink opens lazily. A directory at the active-file path forces
    # a real writer failure after public factory construction succeeds.
    (root / "logs" / f"{service}.log.jsonl").mkdir(parents=True)
    created = create_logger(
        LoggerConfig(service=service, log_root=str(root))
    )
    assert isinstance(created, Ok)
    logger = created.value
    try:
        assert isinstance(logger.log(_event("filesystem-fault")), Ok)
        flushed = logger.flush()
        assert isinstance(flushed, Ok)
        health = logger.health()
        assert isinstance(health, Ok)
        assert health.value.logging.state in ("degraded_dropping", "unavailable")
        assert health.value.logging.last_error is not None
    finally:
        assert isinstance(logger.shutdown(), (Ok, Err))

