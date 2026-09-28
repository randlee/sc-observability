# Error API migration to 2.0

This is the ADR-017 replacement contract, not the historical Phase B additive
`*_typed` rollout. D18 activates the canonical root re-exports and removes the
duplicate logger error/method surfaces. New and migrated callers use
`Logger::new`, `LoggerBuilder::new`, `LoggerBuilder::build`, `Logger::log`,
`Logger::try_log`, `Logger::try_log_with_outcome`, and `Logger::flush`; these
methods return the canonical errors directly. The suffixed `*_typed` logger
methods and their parallel legacy wrappers are not part of the 2.0 API.

## Nine wrapper families

Import the canonical same-name enum from `sc_observability_types` after D18
activation (or the consuming crate's approved re-export). During staged
implementation the canonical definitions live under `v2`; that staging path
is not an additional permanent public API promise.

| 1.x wrapper / additive typed family | Canonical 2.0 enum | Named causes |
| --- | --- | --- |
| `IdentityError` / `IdentityFailure` | `IdentityError` | `Process` |
| `InitError` / `InitFailure` | `InitError` | `Configuration`, `Runtime` |
| `EventError` / `EventFailure` | `EventError` | `Validation`, `Routing` |
| `FlushError` / `FlushFailure` | `FlushError` | `Drain` |
| `ShutdownError` / `ShutdownFailure` | `ShutdownError` | `Timeout`, `Drain` |
| `ProjectionError` / `ProjectionFailure` | `ProjectionError` | `Projection` |
| `SubscriberError` / `SubscriberFailure` | `SubscriberError` | `Subscriber` |
| `LogSinkError` / `LogSinkFailure` | `LogSinkError` | `Write`, `Flush` |
| `ExportError` / `ExportFailure` | `ExportError` | `Transport`, `BlockingBackendInAsyncContext`, `AsyncLifecycleRequired`, `RuntimeTerminated`, `LifecycleTimeout`, `QueueFull`, `WorkerTerminated`, `ShutdownCancelledRetry`, `RetryDeadlineExhausted`, `NonRetryableHttpStatus`, `RetryAttemptsExhausted`, `TerminalExportFailure` |

The definitions in `crates/sc-observability-types/src/errors_v2.rs` are the
source of truth for additional variants and fields. The enums are
`#[non_exhaustive]`: downstream matches need a fallback arm. Replace tuple
wrapper construction and `*FailureKind` classifiers with the named variant
at the operation that knows the cause. Do not reconstruct the cause from a
formatted message, downcast a native error to guess it, or add a parallel
classifier.

## Keep diagnostic and source identity

Each named error carries its original boxed `ErrorContext`. Move that context
when retyping an error; do not build a replacement diagnostic from its display
text. Preserve diagnostic code, structured context, remediation, source chain,
and captured backtrace. Read stable codes from the owning `error_codes`
registry and match enum variants for control flow. Do not use a new code
string as a surrogate enum discriminator.

A construction pattern against the staged contract is:

```rust
use sc_observability_types::{ErrorContext, v2::FlushError};

fn drain_failure(context: Box<ErrorContext>) -> FlushError {
    FlushError::Drain { context }
}
```

The canonical types are root re-exports. For logging lifecycle calls,
distinguish invalid configuration from runtime
startup failure, flush drain failure from shutdown timeout, and shutdown drain
failure from timeout. The five D18 log regressions (`api_freeze`,
`flush_single_flight`, `init_runtime_start`, `shutdown_timeout`,
`static_level_cap`) must use direct canonical imports and check these causes,
stable codes, and source identity. A passing legacy-wrapper test is insufficient.

`DetachError` remains a companion-owned exception at the host bridge boundary.
Detaching an attachment does not grant ownership of the host logger or allow
it to shut the host down. Preserve its timeout/retry semantics.

## Custom sinks and projectors

Implement the canonical open `LogSink` methods with `LogSinkError`, retaining
`Send + Sync`, object safety and `Arc<dyn LogSink>` registration. A write error
uses `Write { context }`; a flush error uses `Flush { context }`. Keep sink
health reporting and fail-open fan-out behavior. The final consumer must not
rely on `TypedLogSink`, `legacy_sink`, `typed_sink`, or duplicate legacy/typed
projector adapters merely to reach the normal registration boundary.

Keep routing and projector ownership in `sc-observe`. Return canonical
`ProjectionError`/`SubscriberError` with original context from those open
extension points. Do not import OTLP transport types into neutral projectors
or add ATM-specific payload handling to shared crates.

## Call-site and wire migration

For canonical 2.0, use the final owner methods and named errors. Do not retain
parallel legacy and `*_typed` methods: migrate callers to the unsuffixed
canonical signatures and remove the superseded duplicates. Preserve logger
admission, flush, shutdown ownership and terminal outcomes while changing
error handling.

Update language projections through the existing canonical DTO/schema path.
Preserve stable code, remediation and source projection; do not derive the
wire category by comparing display strings. See [the release migration
reference](migration.md) for the already observed infallible health projection
and neutral signal serialization changes.

## Executable qualification

The existing consumers are under `scripts/ci/fixtures/error-migration/`.
D18's final `python3 scripts/ci/validate_error_migration.py` must execute the
migrated canonical recipes and reject the intentionally old source. At this
release-governance layer that validator still implements the Phase B warning
contract; its result does not prove canonical removal. The main D18 work owns
that adaptation and the full default/all-features workspace tests.
