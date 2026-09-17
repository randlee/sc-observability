from dataclasses import dataclass, field
from typing import Literal, NoReturn, TypeAlias
from . import Result, generated

@dataclass(frozen=True)
class Resolved:
    admission: generated.Admission
    kind: Literal["resolved"] = field(default="resolved", init=False)

ReceiptState: TypeAlias = Resolved

class LogReceipt:
    def __init__(self, _private: NoReturn) -> None: ...
    def state(self) -> ReceiptState: ...
    async def wait(self, timeout_ms: int = 2000) -> Result[generated.Admission]: ...

from . import AttachedLogger, Logger, LogEvent

def _submit(logger: Logger | AttachedLogger, event: LogEvent) -> Result[LogReceipt]: ...
async def _flush_async(native: object, timeout_ms: int = 2000) -> Result[generated.Completion]: ...
