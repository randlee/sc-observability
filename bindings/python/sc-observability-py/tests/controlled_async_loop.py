"""Test-owned observer clock; native backend clocks are unchanged."""
from __future__ import annotations

import asyncio
import time


class CompletionLoop(asyncio.SelectorEventLoop):
    """Run observation callbacks without making wall time a success criterion.

    Delayed callbacks become eligible on the next loop turn. Tests explicitly
    advance ``now`` to exercise observer expiry. Production polling and native
    operation state still decide completion; this loop never supplies a Result.
    """

    def __init__(self, *, hard_bound_s: float) -> None:
        self.now = 0.0
        self._hard_bound_s = hard_bound_s
        self._hard_deadline = time.monotonic() + hard_bound_s
        super().__init__()
        self.set_debug(True)

    def time(self) -> float:
        return self.now

    def call_later(self, delay, callback, *args, context=None):
        if time.monotonic() > self._hard_deadline:
            raise AssertionError(
                f"observed operation did not complete within {self._hard_bound_s}s "
                f"of real time; observer clock frozen at {self.now}"
            )
        return self.call_at(self.now, callback, *args, context=context)

    def __enter__(self):
        return self

    def __exit__(self, *exc):
        self.run_until_complete(self.shutdown_asyncgens())
        self.close()
