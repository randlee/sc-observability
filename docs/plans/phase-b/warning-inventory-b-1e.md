# B.1e warning and exemption inventory

Checked against telemetry-parent merge `d4997029f83664331c8c4e3cc20005ae656d254a`
on `feature/phase-b-1e-migration-prep`. This is a preparation inventory. The
future B.1e attributes are not active on this branch.

## Version and activation

The B.P2 staged prerequisite is `1.3.0`; the next available workspace minor is
`1.4.0`. B.1e implementation selects and activates that exact version in
`#[deprecated(since = "1.4.0", ...)]`, then validates the warning policy. This
scoped preparation leaves those attributes pending; B.2 qualifies the B.1e
result and B.7 publishes it. Every future note must point to
`references/migrate-error-api.md` and name the exact replacement. `Logger::emit`
is the existing exception with its already-published `since = "1.2.0"` and
must retain its blocking/nonblocking note and behavior.

## Nine wrapper warnings

Each row is a type warning candidate. The wrapper remains structurally and
serially compatible; explicit naming or construction can warn even when the
owning method is exempt.

| Legacy wrapper (source) | Recommended failure | Required note/reason | Activation |
| --- | --- | --- | --- |
| `IdentityError` (`crates/sc-observability-types/src/errors.rs:9`) | `IdentityFailure` | Use typed identity resolution and match `IdentityFailureKind`; see guide. | Pending B.1e implementation |
| `InitError` (`crates/sc-observability-types/src/errors.rs:38`) | `InitFailure` | Use the owning `_typed` constructor and match `InitFailureKind`; owner-constructor method exemptions do not exempt this wrapper. | Pending B.1e implementation |
| `EventError` (`errors.rs:42`) | `EventFailure` | Use typed logger admission or `SpanAssembler::push_typed`; retain fallback matching. | Pending B.1e implementation |
| `FlushError` (`errors.rs:46`) | `FlushFailure` | Use `flush_typed` and preserve fail-open lifecycle behavior. | Pending B.1e implementation |
| `ShutdownError` (`errors.rs:50`) | `ShutdownFailure` | Use `shutdown_typed`; repeated shutdown remains successful where documented. | Pending B.1e implementation |
| `ProjectionError` (`errors.rs:54`) | `ProjectionFailure` | Use explicit typed projector traits/adapters; preserve source/context. | Pending B.1e implementation |
| `SubscriberError` (`errors.rs:58`) | `SubscriberFailure` | Use `TypedObservationSubscriber` plus `legacy_subscriber` at old registration boundaries. | Pending B.1e implementation |
| `LogSinkError` (`errors.rs:62`) | `LogSinkFailure` | Use `TypedLogSink` plus `sc_observability::typed::legacy_sink`. | Pending B.1e implementation |
| `ExportError` (`errors.rs:66`) | `ExportFailure` | Use typed exporter internals; public `TelemetryError` remains supported. | Pending B.1e implementation |

## Mapped method warning candidates

The contract maps each old `foo` to the same owner's implemented `foo_typed`,
retaining arguments, success values, ownership and lifecycle behavior. These
are the only method candidates in the merged B.1b–B.1d surface.

| Owner and source | Old symbol/signature result | Typed replacement | State |
| --- | --- | --- | --- |
| `LoggerBuilder` (`sc-observability/src/builder.rs:49`) | `new(LoggerConfig) -> Result<Self, InitError>` | `new_typed(...) -> Result<Self, InitFailure>` | Pending B.1e implementation |
| `Logger` (`sc-observability/src/runtime.rs:339`) | `builder(LoggerConfig) -> Result<LoggerBuilder, InitError>` | `builder_typed(...) -> Result<LoggerBuilder, InitFailure>` | Pending B.1e implementation |
| `Logger` (`runtime.rs:351`) | `new(LoggerConfig) -> Result<Self, InitError>` | `new_typed(...) -> Result<Self, InitFailure>` | Pending B.1e implementation |
| `Logger` (`runtime.rs:379`) | `log(LogEvent) -> Result<(), LogError>` | `log_typed(...) -> Result<(), LogFailure>` | Pending B.1e implementation |
| `Logger` (`runtime.rs:411`) | `try_log(LogEvent) -> Result<(), TryLogError>` | `try_log_typed(...) -> Result<(), TryLogFailure>` | Pending B.1e implementation |
| `Logger` (`runtime.rs:425`) | `try_log_with_outcome(LogEvent) -> Result<AdmissionOutcome, TryLogError>` | `try_log_with_outcome_typed(...) -> Result<AdmissionOutcome, TryLogFailure>` | Pending B.1e implementation |
| `Logger` (`runtime.rs:497`) | `flush() -> Result<(), FlushError>` | `flush_typed() -> Result<(), FlushFailure>` | Pending B.1e implementation |
| `ObservabilityConfig` (`sc-observe/src/lib.rs:86`) | `default_for(ToolName, PathBuf) -> Result<Self, InitError>` | `default_for_typed(...) -> Result<Self, InitFailure>` | Pending B.1e implementation |
| `ObservabilityConfig` (`lib.rs:116`) | `service_name() -> Result<ServiceName, InitError>` | `service_name_typed() -> Result<ServiceName, InitFailure>` | Pending B.1e implementation |
| `Observability` (`lib.rs:222`) | `new(ObservabilityConfig) -> Result<Self, InitError>` | `new_typed(...) -> Result<Self, InitFailure>` | Pending B.1e implementation |
| `Observability` (`lib.rs:343`) | `flush() -> Result<(), FlushError>` | `flush_typed() -> Result<(), FlushFailure>` | Pending B.1e implementation |
| `Observability` (`lib.rs:370`) | `shutdown() -> Result<(), ShutdownError>` | `shutdown_typed() -> Result<(), ShutdownFailure>` | Pending B.1e implementation |
| `ObservabilityBuilder` (`lib.rs:589`) | `build() -> Result<Observability, InitError>` | `build_typed() -> Result<Observability, InitFailure>` | Pending B.1e implementation |
| `OtlpEndpoint` (`sc-observability-otlp/src/config.rs:44`) | `new(impl Into<String>) -> Result<Self, InitError>` | `new_typed(impl Into<String>) -> Result<Self, InitFailure>` | Pending B.1e implementation |
| `AuthHeader` (`config.rs:98`) | `new(impl Into<String>) -> Result<Self, InitError>` | `new_typed(impl Into<String>) -> Result<Self, InitFailure>` | Pending B.1e implementation |
| `TelemetryConfigBuilder` (`config.rs:366`) | `build() -> Result<TelemetryConfig, InitError>` | `build_typed() -> Result<TelemetryConfig, InitFailure>` | Pending B.1e implementation |
| `SpanAssembler` (`sc-observability-otlp/src/assembly.rs:58`) | `push(SpanSignal) -> Result<Option<CompleteSpan>, EventError>` | `push_typed(...) -> Result<Option<CompleteSpan>, EventFailure>` | Pending B.1e implementation |
| `Telemetry` (`sc-observability-otlp/src/lib.rs:167`) | `new(TelemetryConfig) -> Result<Self, InitError>` | `new_typed(...) -> Result<Self, InitFailure>` | Pending B.1e implementation |
| `Telemetry` (`lib.rs:296`) | `flush() -> Result<(), FlushError>` | `flush_typed() -> Result<(), FlushFailure>` | Pending B.1e implementation |
| `Telemetry` (`lib.rs:377`) | `shutdown() -> Result<(), ShutdownError>` | `shutdown_typed() -> Result<(), ShutdownFailure>` | Pending B.1e implementation |

## Explicit method exemptions and retained APIs

These are supported and must not receive a method-level deprecation attribute:

- `LoggerBuilder::build` (`builder.rs:98`) remains the infallible legacy build;
  `build_typed` adds the fallible startup boundary.
- `Logger::new_with_level_owner` (`runtime.rs:361`) and
  `LoggerBuilder::build_with_level_owner` (`builder.rs:109`) retain their B.P1
  `Result<..., InitError>` signatures. Their `_typed` counterparts are additive.
  An explicit `InitError` use can still produce the wrapper warning.
- `Logger::emit` (`runtime.rs:476`) keeps the existing `1.2.0` deprecation and
  compatibility behavior. New migration guidance directs blocking callers to
  `log_typed()` and nonblocking callers to `try_log_typed()`; the old compiler
  note remains unchanged for the v1.2.0 baseline.
- `Observability::emit`, the three `Telemetry::emit_*` methods, health/query/
  follow methods, registration builders, config fields, old open traits and
  existing `LogError`, `TryLogError`, `ObservationError`, `TelemetryError` and
  `QueryError` are not mapped to a typed counterpart by the current contract.
- The corrected `TelemetryProjectors` surface contains only unchanged
  `with_log_projector`, `with_span_projector` and `with_metric_projector`.
  There are no `with_typed_*` builders. Typed implementations use explicit
  `sc_observability_types::typed::{legacy_log_projector,legacy_span_projector,
  legacy_metric_projector}` adapters before registration. A typed caller that
  reuses an existing legacy implementation uses
  `typed_identity`, `typed_subscriber`, `typed_log_projector`,
  `typed_span_projector`, `typed_metric_projector`, or
  `sc_observability::typed::typed_sink`; the reverse direction uses
  `legacy_identity`, `legacy_subscriber`, the three `legacy_*_projector`
  adapters, or `sc_observability::typed::legacy_sink`.

## Allowance and warning rules

Default-lint legacy consumers must build and run, with warnings visible. A
consumer-selected `-D warnings` or `-D deprecated` may fail by policy. Any
allow must be local, name the compatibility symbol/module and explain why the
old signature or adapter boundary is retained. No crate-root/workspace-wide
allow and no blanket suppression is valid. Typed counterparts must compile
under `#![deny(deprecated)]` without an allowance. Future validation must
separate a wrapper-use diagnostic from a method diagnostic by its span/item.

## Pending gates

The downstream validator, three Cargo fixtures (legacy/default, migrated/deny,
partial/local-allow), all-four-crate checks, trait-adapter checks and old
serialized-value goldens remain owned by the team lead/CI follow-up. They are
not activated or claimed by this docs-only child.
