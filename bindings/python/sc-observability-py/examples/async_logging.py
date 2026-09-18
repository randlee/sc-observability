"""Explicit owned lifecycle and optional receipt/flush observation.

Admission means accepted into the writer queue or filtered before enqueue.
Flush completion follows the configured sink contract, not fsync durability.
A timed-out/cancelled flush keeps running; its old Result has no public accessor.
Inspect health for later failures. Only a later explicit flush starts a new
barrier after the prior native slot is free; no automatic retry occurs.
"""
from __future__ import annotations

import asyncio
import tempfile
from sc_observability import Err, LogEvent, LoggerConfig, create_logger

async def main() -> None:
    with tempfile.TemporaryDirectory() as root:
        created = create_logger(LoggerConfig(service="async-example", log_root=root))
        if isinstance(created, Err):
            print(created.error.code)
            return
        logger = created.value
        logger.log(LogEvent(level="info", target="async.example", action="ignored.log"))
        logger.submit(LogEvent(level="info", target="async.example", action="ignored.receipt"))
        submitted = logger.submit(LogEvent(level="info", target="async.example", action="inspect"))
        if isinstance(submitted, Err):
            print(submitted.error.code)
        else:
            first = await submitted.value.wait(0)
            second = await submitted.value.wait()
            assert first == second
            print(submitted.value.state().admission.kind)
        flushed = await logger.flush_async()
        if isinstance(flushed, Err):
            print(flushed.error.code)
        # shutdown is the explicit synchronous lifecycle API. Offload it so
        # the application event-loop thread never waits for native shutdown.
        stopped = await asyncio.to_thread(logger.shutdown)
        if isinstance(stopped, Err):
            print(stopped.error.code)
            late = await asyncio.to_thread(logger.wait_stopped)
            if isinstance(late, Err):
                print(late.error.code)

if __name__ == "__main__":
    asyncio.run(main(), debug=True)
