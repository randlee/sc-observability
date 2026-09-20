from typing import NoReturn

def assert_never(value: NoReturn) -> NoReturn:
    raise AssertionError(value)
from sc_observability import AttachedLogger, Logger, LogEvent
from sc_observability.async_logging import LogReceipt

async def inspect(logger: Logger | AttachedLogger) -> str:
    submitted = logger.submit(LogEvent(level="info", target="typing.async", action="submit"))
    if submitted.kind == "error":
        return submitted.error.code
    receipt: LogReceipt = submitted.value
    state = receipt.state()
    if state.kind == "resolved":
        assert state.admission.kind in ("accepted", "filtered")
    else:
        assert_never(state)
    admitted = await receipt.wait(0)
    if admitted.kind == "error":
        return admitted.error.code
    if admitted.value.kind == "accepted":
        pass
    elif admitted.value.kind == "filtered":
        pass
    else:
        assert_never(admitted.value)
    flushed = await logger.flush_async()
    if flushed.kind == "error":
        return flushed.error.code
    return flushed.value.kind
