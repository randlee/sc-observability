"""Real extension/backend receipt and asyncio conformance (no backend mocks)."""
from __future__ import annotations

import asyncio
from concurrent.futures import ThreadPoolExecutor
import gc
from pathlib import Path
import threading
import tracemalloc

from sc_observability import Err, LogEvent, LoggerConfig, LogQuery, Ok, create_logger


def test_owned_receipts_ignore_repeat_filter_and_survive_shutdown(tmp_path: Path) -> None:
    made = create_logger(LoggerConfig(service="async-owned", log_root=str(tmp_path)))
    assert isinstance(made, Ok)
    logger = made.value
    logger.log(LogEvent(level="info", target="async.runtime", action="ignored.log"))
    logger.submit(LogEvent(level="info", target="async.runtime", action="ignored.receipt"))
    receipt = logger.submit(LogEvent(level="info", target="async.runtime", action="saved", fields={"exact": 2**64 - 1}))
    assert isinstance(receipt, Ok)
    filtered = logger.submit(LogEvent(level="debug", target="async.runtime", action="filtered"))
    assert isinstance(filtered, Ok) and filtered.value.state().admission.kind == "filtered"
    assert isinstance(asyncio.run(logger.flush_async(), debug=True), Ok)
    records = logger.query(LogQuery())
    assert isinstance(records, Ok)
    assert {record.action for record in records.value.events} == {"ignored.log", "ignored.receipt", "saved"}
    assert isinstance(logger.shutdown(), Ok)
    assert isinstance(logger.wait_stopped(0), Ok)
    for saved in (receipt.value, filtered.value):
        for timeout in (0, 2000, 60_000):
            assert asyncio.run(saved.wait(timeout), debug=True) == Ok(saved.state().admission)
    closed = logger.submit(LogEvent(level="info", target="async.runtime", action="closed"))
    assert isinstance(closed, Err) and closed.error.kind == "closed"
    stopped = asyncio.run(logger.flush_async(), debug=True)
    assert isinstance(stopped, Err) and stopped.error.kind == "closed"


def test_32_synchronized_native_submissions_and_async_producers(tmp_path: Path) -> None:
    made = create_logger(LoggerConfig(service="async-producers", log_root=str(tmp_path)))
    assert isinstance(made, Ok)
    logger = made.value
    barrier = threading.Barrier(32)
    def submit(index: int) -> object:
        barrier.wait(timeout=10)
        return logger.submit(LogEvent(level="info", target="async.runtime", action=f"thread.{index}"))
    with ThreadPoolExecutor(max_workers=32) as workers:
        results = tuple(workers.map(submit, range(32)))
    assert all(isinstance(result, Ok) for result in results)
    async def run() -> None:
        ready = asyncio.Event()
        async def producer(index: int) -> None:
            await ready.wait()
            submitted = logger.submit(LogEvent(level="info", target="async.runtime", action=f"task.{index}"))
            assert isinstance(submitted, Ok)
            assert isinstance(await submitted.value.wait(0), Ok)
        producers = [asyncio.create_task(producer(index)) for index in range(32)]
        ready.set()
        await asyncio.gather(*producers)
        assert isinstance(await logger.flush_async(), Ok)
    asyncio.run(run(), debug=True)
    records = logger.query(LogQuery(limit=100))
    assert isinstance(records, Ok) and len(records.value.events) == 64
    assert isinstance(logger.shutdown(), Ok)


def test_receipt_retention_is_caller_owned_and_invalid_timeout_starts_no_flush(tmp_path: Path) -> None:
    made = create_logger(LoggerConfig(service="async-memory", log_root=str(tmp_path), level="off"))
    assert isinstance(made, Ok)
    logger = made.value
    for invalid in (True, -1, 60_001, 1.1, float("nan")):
        result = asyncio.run(logger.flush_async(invalid), debug=True)
        assert isinstance(result, Err) and result.error.kind == "validation"
    tracemalloc.start()
    gc.collect()
    before = tracemalloc.get_traced_memory()[0]
    receipts = [logger.submit(LogEvent(level="info", target="async.runtime", action="memory")) for _ in range(2000)]
    assert all(isinstance(value, Ok) for value in receipts)
    held = tracemalloc.get_traced_memory()[0]
    del receipts
    gc.collect()
    after = tracemalloc.get_traced_memory()[0]
    tracemalloc.stop()
    assert held > before
    assert after - before < (held - before) // 2
    assert isinstance(logger.shutdown(), Ok)


def test_interpreter_exit_with_native_writer_and_flush_held(tmp_path: Path) -> None:
    import os
    import subprocess
    import sys
    script = r'''
import asyncio, sys
from sc_observability import Err, Ok, LogEvent, LoggerConfig, create_logger
from sc_observability.async_logging import _pools
made = create_logger(LoggerConfig(service="async-teardown", log_root=sys.argv[1], enable_console_sink=True))
assert isinstance(made, Ok), made
logger = made.value
# The parent deliberately never drains stdout while this interpreter lives.
# More than a pipe buffer is admitted, holding the actual native console writer.
for index in range(200):
    result = logger.log(LogEvent(level="info", target="async.teardown", action="held", message="x" * 4096))
    assert isinstance(result, Ok), result
loop = asyncio.new_event_loop()
wait = logger.flush_async(60000)
loop.call_soon(wait.send, None)
loop.run_until_complete(asyncio.sleep(0))
assert len(_pools[logger._native.observer_key()].observers) == 1
loop.close()
sys.stderr.write("B6_TEARDOWN_HELD\n")
sys.stderr.flush()
# Module/observer teardown drops only observation. No native completion calls
# Python or waits for this closed loop during interpreter finalization.
'''
    environment = dict(os.environ, PYTHONASYNCIODEBUG="1", PYTHONWARNINGS="error")
    child = subprocess.Popen([sys.executable, "-I", "-W", "error", "-c", script, str(tmp_path)],
                             stdout=subprocess.PIPE, stderr=subprocess.PIPE, env=environment)
    try:
        child.wait(timeout=15)
        assert child.stderr is not None
        errors = child.stderr.read().decode()
        assert child.returncode == 0, errors
        assert errors == "B6_TEARDOWN_HELD\n", errors
    finally:
        if child.poll() is None:
            child.kill()
            child.wait()
        if child.stdout is not None:
            child.stdout.close()
        if child.stderr is not None:
            child.stderr.close()


def test_submit_snapshots_context_and_nested_values_at_call_boundary(tmp_path: Path) -> None:
    from sc_observability.context import bind_context
    made = create_logger(LoggerConfig(service="async-context", log_root=str(tmp_path)))
    assert isinstance(made, Ok)
    logger = made.value
    async def produce(index: int) -> None:
        context = bind_context(request_id=f"task-{index}", correlation_id="mixed-async")
        assert isinstance(context, Ok)
        assert isinstance(context.value.enter(), Ok)
        payload = {"nested": [index]}
        submitted = logger.submit(LogEvent(level="info", target="async.runtime", action=f"snapshot.{index}", fields=payload))
        assert isinstance(submitted, Ok)
        payload["nested"].append(999)
        assert isinstance(context.value.close(), Ok)
        await asyncio.sleep(0)
        assert isinstance(await submitted.value.wait(0), Ok)
    async def run() -> None:
        await asyncio.gather(*(produce(index) for index in range(32)))
        assert isinstance(await logger.flush_async(), Ok)
    asyncio.run(run(), debug=True)
    records = logger.query(LogQuery(limit=100))
    assert isinstance(records, Ok) and len(records.value.events) == 32
    for event in records.value.events:
        index = int(event.action.split(".")[-1])
        assert event.request_id == f"task-{index}"
        assert event.correlation_id == "mixed-async"
        assert len(event.fields["nested"].value) == 1
        assert event.fields["nested"].value[0].value == index
    assert isinstance(logger.shutdown(), Ok)
