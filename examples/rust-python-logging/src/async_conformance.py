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
if mode == "owned":
    assert isinstance(logger.shutdown(), Ok)
    assert isinstance(logger.wait_stopped(0), Ok)
