"""Explicit standard logging with observable scope and owner cleanup."""
from __future__ import annotations

import logging
from pathlib import Path
import tempfile

from sc_observability import Err, LoggerConfig, LogQuery, create_logger
from sc_observability.context import bind_context
from sc_observability.logging import create_handler


def main(root: Path) -> bool:
    created = create_logger(LoggerConfig(service="python-example", log_root=str(root)))
    if isinstance(created, Err):
        print(created.error.code)
        return False
    owner = created.value
    handler = None
    named = logging.getLogger("example.request")
    previous_level = named.level
    try:
        configured = create_handler(owner, extra_fields=("job_id",))
        if isinstance(configured, Err):
            print(configured.error.code)
            return False
        handler = configured.value
        named.addHandler(handler)  # explicit opt-in; root logger is untouched
        named.setLevel(logging.INFO)
        bound = bind_context(request_id="request-7", correlation_id="shared-7")
        if isinstance(bound, Err):
            print(bound.error.code)
            return False
        entered = bound.value.enter()
        if isinstance(entered, Err):
            print(entered.error.code)
            return False
        try:
            named.info("request accepted", extra={"job_id": 7})
        finally:
            closed = bound.value.close()  # preserves any application exception
            if isinstance(closed, Err):
                print(closed.error.code)
        if isinstance(closed, Err):
            return False
        result = handler.last_result()
        if isinstance(result, Err):
            print(result.error.code)
            return False
        flushed = owner.flush()
        if isinstance(flushed, Err):
            print(flushed.error.code)
            return False
        rows = owner.query(LogQuery(correlation_id="shared-7"))
        return not isinstance(rows, Err) and bool(rows.value.events)
    finally:
        if handler is not None:
            named.removeHandler(handler)
            handler.close()  # borrowed logger stays live until explicit owner shutdown
        named.setLevel(previous_level)
        stopped = owner.shutdown()
        if isinstance(stopped, Err):
            print(stopped.error.code)


if __name__ == "__main__":
    with tempfile.TemporaryDirectory() as directory:
        print("B5_STANDARD_LOGGING_OK" if main(Path(directory)) else "B5_STANDARD_LOGGING_FAILED")
