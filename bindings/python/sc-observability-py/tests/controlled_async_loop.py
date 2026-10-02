"""Test-owned observer clock; native backend clocks are unchanged."""
from __future__ import annotations

import asyncio


class CompletionLoop(asyncio.SelectorEventLoop):
    """Run observation callbacks without making wall time a success criterion.

    Delayed callbacks become eligible on the next loop turn. Tests explicitly
    advance ``now`` to exercise observer expiry. Production polling and native
    operation state still decide completion; this loop never supplies a Result.
    """

    def __init__(self) -> None:
        self.now = 0.0
        super().__init__()
        self.set_debug(True)

    def time(self) -> float:
        return self.now

    def call_later(self, delay, callback, *args, context=None):
        return self.call_at(self.now, callback, *args, context=context)

    def __enter__(self):
        return self

    def __exit__(self, *exc):
        self.run_until_complete(self.shutdown_asyncgens())
        self.close()
