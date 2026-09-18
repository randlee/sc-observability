# Optional asyncio observation

`Logger.submit(event)` and `AttachedLogger.submit(event)` are ordinary synchronous
methods. They copy the event and current scoped context, perform one nonblocking
admission, and return `Result[LogReceipt]`. An admission failure is an `Err`;
a successful receipt is already `Resolved`, with the exact `accepted` or
`filtered` admission value. Calling `log` or `submit` needs no event loop, and
ignoring either Result creates no warning or unhandled Future exception.

A receipt holds only its admission result, not the event or logger. `state()`
remains usable after shutdown or loop closure. `await receipt.wait(timeout_ms)`
validates integer milliseconds in 0..60000, then returns immediately without a
suspension point, timer or waiter allocation. Zero is valid; repeated waits
return the saved admission. Cancellation before an asyncio task starts belongs
to asyncio. Once receipt waiting executes, it has no cancellation suspension.
Keeping many receipts is caller-owned memory; the logger keeps no receipt list.

`await logger.flush_async(timeout_ms=2000)` starts one native flush and observes
its saved completion on the current loop. Pending observation uses one timer
scheduled every millisecond and a monotonic deadline. The event loop never
waits for writer I/O, queue capacity, a native mutex or a native thread join.
There is no Python callback from native threads and no per-call native worker.

A shared Python backend admits at most 64 flush observers. Separate attached
wrappers share that cap. A completed native operation still occupies its Python
observer until the loop observes it. Capacity is reserved before native start;
exhaustion returns `queue_full/SC_OBSERVABILITY_BINDING_WAITERS_FULL`. Rejection,
completion, timeout and cancellation release the observer and cancel its timer.
Closed-loop reservations are reclaimed before another reservation; module
teardown drops observation without invoking or shutting down an attached host.

Only one native flush is in flight per backend. Overlap returns
`SC_OBSERVABILITY_BINDING_FLUSH_IN_PROGRESS`; the native bridge can separately
report `SC_OBSERVABILITY_LOG_FLUSH_IN_PROGRESS`. Timeout and cancellation end
observation, never the underlying flush. Core keeps its slot until its flush
returns. A bridge-native timeout releases the adapter slot while the bridge may
still hold its own slot. No timeout or cancellation triggers a retry.

An executing flush wait converts cancellation to a tagged cancelled Result.
Cancellation before task start remains asyncio's own cancellation. A flush that
outlives its wait has no public prior-result accessor. Inspect health for later
failures; a later explicit flush is a new barrier after the old slot is free.
Shutdown is different: `wait_stopped` retains its eventual lifecycle Result.
The example offloads synchronous shutdown to keep its event loop responsive.

Admission records queue acceptance, not successful writing. Later writer faults
appear in health and do not retroactively alter a saved receipt. Flush completion
uses the core sink-flush contract; it does not promise fsync durability. The
example in `async_logging.py` shows ignored results, repeated receipt inspection,
async flush and explicit owned lifecycle without implicit root-logger setup.
