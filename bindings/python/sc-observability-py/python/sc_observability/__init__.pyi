from dataclasses import dataclass, field
from typing import Generic, Literal, Mapping, TypeAlias, TypeVar, NoReturn
from . import generated as generated
from .async_logging import LogReceipt as LogReceipt, ReceiptState as ReceiptState, Resolved as Resolved
T = TypeVar('T')
Level: TypeAlias = Literal['trace', 'debug', 'info', 'warn', 'error']
LevelFilter: TypeAlias = Literal['off', 'error', 'warn', 'info', 'debug', 'trace']
LevelChangeSource: TypeAlias = Literal['application', 'user_request', 'diagnostic_session']

@dataclass(frozen=True)
class Ok(Generic[T]):
    value: T
    kind: Literal['ok'] = field(default='ok', init=False)

@dataclass(frozen=True)
class Err:
    error: generated.Failure
    kind: Literal['error'] = field(default='error', init=False)
Result: TypeAlias = Ok[T] | Err

@dataclass(frozen=True)
class LoggerConfig:
    service: str
    log_root: str
    level: LevelFilter = 'info'
    enable_file_sink: bool = True
    enable_console_sink: bool = False

@dataclass(frozen=True)
class TraceContext:
    trace_id: str
    span_id: str
    parent_span_id: str | None = None

@dataclass(frozen=True)
class FieldMatch:
    field: str
    value: object

@dataclass(frozen=True)
class LogEvent:
    level: Level
    target: str
    action: str
    message: str | None = None
    trace: TraceContext | None = None
    request_id: str | None = None
    correlation_id: str | None = None
    outcome: str | None = None
    fields: Mapping[str, object] = field(default_factory=dict)

@dataclass(frozen=True)
class LogQuery:
    service: str | None = None
    levels: tuple[Level, ...] = ()
    target: str | None = None
    action: str | None = None
    request_id: str | None = None
    correlation_id: str | None = None
    since: str | None = None
    until: str | None = None
    field_matches: tuple[FieldMatch, ...] = ()
    limit: int = 100
    order: Literal['oldest_first', 'newest_first'] = 'oldest_first'

def _at() -> str:
    ...

def _failure(field_name: str, message: str) -> generated.Failure:
    ...

def _internal(message: str) -> generated.Failure:
    ...

def _event(event: object) -> Result[dict[str, object]]:
    ...

def _decode_control(payload: object) -> Result[None]:
    ...

class Logger:

    def __init__(self, _private: NoReturn) -> None:
        ...

    def log(self, event: LogEvent) -> Result[generated.Admission]:
        ...

    def submit(self, event: LogEvent) -> Result[LogReceipt]: ...
    async def flush_async(self, timeout_ms: int = 2000) -> Result[generated.Completion]: ...

    def query(self, query: LogQuery) -> Result[generated.LogSnapshot]:
        ...

    def health(self) -> Result[generated.LogHealth]:
        ...

    def flush(self, timeout_ms: int=2000) -> Result[generated.Completion]:
        ...

    def shutdown(self, timeout_ms: int=2000) -> Result[generated.LogHealth]:
        ...

    def wait_stopped(self, timeout_ms: int=2000) -> Result[generated.LogHealth]:
        ...

    def elevate_level(self, level: LevelFilter, source: LevelChangeSource='application') -> Result[generated.LevelChange]:
        ...

    def reset_level(self, source: LevelChangeSource='application') -> Result[generated.LevelChange]:
        ...

class AttachedLogger:

    def __init__(self, _private: NoReturn) -> None:
        ...

    def log(self, event: LogEvent) -> Result[generated.Admission]:
        ...

    def submit(self, event: LogEvent) -> Result[LogReceipt]: ...
    async def flush_async(self, timeout_ms: int = 2000) -> Result[generated.Completion]: ...

    def query(self, query: LogQuery) -> Result[generated.LogSnapshot]:
        ...

    def health(self) -> Result[generated.LogHealth]:
        ...

    def flush(self, timeout_ms: int=2000) -> Result[generated.Completion]:
        ...

def create_logger(config: LoggerConfig) -> Result[Logger]:
    ...

def get_host_logger() -> Result[AttachedLogger]:
    ...

def _timeout(timeout_ms: object) -> Result[str]: ...
def _decode(name: str, payload: object) -> Result[object]: ...
def _typed(value: Result[object]) -> Result[T]: ...
