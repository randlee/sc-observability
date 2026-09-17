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

        debug = logger.elevate_level("debug")
        assert isinstance(debug, Ok)
        assert debug.value.kind == "changed"
        assert debug.value.current.effective_level == "debug"
        assert debug.value.current.level_revision == 1

        repeated = logger.elevate_level("debug", "user_request")
        assert isinstance(repeated, Ok)
        assert repeated.value.kind == "unchanged"
        assert repeated.value.state.level_revision == 1

        below_baseline = logger.elevate_level("off", "diagnostic_session")
        assert isinstance(below_baseline, Err)
        assert below_baseline.error.kind == "below_baseline"
        assert below_baseline.error.requested == "off"
        assert below_baseline.error.configured == "info"

        reset = logger.reset_level("application")
        assert isinstance(reset, Ok)
        assert reset.value.kind == "changed"
        assert reset.value.current.effective_level == "info"
        assert reset.value.current.level_revision == 2
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
