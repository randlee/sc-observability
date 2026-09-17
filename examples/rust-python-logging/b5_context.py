"""One shared Rust/Python writer with explicitly transferred request identifiers."""
import logging
from sc_observability import Err, Ok, LogEvent, LogQuery, get_host_logger
from sc_observability.context import bind_context
from sc_observability.logging import create_handler

attached_result = get_host_logger()
assert isinstance(attached_result, Ok)
attached = attached_result.value
configured = create_handler(attached, extra_fields=("job_id",))
assert isinstance(configured, Ok)
handler = configured.value
named = logging.getLogger("python.mixed")
prior_level, prior_propagate = named.level, named.propagate
named.setLevel(logging.INFO)
named.propagate = False
named.addHandler(handler)
bound = bind_context(request_id="b5-request", correlation_id="b5-mixed-request")
assert isinstance(bound, Ok)
entered = bound.value.enter()
assert isinstance(entered, Ok)
try:
    named.info("Python handles the shared request", extra={"job_id": 7})
    try:
        raise ValueError("Bearer embedded-exception-secret")
    except ValueError:
        named.exception("contained request failure")
    assert isinstance(handler.last_result(), Ok)
finally:
    closed = bound.value.close()
    named.removeHandler(handler)
    named.setLevel(prior_level)
    named.propagate = prior_propagate
    handler.close()
    assert isinstance(closed, Ok)
assert isinstance(attached.flush(), Ok)
rows = attached.query(LogQuery(correlation_id="b5-mixed-request"))
assert isinstance(rows, Ok)
events = rows.value.events
assert any(row.target == "rust.mixed" for row in events)
assert sum(row.target == "python.mixed" for row in events) == 2
assert {row.fields["sc_observability.binding.language"].value for row in events} == {"rust", "python"}
for row in events:
    assert row.request_id == "b5-request"
    if "exception" in row.fields:
        assert "embedded-exception-secret" not in row.fields["exception"].value
assert isinstance(attached.log(LogEvent(level="info", target="python.after.handler", action="still.live")), Ok)
assert isinstance(attached.health(), Ok)
print("B5_MIXED_CONTEXT_OK", flush=True)
