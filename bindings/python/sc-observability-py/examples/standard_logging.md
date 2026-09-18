# Standard logging and request context

Use `sc-observability` with GIL-enabled CPython 3.10–3.14 on a qualified
Linux, macOS, or Windows platform. Install the matching qualified wheel in your
application environment; distribution publication is handled separately in B.7.
The package and embedded Rust module must use the same binding schema version
(version 1). A standalone wheel creates its own logger; a Rust embedding host
registers the shared backend and Python obtains it with `get_host_logger()`.
Attachment cannot grant shutdown or level-change ownership.

Importing the package, creating a handler, and attaching to a host leave the
Python root logger unchanged. Install the adapter explicitly on the logger your
application owns:

```python
from sc_observability import Err
from sc_observability.logging import create_handler

configured = create_handler(backend, extra_fields=("job_id",))
if isinstance(configured, Err):
    report_to_application(configured.error)
else:
    handler = configured.value
    application_logger.addHandler(handler)
```

`backend` is an owned `Logger` or an `AttachedLogger`. Set the standard library
logger's level explicitly if necessary; its own filtering happens before the
handler. Levels below DEBUG map to trace, DEBUG to debug, INFO to info, WARNING
to warn, and ERROR or above to error. Names become event targets and the action
is `python.log`. Message formatting runs once; only selected extra fields and
exception/stack text are included. Core validation, redaction, and protected
source metadata still apply. Formatting errors become retained internal failures.

Submission is nonblocking. `emit`, `flush`, and `close` follow the standard
library protocol and return `None`; inspect `last_result()` or `health()` after
an operation to observe its Result. Health snapshots are immutable and include
all failure counters. Each rejected event increments its corresponding counter
once, saturating at `u64::MAX`. Filtered events are successful admissions.
Recursive handler invocation has a separate `reentrant` counter and retains
`SC_OBSERVABILITY_PY_HANDLER_REENTRANT`; it never logs that error recursively.
The latest completed operation replaces the previous status, except successful
outer emission preserves a recursion failure encountered during formatting.

`handler.flush()` waits at most 2 seconds and saves its Result without incrementing
an event-drop counter. `handler.close()` releases the borrowed reference and
never calls backend shutdown. Remove the handler from the named logger before
closing it. The owner remains responsible for explicit `Logger.shutdown()`;
attached loggers do not own that operation. This also holds during the standard
library's shutdown/atexit cleanup.

Request scopes return Results at every fallible boundary:

```python
from sc_observability import Err
from sc_observability.context import bind_context

bound = bind_context(request_id="request-7", correlation_id="shared-7")
if isinstance(bound, Err):
    report_to_application(bound.error)
else:
    entered = bound.value.enter()
    if isinstance(entered, Err):
        report_to_application(entered.error)
    else:
        try:
            application_logger.info("request accepted", extra={"job_id": 7})
        finally:
            closed = bound.value.close()
            if isinstance(closed, Err):
                report_to_application(closed.error)
```

Validate first, then enter: binding a scope alone changes no context and emits
no event. Close scopes in reverse entry order on the same thread and async task.
A closed scope cannot be reused; closing it again succeeds. Invalid transitions
return `SC_OBSERVABILITY_PY_CONTEXT_SCOPE_INVALID` and preserve existing context.
At most 64 scopes may be active in one context. There is deliberately no context
manager adapter: checking close's Result in `finally` preserves an application
exception even if cleanup fails.

Child async tasks inherit values, but do not own their parent's scope token.
Each child can enter its own scope; siblings retain separate values. Transfer
IDs explicitly to new threads and Rust work. Explicit non-None event fields
override scoped fields; absent fields inherit request, correlation, and trace
IDs. Trace IDs are diagnostic data and confer no authority.

Run `standard_logging.py` for the complete typed owned-mode lifecycle. The
`examples/rust-python-logging` executable registers one real Rust backend and
runs `b5_context.py`: Rust and Python emit the same validated correlation ID,
query both languages together, prove exception redaction, and verify that closing
the handler leaves the host alive. A final host-stop probe records a closed
failure in handler health. No duplicate Python writer is created.
