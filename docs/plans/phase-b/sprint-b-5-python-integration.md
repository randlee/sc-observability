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
`must_follow` B.4: use its tested owned/attached runtime and stable errors.
B.6 `must_follow` this sprint for optional async waiting; B.7 owns publication.

## Deliverables (authoritative)

1. Add `python/sc_observability/logging.py` with a standard-library
   `logging.Handler` adapter for either B.4 Logger or AttachedLogger. Map Python
   levels, logger name, message, exception/stack text and explicitly selected
   structured `extra` fields into the shared event DTO. Preserve backend
   redaction and protected source metadata. The handler defaults to nonblocking
   submission with contained failures; Logger.log also defaults to fail-open
   fire-and-forget. Direct emission returns Result; the standard-library adapter records ignored
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
class ObservabilityHandler(logging.Handler):
    # Construct through create_handler; no public fallible constructor.
    def emit(self, record: logging.LogRecord) -> None: ...
    def flush(self) -> None: ...
    def close(self) -> None: ...
    def health(self) -> Result[HandlerHealth]: ...
    def last_result(self) -> Result[HandlerOutcome]: ...

@dataclass(frozen=True)
class HandlerHealth:
    dropped_by_cause: Mapping[str, int]
    last_result: Result[HandlerOutcome]

def create_handler(logger: Logger | AttachedLogger, *,
                   level: int = logging.NOTSET,
                   extra_fields: tuple[str, ...] = ()) -> Result[ObservabilityHandler]: ...

def bind_context(*, request_id: str | None = None,
                 correlation_id: str | None = None,
                 trace: TraceContext | None = None) -> Result[ContextScope]: ...
```

HandlerOutcome is a generated tagged union with idle/emitted/flushed/closed
variants; initial status is Ok(idle), never a fabricated completed operation.
An emission result maps the underlying admission result without discarding its
Failure variant. The fixed standard-library protocol chooses not to return that
value; direct logger calls return it normally.

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

bind_context validates first and returns Result[ContextScope]. An Ok scope
implements the nonfallible context-manager entry/exit protocol, restoring the
previous context and preserving any exception raised by application code.
Malformed context returns Err before a scope is entered.

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
  the B.4 support matrix. Python documentation describes supported operations,
  failure behavior, lifecycle, installation and version compatibility directly.

## Required validation (authoritative)

```sh
bash scripts/ci/validate_python_bindings.sh
```

Extend that script with standard logging/handler shutdown tests, formatter
recursion, exception-redaction, ContextVar async isolation, explicit thread
transfer and Rust-host correlation tests. Run the tests on packaged Python code
and the embedding example; do not rely only on mocks of the backend.

## Paths to delete

None.

## Non-closure

No automatic root-logger replacement, Rust proc-macro equivalent, follow stream,
OTLP, cross-process logging service, Go or Node.js. Package publication is B.7.
