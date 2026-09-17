from dataclasses import dataclass, field
from typing import Literal, Never, Union
from . import Result, TraceContext

@dataclass(frozen=True)
class ContextIdle:
    kind: Literal["idle"] = field(default="idle", init=False)
@dataclass(frozen=True)
class ContextEntered:
    kind: Literal["entered"] = field(default="entered", init=False)
@dataclass(frozen=True)
class ContextClosed:
    kind: Literal["closed"] = field(default="closed", init=False)
ContextOutcome = Union[ContextIdle, ContextEntered, ContextClosed]
class ContextScope:
    def __init__(self, _private: Never) -> None: ...
    def enter(self) -> Result[ContextOutcome]: ...
    def close(self) -> Result[ContextOutcome]: ...
    def last_result(self) -> Result[ContextOutcome]: ...
def bind_context(*, request_id: str | None = ..., correlation_id: str | None = ..., trace: TraceContext | None = ...) -> Result[ContextScope]: ...

from . import LogEvent
def _inherit_event(event: LogEvent) -> Result[LogEvent]: ...
def _validate_event(event: LogEvent) -> Result[None]: ...
