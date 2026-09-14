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
`must_follow` B.4 for idiomatic Python integration; B.6 owns publication.

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
   Rust operations; add `examples/rust-python-logging/` to prove attachment.
   Convert to
   owned Rust inputs while attached, then detach around blocking `log`, query,
   flush, shutdown and synchronization waits. Python callbacks are excluded.
3. Implement checked ergonomic Python-to-wire conversions and a stable
   `ObservabilityError(Exception)` carrying code/message/remediation. Python
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
class Logger:
    def __init__(self, config: LoggerConfig) -> None: ...
    def try_log(self, event: LogEvent) -> None: ...
    def log(self, event: LogEvent) -> None: ...
    def query(self, query: LogQuery) -> LogSnapshot: ...
    def health(self) -> LogHealth: ...
    def flush(self, timeout_ms: int = 2000) -> None: ...
    def shutdown(self, timeout_ms: int = 2000) -> None: ...
    def wait_stopped(self, timeout_ms: int = 2000) -> LogHealth: ...
    def __enter__(self) -> "Logger": ...
    def __exit__(self, exc_type, exc, tb) -> bool: ...

class ObservabilityError(Exception):
    code: str
    message: str
    remediation: Remediation
```

Rust embedding surface (uses public DTO/error types from the shared contract):

```rust
pub trait HostLoggingBackend: Send + Sync {
    fn try_log(&self, event: LogEventDto) -> Result<(), ObservabilityErrorDto>;
    fn log(&self, event: LogEventDto) -> Result<(), ObservabilityErrorDto>;
    fn query(&self, query: LogQueryDto) -> Result<LogSnapshotDto, ObservabilityErrorDto>;
    fn health(&self) -> Result<LogHealthDto, ObservabilityErrorDto>;
    fn flush(&self, timeout: std::time::Duration) -> Result<(), ObservabilityErrorDto>;
}
pub fn install_host_logger(
    module: &pyo3::Bound<'_, pyo3::types::PyModule>,
    backend: std::sync::Arc<dyn HostLoggingBackend>,
) -> pyo3::PyResult<()>;
```

The module exposes `get_host_logger() -> AttachedLogger`; it fails with a stable
not-attached error when no host installed a backend. `AttachedLogger` exposes
log/try_log/query/health/flush only. Host shutdown or Python handle drop never
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
crate is a B.6 publish artifact so hosts can consume this API from crates.io.

Python `LoggerConfig` requires `service` and `log_root`; optional level defaults
to info, file sink to enabled and console sink to disabled, matching the checked
core defaults. No user-provided Rust ownership pointers or callback objects are
accepted. `log`/`try_log` return None on accepted/filter-handled admission and
raise stable typed exceptions otherwise. Bounds reject bool-as-int, negative,
non-finite and overflowing timeout/query inputs.

Owned-mode lifecycle is `running -> stopping -> stopped` or terminal `failed`. Timeout is
a wait result, never a claim that a worker stopped. Shutdown closes admission
once, assigns exactly one owner to final core shutdown, and retains completion
and final health independently of Python reference counts. Already admitted
operations may finish; later log/query/flush calls fail with the stable closed
code. `health` and `wait_stopped` remain usable. Repeated shutdown waits on the
same completion; it never starts a second core shutdown. A worker failure is
observable via the saved stable error. Read-only handles cannot prevent final
ownership transfer indefinitely. Outstanding flush helpers are coalesced/bounded.

Normal context exit invokes explicit shutdown. If cleanup alone fails, raise
its error. If a body exception is already active, retain it and attach cleanup
failure information without masking it. GC finalization schedules best-effort
cleanup once without an unbounded interpreter-thread wait; finalization does
not guarantee persistence and must not call back into Python during teardown.
Document explicit context/shutdown as the supported durability path.

## Acceptance criteria (authoritative)

- AC1: Clean wheel installations run every supported operation and the typed
  example on the approved Python/platform matrix with no source-tree imports.
  File logs and bounded snapshots match shared Rust/TypeScript fixtures.
- AC2: A full queue or held sink does not stop an independent Python thread;
  concurrent log/query/flush/shutdown yields documented results, never PyO3
  borrow-check exceptions as lifecycle policy. Two owned instances remain isolated; an attached instance instead shares the
  host log/health/correlation and cannot shut it down or silently open a second
  writer. Host shutdown while Python is active produces stable closed outcomes.
- AC3: Timeout then late completion/failure is observed through wait_stopped;
  shutdown runs once; post-stop health persists; explicit and GC cleanup paths
  pass subprocess tests without process hangs or implicit global logger install.
- AC4: Wheel and source distributions contain stubs, py.typed and all required
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
Development installs alone do not satisfy validation.

## Paths to delete

None.

## Non-closure

No PyPI publication (B.6), standard-library Handler/context integration (B.5), implicit Rust
facade installation, Python callback sinks/redactors, follow stream, async API,
OTLP, Go, or whole-workspace public API parity.

## Technical references

[PyO3 detach guidance](https://pyo3.rs/main/parallelism) applies to every blocking
Rust call. [PyO3 thread safety](https://pyo3.rs/v0.29.0/class/thread-safety.html)
requires explicit concurrency design beyond releasing the GIL.
[PyO3 build and embedding guidance](https://pyo3.rs/main/building-and-distribution)
requires distinct linking configuration for embedding and extension builds.
[Maturin mixed-project guidance](https://www.maturin.rs/tutorial.html?highlight=stable)
informs the extension/package layout and wheel validation.
