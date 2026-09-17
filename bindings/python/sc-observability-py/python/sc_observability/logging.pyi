from dataclasses import dataclass, field
from enum import Enum
import logging
from typing import Literal, Mapping, NoReturn, Union
from . import AttachedLogger, Logger, Result, generated

class HandlerDropCause(str, Enum):
    VALIDATION = "validation"
    QUEUE_FULL = "queue_full"
    BELOW_BASELINE = "below_baseline"
    UNSUPPORTED_LEVEL = "unsupported_level"
    PERMISSION_DENIED = "permission_denied"
    CLOSED = "closed"
    UNAVAILABLE = "unavailable"
    IO = "io"
    TIMEOUT = "timeout"
    CANCELLED = "cancelled"
    UNSUPPORTED_VERSION = "unsupported_version"
    INTERNAL = "internal"
    UNKNOWN_REMOTE = "unknown_remote"
    REENTRANT = "reentrant"
@dataclass(frozen=True)
class HandlerIdle:
    kind: Literal["idle"] = field(default="idle", init=False)
@dataclass(frozen=True)
class HandlerEmitted:
    admission: generated.Admission
    kind: Literal["emitted"] = field(default="emitted", init=False)
@dataclass(frozen=True)
class HandlerFlushed:
    kind: Literal["flushed"] = field(default="flushed", init=False)
@dataclass(frozen=True)
class HandlerClosed:
    kind: Literal["closed"] = field(default="closed", init=False)
HandlerOutcome = Union[HandlerIdle, HandlerEmitted, HandlerFlushed, HandlerClosed]
@dataclass(frozen=True)
class HandlerHealth:
    dropped_by_cause: Mapping[HandlerDropCause, int]
    last_result: Result[HandlerOutcome]
class ObservabilityHandler(logging.Handler):
    def __init__(self, _private: NoReturn) -> None: ...
    def emit(self, record: logging.LogRecord) -> None: ...
    def flush(self) -> None: ...
    def close(self) -> None: ...
    def health(self) -> Result[HandlerHealth]: ...
    def last_result(self) -> Result[HandlerOutcome]: ...
def create_handler(logger: Logger | AttachedLogger, *, level: int = ..., extra_fields: tuple[str, ...] = ...) -> Result[ObservabilityHandler]: ...
