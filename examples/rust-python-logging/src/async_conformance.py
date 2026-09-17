import asyncio
import time
from sc_observability import Err, Ok, LogEvent, LoggerConfig, create_logger, get_host_logger

made = create_logger(LoggerConfig(service="b6-owned", log_root=root, enable_console_sink=True)) if mode == "owned" else get_host_logger()
assert isinstance(made, Ok), made
logger = made.value
saved = logger.submit(LogEvent(level="info", target="async.embed", action="held"))
assert isinstance(saved, Ok), saved

async def run():
    ticks = 0
    running = True
    async def heartbeat():
        nonlocal ticks
        while running:
            ticks += 1
            await asyncio.sleep(0)
    heartbeat_task = asyncio.create_task(heartbeat())
    first = asyncio.create_task(logger.flush_async(2000))
    await asyncio.sleep(0.010)
    assert ticks >= 3, ticks
    overlap = await logger.flush_async(2000)
    assert isinstance(overlap, Err) and overlap.error.code == "SC_OBSERVABILITY_BINDING_FLUSH_IN_PROGRESS", overlap
    first.cancel()
    cancelled = await first
    assert isinstance(cancelled, Err) and cancelled.error.kind == "cancelled", cancelled
    # Cancellation must leave the native slot occupied while writer is held.
    still_held = await logger.flush_async(1)
    assert isinstance(still_held, Err) and still_held.error.code == "SC_OBSERVABILITY_BINDING_FLUSH_IN_PROGRESS", still_held
    for _ in range(32):
        submitted = logger.submit(LogEvent(level="info", target="async.embed", action="concurrent"))
        assert isinstance(submitted, Ok), submitted
        assert isinstance(await submitted.value.wait(0), Ok)
    assert ticks >= 3
    for _ in range(4096):
        saturated = logger.submit(LogEvent(level="info", target="async.embed", action="queue.fill"))
        if isinstance(saturated, Err):
            assert saturated.error.kind == "queue_full", saturated
            assert saturated.error.code != "SC_OBSERVABILITY_BINDING_DISPATCH_FULL"
            break
    else:
        raise AssertionError("held real writer did not reach its bounded queue")
    # Deliberately ignore a known full-queue Result: no warning or exception.
    logger.submit(LogEvent(level="info", target="async.embed", action="queue.ignore"))
    before_release = controls.flush_calls()
    controls.release()
    # Observe completion using native state of new explicit requests; no library
    # retry exists. Bounded fixture polling is only waiting for slot availability.
    deadline = time.monotonic() + 5
    while True:
        completed = await logger.flush_async(1000)
        if isinstance(completed, Ok):
            break
        assert completed.error.kind == "queue_full", completed
        assert time.monotonic() < deadline
        await asyncio.sleep(0.001)
    assert isinstance(await saved.value.wait(), Ok)
    running = False
    await heartbeat_task
    if mode != "owned":
        assert before_release == 3, before_release

asyncio.run(run(), debug=True)
# Real native timeout and late slot release: observation expiry cannot retry.
controls.hold()
assert isinstance(logger.submit(LogEvent(level="info", target="async.embed", action="timeout.hold")), Ok)
async def timeout_case():
    timed = await logger.flush_async(2)
    assert isinstance(timed, Err) and timed.error.kind == "timeout", timed
    await asyncio.sleep(0.005)
    overlap = await logger.flush_async(2)
    assert isinstance(overlap, Err) and overlap.error.kind == "queue_full", overlap
    allowed = {"SC_OBSERVABILITY_BINDING_FLUSH_IN_PROGRESS"}
    if mode == "bridge":
        allowed.add("SC_OBSERVABILITY_LOG_FLUSH_IN_PROGRESS")
    assert overlap.error.code in allowed, overlap
    controls.release()
    deadline = time.monotonic() + 5
    while True:
        flushed = await logger.flush_async(1000)
        if isinstance(flushed, Ok):
            break
        assert flushed.error.kind == "queue_full", flushed
        assert time.monotonic() < deadline
        await asyncio.sleep(0.001)
asyncio.run(timeout_case(), debug=True)

# Close a loop with its real flush timer active. A later loop reclaims the
# reservation; releasing the writer lets the original native operation finish.
from sc_observability.async_logging import _pools
controls.hold()
assert isinstance(logger.submit(LogEvent(level="info", target="async.embed", action="loop.hold")), Ok)
loop = asyncio.new_event_loop()
wait = logger.flush_async(2000)
loop.call_soon(wait.send, None)
loop.run_until_complete(asyncio.sleep(0))
key = logger._native.observer_key()
assert len(_pools[key].observers) == 1
loop.close()
closed_loop = asyncio.run(logger.flush_async(0), debug=True)
assert isinstance(closed_loop, Err) and closed_loop.error.kind == "queue_full", closed_loop
assert key not in _pools or not _pools[key].observers
wait.close()
controls.release()
async def final_barrier():
    deadline = time.monotonic() + 5
    while True:
        result = await logger.flush_async(1000)
        if isinstance(result, Ok):
            return
        assert result.error.kind == "queue_full", result
        assert time.monotonic() < deadline
        await asyncio.sleep(0.001)
asyncio.run(final_barrier(), debug=True)
# Deterministic real completion-before-next-poll race. Freeze only this test
# loop's monotonic clock: native operations complete normally while every Python
# timer remains due one virtual millisecond later. No backend or Operation mock.
loop = asyncio.new_event_loop()
clock = [loop.time()]
loop.time = lambda: clock[0]
waits = []
start_count = controls.flush_calls()
for index in range(64):
    controls.hold()
    assert isinstance(logger.submit(LogEvent(level="info", target="async.embed", action="capacity.hold")), Ok)
    alias = logger if mode == "owned" else get_host_logger().value
    assert alias._native.observer_key() == key
    wait = alias.flush_async(60000)
    waits.append(wait)
    loop.call_soon(wait.send, None)
    loop.call_soon(loop.stop)
    loop.run_forever()
    pool = _pools[key]
    assert len(pool.observers) == index + 1, (index, len(pool.observers))
    observer = tuple(pool.observers.values())[-1]()
    operation = observer.operation
    controls.release()
    deadline = time.monotonic() + 5
    while operation.state() is None:
        assert time.monotonic() < deadline
        time.sleep(0.001)  # fixture outside the stopped event loop
    assert observer.timer is not None
    assert not observer.future.done()
overflow = loop.run_until_complete(logger.flush_async())
assert isinstance(overflow, Err) and overflow.error.code == "SC_OBSERVABILITY_BINDING_WAITERS_FULL", overflow
if mode != "owned":
    assert controls.flush_calls() - start_count == 64
clock[0] += 0.002
for _ in range(4):
    loop.run_until_complete(asyncio.sleep(0))
for wait in waits:
    try:
        wait.send(None)
    except StopIteration as completed:
        assert isinstance(completed.value, Ok), completed.value
    else:
        raise AssertionError("completed native operation did not resolve its waiter")
assert key not in _pools or not _pools[key].observers
loop.close()

if mode == "owned":
    assert isinstance(logger.shutdown(), Ok)
    assert isinstance(logger.wait_stopped(0), Ok)
