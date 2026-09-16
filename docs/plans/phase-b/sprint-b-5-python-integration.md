---
id: B.5
status: proposed
branch: feature/phase-b-5-python-integration
base: develop
---

# B.5 — First-class Python logging and mixed-language context

## Goal and dependencies

Make existing Python code idiomatic to integrate without rewriting every log
call, while sharing Rust-hosted observability in mixed applications.
`must_follow` B.4a: use its qualified packages and B.4’s tested owned/attached
runtime and stable errors.
B.6 `must_follow` this sprint for optional async waiting; B.7 owns publication.
These sprints share the Python runtime and conformance artifacts, so they are
not parallel_safe. Follow the phase parent-push merge-forward rule before every
child development/fix round; parent PR merges before child completion.

## Deliverables (authoritative)

1. Add `python/sc_observability/logging.py` with a standard-library
   `logging.Handler` adapter for either B.4 Logger or AttachedLogger. Map Python
   levels, logger name, message, exception/stack text and explicitly selected
   structured `extra` fields into the shared event DTO. Preserve backend
   redaction and protected source metadata. The handler defaults to nonblocking
   submission with contained failures; Logger.log remains nonblocking and returns its admission result. Direct emission returns Result; the standard-library adapter records ignored
   results in its inspectable health without raising them.
2. Add `python/sc_observability/context.py` with ContextVar-backed scoped
   request/correlation/trace context. Propagate context in Python async tasks;
   require explicit context transfer to new threads and Rust work. Validate
   supplied context before admission, copy owned context into the Rust event,
   and restore the previous context on normal return and exception. Never use
   a process-global mutable context or assume Rust thread-local spans cross the
   Python async boundary automatically.
3. Extend typed stubs, packaging and examples with standard logging setup,
   explicit owned-mode lifecycle and a mixed Rust/Python request walkthrough.
   Document installation as explicit opt-in: neither import nor host attachment
   modifies the Python root logger automatically. Add tests to the B.4 validator
   and record results in `docs/plans/phase-b/handoff-b-5.md`.

## Public signatures and behavior

```python
# Result, Failure, Admission, Completion and TraceContext reuse B.3/B.4.
class HandlerDropCause(str, Enum):
    VALIDATION = "validation"
    QUEUE_FULL = "queue_full"
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
    admission: Admission  # accepted or filtered, never collapsed
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
    # Construct through create_handler; no public fallible constructor.
    def emit(self, record: logging.LogRecord) -> None: ...
    def flush(self) -> None: ...
    def close(self) -> None: ...
    def health(self) -> Result[HandlerHealth]: ...
    def last_result(self) -> Result[HandlerOutcome]: ...

def create_handler(logger: Logger | AttachedLogger, *,
                   level: int = logging.NOTSET,
                   extra_fields: tuple[str, ...] = ()) -> Result[ObservabilityHandler]: ...

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
    # Opaque context-owned resource, constructed only by bind_context.
    def enter(self) -> Result[ContextOutcome]: ...
    def close(self) -> Result[ContextOutcome]: ...
    def last_result(self) -> Result[ContextOutcome]: ...

def bind_context(*, request_id: str | None = None,
                 correlation_id: str | None = None,
                 trace: TraceContext | None = None) -> Result[ContextScope]: ...
```

HandlerHealth always contains all cause keys, initially zero; counters saturate
at u64::MAX and snapshots are immutable copies. Backend Failure.kind maps to the
same-named cause; recursion uses reentrant. Additional owner-only level-change
errors do not arise on handler submission. Initial status is Ok(HandlerIdle).
A filtered event is HandlerEmitted(filtered), not a drop. Each failed event
increments one cause once; failed flush/close updates last_result without counting
an event drop. No retained LogRecord or unbounded exception history is kept.
Handler status follows operation completion order; reentrant failure is retained
unless the outer call independently produces a later failure. Snapshots do not
claim every historical result is retained.

Level mapping: below DEBUG -> trace, DEBUG..INFO-1 -> debug,
INFO..WARNING-1 -> info, WARNING..ERROR-1 -> warn, ERROR and above -> error.
Python logger name becomes validated target; action defaults to `python.log`.
Invalid target/field labels use the same frozen adapter validation policy;
formatting uses LogRecord.getMessage once inside the handler's recursion and
exception containment boundary. Exception text and selected extra fields pass
through shared redaction; do not dump arbitrary record.__dict__ into JSON.

Python logging requires emit/flush/close to return None. These protocol methods
are explicit adapters: they store their operation Result for last_result/health,
and never raise or manufacture success after a failure. Direct library APIs
return the Result to the caller. Record failures by stable cause in HandlerHealth, including recursive
logging and queue-full, without reporting them through the same handler.
Foreign formatter errors convert to the shared Failure union; the library does
not intentionally raise exceptions as control flow. Exception formatting uses
no local-variable capture. `flush` waits at most 2 seconds and records its stable
failure Result; callers wanting immediate inspection use Logger.flush explicitly. `close`
removes handler resources and never shuts down its borrowed logger. Logging's
own shutdown/atexit sequence must not introduce another core shutdown attempt.

bind_context validates first and returns an inactive scope; it does not change
the current context. enter activates it once and records a ContextVar token.
close restores the previous value only from the originating thread/task and in
LIFO order. Scope state is inactive, active, or closed, retained internally;
entering an active/closed scope or closing inactive/out-of-order/inherited-task
scope returns validation with code SC_OBSERVABILITY_PY_CONTEXT_SCOPE_INVALID
without changing context. Closing an already successfully closed scope is
idempotent Ok(ContextClosed). Initial last_result is Ok(ContextIdle); successful
entry records ContextEntered and successful close records ContextClosed. Check task/thread identity and stack top before reset;
foreign ContextVar failures become internal results and preserve saved state.
A newly created child task may inherit context values but never owns the parent's
scope token. No process-global stack is used.

ContextScope intentionally has no context-manager adapter in this release.
Use explicit enter and close, checking their Results; a caller uses try/finally
around application work to restore context even when application code raises.
close returns its failure without suppressing or replacing the application's
exception. This avoids hiding fallible cleanup behind Python's __exit__ protocol.
Examples check enter before executing scoped work and inspect close afterward.
The per-context active-scope stack is limited to 64; a 65th enter returns the
same validation code before mutating the stack. An inactive scope cannot close
an existing outer scope. No failed entry installs a cleanup marker.

Explicit event context overrides scoped context only for fields it supplies;
absent fields inherit the scope. Entering/exiting a context emits no extra event.
Trace identifiers supplied by a frontend are diagnostic input, not authority.
Rust host and Python use the same validated IDs to query a mixed request.

## Acceptance criteria (authoritative)

- AC1: Existing logging.getLogger calls through an explicitly installed handler
  produce structured records with correct level, target, exception and selected
  extra fields in both owned and host-attached modes. No global handler is added
  on import, and no duplicate writer is created.
- AC2: Concurrent async tasks retain separate correlation context; explicit
  thread/Rust transfer preserves IDs; exceptions restore the previous context.
  Mixed Rust/Python records for one request query together without context bleed.
- AC3: Recursive/panicking Python formatting, queue-full, invalid fields and
  host shutdown are reflected in handler health without recursive logging,
  hidden duplicate shutdown, or unbounded exit waits. Redaction fixtures pass. Handler creation/context validation return Err on
  invalid inputs; adapter status preserves each ignored failure variant.
- AC4: Typed examples run against built wheels and the embedded Rust module on
  the B.4a support matrix. Python documentation describes supported operations,
  failure behavior, lifecycle, installation and version compatibility directly.

## Required validation (authoritative)

```sh
bash scripts/ci/validate_python_bindings.sh
```

Extend that script with standard logging/handler shutdown tests, formatter
recursion, exception-redaction, ContextVar async isolation, explicit thread
transfer and Rust-host correlation tests. Required cases: empty/malformed context;
sibling-task isolation; child inheritance without token ownership; explicit
thread transfer; re-enter/reuse; close-before-enter, repeated close, out-of-order
close, wrong task/thread close; failed re-entry without closing the outer scope, stack-limit overflow,
and preservation of an application exception when cleanup fails. Handler cases
include every level boundary, missing selected extras, failing getMessage/exception
formatter, recursion, filtered events, full queue, stopped host, counter saturation,
flush timeout, repeat close and logging shutdown/atexit without duplicate core
shutdown. Verify exactly-once event drop accounting and immutable health snapshots. Run the tests on packaged Python code
and the embedding example; do not rely only on mocks of the backend.

## Paths to delete

None.

## Non-closure

No ContextScope context-manager protocol, automatic root-logger replacement,
Rust proc-macro equivalent, follow stream,
OTLP, cross-process logging service, Go or Node.js. Package publication is B.7.
