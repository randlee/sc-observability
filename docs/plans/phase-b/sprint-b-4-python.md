---
id: B.4
status: proposed
branch: feature/phase-b-4-python
base: develop
---

# B.4 — Python bindings through PyO3 and maturin

## Goal and dependencies

Deliver owned and Rust-host-attached Python logging over the public Rust API,
with typed Python values, deterministic lifecycle behavior and installable wheels.
`must_follow` B.3: reuse its accepted wire schema, stable error registry and
conformance fixtures, rather than creating a second schema authority. B.5
`must_follow` B.4 for idiomatic Python integration; B.7 owns publication.

The complete new DTO declarations, conversion boundaries, validation defaults
and error mapping are incorporated from [the binding contract](binding-contract.md).
They are part of this sprint's reviewable contract, not future design work.

## Deliverables (authoritative)

1. Create the isolated mixed Rust/Python project
   `bindings/python/sc-observability-py/` with `pyproject.toml`, locked Rust
   dependencies, PyO3 `cdylib` plus Rust `rlib` embedding target,
   `python/sc_observability/__init__.py`, stubs,
   `py.typed`, tests and examples. Proposed distribution: `sc-observability`;
   availability is a publication gate. Use the shared DTO crate via packaged
   dependencies and record its supported schema version.
2. Implement the API below using public `Logger`, query, health and config
   methods. Owned instances create independent nonglobal loggers. Attached
   instances share
   an application-provided backend and never create a second writer or own host
   shutdown. A synchronization layer separates Python lifetime from in-flight
   Rust operations. Owned instances retain LevelOwner and expose elevate/reset;
   attached instances expose only shared read-only level health. Add `examples/rust-python-logging/` to prove attachment.
   Convert to
   owned Rust inputs while attached, then detach around blocking backend calls, query,
   flush, shutdown and synchronization waits. Python callbacks are excluded.
3. Implement checked ergonomic Python-to-wire conversions and a stable
   discriminated Result/Failure data model carrying code/message/remediation. Python
   integers remain exact; convert through B.3's integer representation for
   fixtures and serialization. Use the same event/query/error rules, with
   explicit `LoggerConfig` creation mapping for service, root, level and built-in
   sinks; remaining core config uses documented defaults. No silent `str(error)`
   parsing, panic escape, process-global logging installation, or unrelated
   instance sharing.
4. Add `scripts/ci/validate_python_bindings.sh`, runtime/conformance/threaded
   tests, wheel build/install CI, and `docs/plans/phase-b/handoff-b-4.md`.
   Proposed initial support: GIL-enabled CPython 3.10–3.14, `abi3-py310`, macOS
   arm64/x86_64, Linux glibc x86_64/aarch64, Windows x86_64. Confirm compatible
   pinned PyO3/maturin versions and Rust MSRV at entry; if that matrix cannot
   pass, revise this proposed scope before implementation instead of silently
   dropping platforms. Use manylinux_2_28 for Linux and deployment targets
   recorded in wheel metadata. Free-threaded Python, PyPy, musl and Windows
   arm64 are explicitly deferred.

## Public signatures and lifecycle

```python
@dataclass(frozen=True)
class Ok(Generic[T]):
    value: T
    kind: Literal["ok"] = field(default="ok", init=False)

@dataclass(frozen=True)
class Err:
    error: Failure
    kind: Literal["error"] = field(default="error", init=False)

Result = Union[Ok[T], Err]

# Failure is a generated union of frozen dataclasses with the B.3 kind tags.
# No variant inherits Exception. Clients narrow by kind or pattern matching.
def create_logger(config: LoggerConfig) -> Result[Logger]: ...
def get_host_logger() -> Result[AttachedLogger]: ...

class Logger:
    def log(self, event: LogEvent) -> Result[Admission]: ...
    def query(self, query: LogQuery) -> Result[LogSnapshot]: ...
    def health(self) -> Result[LogHealth]: ...
    def flush(self, timeout_ms: int = 2000) -> Result[Completion]: ...
    def shutdown(self, timeout_ms: int = 2000) -> Result[LogHealth]: ...
    def wait_stopped(self, timeout_ms: int = 2000) -> Result[LogHealth]: ...
    def elevate_level(self, level: LevelFilter,
                      source: LevelChangeSource = "application") -> Result[LevelChange]: ...
    def reset_level(self, source: LevelChangeSource = "application") -> Result[LevelChange]: ...
```

Factory construction replaces a public fallible __init__. DTO decoding and
validation also return Result. Python wrappers convert foreign exceptions and
PyO3 extraction failures into tagged data before returning to user code; no
internal raise/catch implementation for the library's own expected errors and
no public unwrap-or-raise helper. Rust helpers use Result; a native panic cannot
unwind through the FFI boundary and must produce a stable internal failure where
containment is possible. Do not change unrelated published core APIs in this
binding sprint; report any uncovered source contract incompatibility explicitly.

Rust embedding surface (uses public DTO/error types from the shared contract):

```rust
pub trait HostLoggingBackend: Send + Sync {
    fn try_log(&self, event: LogEventDto) -> Result<AdmissionDto, Failure>;
    fn query(&self, query: LogQueryDto) -> Result<LogSnapshotDto, Failure>;
    fn health(&self) -> Result<LogHealthDto, Failure>;
    fn flush(&self, timeout: std::time::Duration) -> Result<(), Failure>;
}
pub fn install_host_logger(
    module: &pyo3::Bound<'_, pyo3::types::PyModule>,
    backend: std::sync::Arc<dyn HostLoggingBackend>,
) -> Result<(), Failure>;
```

Host installation is immutable and once per PyO3 module instance. The first
install stores a backend Arc in module state; any repeat, even the same Arc,
returns unavailable with SC_OBSERVABILITY_BINDING_HOST_ALREADY_INSTALLED and
leaves the first backend unchanged. No replacement/reset API ships. Concurrent
installs use one atomic winner; losers return that same error. get_host_logger
before installation returns unavailable with
SC_OBSERVABILITY_BINDING_HOST_NOT_INSTALLED. Each attached handle retains the
backend Arc so removing the module does not create a dangling reference; backend
implementations must keep only nonowning control of the host logger and never
retain its lifecycle owner. Existing handles observe closed after host shutdown,
even if module/handle/backend references remain. Interpreter teardown releases
module-owned state without calling Python or shutting down the host.

The module exposes the result-returning host factory above. AttachedLogger exposes log/query/health/flush
with the same Result signatures as Logger, without shutdown or wait_stopped. Host shutdown or Python handle drop never
transfers host lifecycle ownership. Backend methods must release locks before
blocking on writer operations and return stable closed outcomes after host stop.
The Rust embedding example implements this trait using the accepted public core
or bridge control surface. It proves Rust and Python records reach the same
writer and observe the same redaction, correlation, health and stop boundary.

The Rust host compiles the binding `rlib` and registers its PyO3 module in its
embedded interpreter. Do not also load an independently built wheel extension
and exchange Rust trait objects across dynamic-library boundaries. No raw pointer,
Python integer handle, or C ABI is exposed; arbitrary external-process attachment
is future scope. This build topology and supported embedding initialization order
must be documented and tested. Extension-wheel and embedded-executable linking
configurations are distinct: pin the PyO3 build settings for each and verify both
without extension-only flags leaking into the host executable. The Rust embedding
crate is a B.7 publish artifact so hosts can consume this API from crates.io.

Python `LoggerConfig` requires `service` and `log_root`; optional level defaults
to info, file sink to enabled and console sink to disabled, matching the checked
core defaults. No user-provided Rust ownership pointers or callback objects are
accepted. `log(event)` performs nonblocking admission through public Rust try_log
and returns Result[Admission]. It never waits for queue capacity or I/O. Queue-full,
invalid fields, conversion/formatting errors and unavailable/closed host are tagged
errors, not exceptions. The caller can ignore the result for fire-and-forget usage
or handle it at a higher level. Admission is the generated union of accepted and filtered values: accepted
means queue admission, filtered means valid but excluded by threshold. Neither
is proof of persistence. The core implementation uses the additive
try_log_with_outcome API, not a guessed outcome from legacy Result<()>.
LevelFilter and LevelChangeSource are Literal unions matching B.3; LevelState,
LevelChange and ChangeDiagnostic are generated frozen dataclasses matching its
field names and discriminator tags, with Python exact ints for revisions and
checked decimal-string encoding on wire. Owned operations serialize with
shutdown over the same owner lock, never holding it for sink I/O. Invalid
level/source arguments return validation; all runtime mutation failures preserve
B.3's tagged error mapping and every diagnostic. A successful change whose
logging failed remains Ok(changed/not_accepted). AttachedLogger and
HostLoggingBackend expose no elevation/reset methods. Already accepted events that fail later
are reported through the discriminated health snapshot, not retroactive failure
on the producer call.

Maintain bounded in-memory counters and a last diagnostic if possible. Failure in
that bookkeeping must preserve the original result without raising or recursively
logging. Validate timeouts/query inputs into validation variants (including
bool-as-int, negative, non-finite and overflowing values). Setup and lifecycle
failures also use Result, so there is no exception-based alternate error API.

Owned-mode lifecycle is `running -> stopping -> stopped` or terminal `failed`. Timeout is
a wait result, never a claim that a worker stopped. Shutdown closes admission
once, assigns exactly one owner to final core shutdown, and retains completion
and final health independently of Python reference counts. Already admitted
operations may finish; later log/query/flush calls return Err with a closed
variant. A caller ignoring that result remains unaffected. `health` and `wait_stopped` remain usable. Repeated shutdown waits on the
same completion; it never starts a second core shutdown. A worker failure is
observable via the saved stable error. Read-only handles cannot prevent final
ownership transfer indefinitely. One in-flight flush slot is retained until completion; concurrent requests
return queue_full/SC_OBSERVABILITY_LOG_FLUSH_IN_PROGRESS without coalescing distinct barriers.

GC finalization schedules bounded best-effort cleanup once without an unbounded
interpreter-thread wait and records the result if possible. It never raises or
calls Python during interpreter teardown. Explicit shutdown/wait_stopped return
the final Result and are the supported observable cleanup path. Context-manager
protocol conveniences are deferred until they can expose cleanup results without
hiding them behind __exit__'s boolean protocol or masking application exceptions.

## Acceptance criteria (authoritative)

- AC1: Clean wheel installations run every supported operation and the typed
  example on the approved Python/platform matrix with no source-tree imports.
  File logs, Result/Failure tags and bounded snapshots match the shared
  Rust/TypeScript fixtures. No exception class is exported as an error contract.
- AC2: A full queue or held sink does not stop an independent Python thread;
  concurrent log/query/flush/shutdown yields documented results, never PyO3
  borrow-check exceptions as lifecycle policy. Two owned instances remain isolated; an attached instance instead shares the
  host log/health/correlation and cannot shut it down or silently open a second
  writer. Host shutdown while Python is active produces stable closed outcomes.
  Missing host, duplicate install and concurrent install follow the once-per-module
  contract; module teardown and surviving handles never shut down the host.
- AC3: Timeout then late completion/failure is observed through wait_stopped;
  shutdown runs once; post-stop health persists; explicit and GC cleanup paths
  pass subprocess tests without process hangs or implicit global logger install.
- AC4: Default log calls with formatting/conversion failure, queue-full, failed
  sink or stopped host never propagate a logging exception or wait for sink I/O.
  Failures return the appropriate union variant, including factory/query/health/
  flush/shutdown errors. Failed diagnostic accounting preserves the original
  result; recursion is rejected and counted once. Typed examples demonstrate
  both exhaustive handling and intentional omission of the returned result.
- AC5: Owned baseline/elevate/reduce/reset/repeat/Off and late-owner lifecycle
  cases agree with Rust and B.3 fixtures, including invalid input, unsupported
  levels where applicable, queue-full diagnostic failure and revision overflow.
  Attached health follows host changes with exact revisions but exposes no owner
  mutation methods. Concurrent mutation/shutdown stays typed and isolated between
  independent owned instances. Admission accepted/filtered and diagnostic message and remediation
  steps survive Rust/Python/wire conversion.
- AC6: Wheel and source distributions contain stubs, py.typed and all required
  Rust sources or resolvable registry dependencies. Installation/type checking
  succeeds outside the monorepo; imported core API/dependency gates still pass.

## Required validation (authoritative)

```sh
bash scripts/ci/validate_python_bindings.sh
bash scripts/ci/validate_dependency_bans.sh
bash scripts/ci/validate_docs_consistency.sh
```

The new script builds wheels with locked maturin/PyO3, installs them into clean
venvs, runs pytest and stub/type checks there, builds/runs the Rust embedding example,
executes deterministic threaded
and shutdown subprocess tests, validates shared fixtures, and rebuilds a wheel
from the produced sdist outside the checkout. CI supplies each supported target;
record exact wheel tags, architectures, Python versions, hashes and results.
Development installs alone do not satisfy validation. Add static checks against
authored raise/panic/unwrap-based operational control flow, fault injection for
every public Result path (including factories and health), and Python type-check
fixtures that exhaustively narrow Result/Failure variants. Foreign PyO3/formatter
errors must be converted at the boundary without escaping. Explicit host-lifetime
fixtures cover get_host_logger before installation; second installation with
both the same and a different backend; simultaneous installs with exactly one
winner and one HOST_ALREADY_INSTALLED result; module collection with live attached
handles; backend retention until the last module/handle reference is gone; host
shutdown while handles survive; and interpreter teardown without calling Python
or shutting down the application-owned logger. No race test may accept two
successful installations or conceal a replacement by comparing only pointers. Include real owned
mutation and attached-host health tests, threaded owner/shutdown races,
accepted/filtered fixtures, full remediation round-trips and every level union
variant; validate generated stubs and runtime values together.

## Paths to delete

None.

## Non-closure

No PyPI publication (B.7), standard-library Handler/context integration (B.5), context-manager lifecycle
conveniences, implicit Rust
facade installation, Python callback sinks/redactors, follow stream, async receipt/wait API (B.6),
OTLP, Go, or whole-workspace public API parity.

## Technical references

[PyO3 detach guidance](https://pyo3.rs/main/parallelism) applies to every blocking
Rust call. [PyO3 thread safety](https://pyo3.rs/v0.29.0/class/thread-safety.html)
requires explicit concurrency design beyond releasing the GIL.
[PyO3 build and embedding guidance](https://pyo3.rs/main/building-and-distribution)
requires distinct linking configuration for embedding and extension builds.
[Maturin mixed-project guidance](https://www.maturin.rs/tutorial.html?highlight=stable)
informs the extension/package layout and wheel validation.
