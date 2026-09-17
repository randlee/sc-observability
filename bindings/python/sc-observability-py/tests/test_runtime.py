"""Runtime fixtures that require a wheel/source installation of ``_native``.

The normal facade unit suite deliberately runs without the extension.  CI sets
``SC_OBSERVABILITY_RUNTIME_TEST=1`` only after installing the freshly built
wheel, so these tests prove the public Python API reaches the real Rust backend.
"""
from __future__ import annotations

import os
from concurrent.futures import ThreadPoolExecutor
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
        with ThreadPoolExecutor(max_workers=32) as workers:
            results = list(workers.map(lambda index: logger.log(_event(f"parallel-{index}", index)), range(32)))
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
