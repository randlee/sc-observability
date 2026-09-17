# Checked nine-family error inventory (B.1 integration)

This replaces the preliminary B.1a inventory now that B.1a-B.1d are merged and
B.1e has activated the migration warnings. Every occurrence of the nine legacy
wrapper families is classified with a typed-production or named-compatibility
disposition. The workspace parity test at
`crates/sc-observability-otlp/tests/error_registry_parity.rs` executes every
row's owning-crate registry-constant claim below; it constructs each typed
family's named production constructor and asserts the resulting diagnostic
code equals the `error_codes` constant cited in this table, so a renamed or
removed registry constant fails that test instead of only drifting silently.

Neutral typed values and traits (`sc_observability_types::typed`) depend only
on `sc-observability-types` itself; they do not depend on `sc-observability`,
`sc-observe`, or `sc-observability-otlp` (verified by
`sc-observability-types/Cargo.toml` having no such dependency and by
`cargo tree -p sc-observability-types` showing no runtime-crate edge).

## AC-by-AC source reconciliation (B.1a-B.1d)

Registry parity proves each typed constructor's diagnostic code matches its
owning registry constant, but not that each sprint's contracted API surface
actually exists or that its acceptance criteria hold against merged source.
This section records that separate check.

- **B.1a** (`sprint-b-1a-error-api.md`): AC1 confirmed — all nine
  `*FailureKind` enums have an `Unclassified` variant reached by unknown and
  cross-family codes (`typed.rs` fallback match arms and the
  `unknown`/`cross_family` test fixtures at lines ~1236-1246). AC2 confirmed —
  legacy/typed conversions compare the same `backtrace()` pointer address
  across round trips (`typed.rs:652-1118`). AC3 confirmed — the
  `Legacy*Adapter`/`Typed*Adapter` structs (`typed.rs:430-520`) are single-call
  `.map_err(Into::into)` delegations with no recursion or retry, and all five
  extension traits are declared `Send + Sync`. AC4 (no premature deprecation)
  is now moot: B.1b-B.1d have landed and B.1e's later, separately-gated
  activation is the intended trigger.
- **B.1b** (`sprint-b-1b-logger-errors.md`): AC1/AC4 confirmed — every
  contracted method (`Logger::{new_typed, new_with_level_owner_typed,
  log_typed, try_log_typed, try_log_with_outcome_typed, flush_typed}`,
  `LoggerBuilder::{new_typed, build_typed, build_with_level_owner_typed}`)
  exists with the exact contracted signature in `runtime.rs`/`builder.rs`; the
  legacy methods remain the compatibility boundary. AC2 confirmed — `typed.rs`
  in `sc-observability` defines `TypedLogSink`, `legacy_sink`, and
  `typed_sink` exactly as contracted. AC3 ("any internal compatibility
  accommodation is isolated and recorded without altering the source import
  SHA") is the one AC **not yet fully satisfied**: the narrow
  `#[allow(deprecated, reason = ...)]` accommodations in the bridge are
  isolated (per-call-site, reason-bearing), but "recorded" requires the
  `import-provenance.json` adaptation entries lobs is still landing (tracked
  under B1I-C01); the top-level accepted `source_commit` itself is unchanged.
- **B.1c** (`sprint-b-1c-observation-errors.md`): all six contracted methods
  (`ObservabilityConfig::{default_for_typed, service_name_typed}`,
  `Observability::{new_typed, flush_typed, shutdown_typed}`,
  `ObservabilityBuilder::build_typed`) exist in `sc-observe/src/lib.rs` with
  the exact contracted signatures. B.1c owns no dedicated legacy family of its
  own; its registry constants are exercised through the shared `InitFailure`/
  `FlushFailure`/`ShutdownFailure`/`ProjectionFailure`/`SubscriberFailure`
  parity tests.
- **B.1d** (`sprint-b-1d-telemetry-errors.md`): all seven contracted methods
  (`OtlpEndpoint::new_typed`, `AuthHeader::new_typed`,
  `TelemetryConfigBuilder::build_typed`, `SpanAssembler::push_typed`,
  `Telemetry::{new_typed, flush_typed, shutdown_typed}`) exist in
  `config.rs`/`assembly.rs`/`lib.rs` with the exact contracted signatures. The
  private exporter traits' disposition is corrected above (typed production,
  not compatibility) — this reconciliation is what caught that inaccuracy.

## IdentityError

| Occurrence | Disposition | Owning registry constant |
| --- | --- | --- |
| `sc-observability-types/src/process.rs`: open `ProcessIdentityResolver::resolve` legacy signature | Named compatibility: retained; typed counterpart is `typed::TypedProcessIdentityResolver::resolve` | `sc_observability_types::error_codes::IDENTITY_RESOLUTION_FAILED` |
| `sc-observability-types/src/errors.rs`: `IdentityError` definition + `DiagnosticInfo` impl | Named compatibility: `#[deprecated]`, retained struct/impl unchanged | (definition site, not a use site) |
| `sc-observability-types/src/lib.rs`: root re-export | Named compatibility: re-exported unchanged, `#[allow(deprecated)]` scoped to the re-export | n/a |
| `sc-observability-types/src/diagnostic.rs` tests | Named compatibility: exercises retained wrapper's `DiagnosticInfo` impl directly | n/a (test) |
| `sc-observability-types/tests/neutral_contracts.rs` | Typed production: exercises `typed::IdentityFailure::resolution_failed` and both adapter directions | `sc_observability_types::error_codes::IDENTITY_RESOLUTION_FAILED` |
| `sc-observability-log/src/mapping.rs` (frozen bridge import): constructors, `resolve()` implementations, and test helpers referencing the `.0` field and downcasting to `IdentityError` (18 clippy diagnostic sites as of this writing; exact locations in the `cargo clippy --all-targets --all-features` output) | Named compatibility: copied BTIT source constructs `IdentityError` directly; immutable per `import-provenance.json`; per-call-site `#[allow(deprecated, reason = ...)]` annotations are being coordinated with lobs as a new documented adaptation kind in `import-provenance.json` rather than a workspace- or bridge-wide clippy suppression (in progress; not yet landed) | n/a (bridge-owned) |

Typed production constructor: `IdentityFailure::resolution_failed`. Verified
by `error_registry_parity.rs::identity_failure_matches_owning_registry`.

## InitError

| Occurrence | Disposition | Owning registry constant |
| --- | --- | --- |
| `sc-observability/src/builder.rs`, `runtime.rs`: `Logger::new`, `LoggerBuilder::build*` legacy constructors | Named compatibility: retained; typed counterparts are `Logger::new_typed`, `LoggerBuilder::build_typed`/`build_with_level_owner_typed` | `sc_observability::error_codes::LOGGER_INIT_FAILED` |
| `sc-observe/src/lib.rs`: `ObservabilityConfig::default_for`/`service_name`, `Observability::new`, `ObservabilityBuilder::build` | Named compatibility: retained; typed counterparts are the `*_typed` methods on the same types | `sc_observe::error_codes::OBSERVABILITY_INIT_FAILED` (and `sc_observability_types::error_codes::IDENTITY_RESOLUTION_FAILED` for the identity-resolution path) |
| `sc-observability-otlp/src/config.rs`, `lib.rs`: `TelemetryConfigBuilder::build`, `OtlpEndpoint::new`, `AuthHeader::new`, `Telemetry::new` | Named compatibility: retained; typed counterparts are `build_typed`/`new_typed` | `sc_observability_otlp::error_codes::TELEMETRY_INVALID_CONFIG`, `TELEMETRY_INVALID_PROTOCOL`, `TELEMETRY_EXPORTER_INIT_FAILED` |
| `sc-observability-log/src/handle.rs:903` (frozen bridge import) | Named compatibility: copied BTIT source calls `sc_observability::Logger::new` directly; immutable; per-call-site allowance coordinated with lobs (in progress; not yet landed), same mechanism as the `IdentityError` bridge row above | n/a (bridge-owned) |

Typed production constructors: `InitFailure::logger_initialization`,
`observation_initialization`, `invalid_telemetry_config`,
`invalid_protocol`, `exporter_initialization`, `identity_resolution`.
Verified by `error_registry_parity.rs::init_failure_matches_owning_registry`.

## EventError

| Occurrence | Disposition | Owning registry constant |
| --- | --- | --- |
| `sc-observability/src/runtime.rs`: event validation/admission; `src/lib.rs`: `LogError`/`TryLogError` compatibility surface | Named compatibility: retained; typed counterparts are `Logger::log_typed`/`try_log_typed` returning `LogFailure`/`TryLogFailure` | `sc_observability::error_codes::LOGGER_INVALID_EVENT`, `LOGGER_SHUTDOWN`, `LOGGER_QUEUE_FULL`, `LOGGER_WRITER_DEGRADED`, `LOGGER_SHUTDOWN_TIMED_OUT` |
| `sc-observability-otlp/src/assembly.rs`: `SpanAssembler::push` | Named compatibility: retained; typed counterpart is `push_typed` | `sc_observability_otlp::error_codes::TELEMETRY_SPAN_ASSEMBLY_FAILED` |
| `sc-observability-log/src/control.rs:111`, `handle.rs:504` (frozen bridge import): `.try_log_with_outcome(event)` calls the deprecated `Logger::try_log_with_outcome`, returning `TryLogError` (`TryLogError::InvalidEvent` wraps core `EventError` directly, per `sc-observability/src/lib.rs`'s `From<TryLogError> for TryLogFailure` impl) | Named compatibility: copied BTIT source calls the legacy admission method directly; immutable; per-call-site allowance coordinated with lobs (in progress; not yet landed), same mechanism as the other bridge rows in this inventory | n/a (bridge-owned) |

Typed production constructors: `EventFailure::invalid_event`, `closed`,
`queue_full`, `writer_degraded`, `shutdown_timed_out`, `span_assembly`.
Verified by `error_registry_parity.rs::event_failure_matches_owning_registry`.

## FlushError

| Occurrence | Disposition | Owning registry constant |
| --- | --- | --- |
| `sc-observability/src/maintenance.rs`, `runtime.rs`: `Logger::flush` | Named compatibility: retained; typed counterpart is `flush_typed` | `sc_observability::error_codes::LOGGER_FLUSH_FAILED`, `LOGGER_WRITER_DEGRADED` |
| `sc-observe/src/lib.rs`: `Observability::flush` | Named compatibility: retained; typed counterpart is `flush_typed` | `sc_observe::error_codes::OBSERVABILITY_FLUSH_FAILED` |
| `sc-observability-otlp/src/lib.rs`: `Telemetry::flush` | Named compatibility: retained; typed counterpart is `flush_typed` | `sc_observability_otlp::error_codes::TELEMETRY_FLUSH_FAILED`, `TELEMETRY_SHUTDOWN` |
| `sc-observability-log/src/handle.rs:716,847` (frozen bridge import): `.logger.flush()` calls the deprecated `sc_observability::Logger::flush`, returning core `sc_observability_types::FlushError` (the type also appears at `handle.rs:689` as the `Completed` variant's field type) | Named compatibility: copied BTIT source calls the legacy `Logger::flush` directly; immutable; per-call-site allowance coordinated with lobs (in progress; not yet landed), same mechanism as the `IdentityError`/`InitError` bridge rows above | n/a (bridge-owned) |

Typed production constructors: `FlushFailure::logger_flush`,
`writer_degraded`, `observation_flush`, `telemetry_flush`, `closed`.
Verified by `error_registry_parity.rs::flush_failure_matches_owning_registry`.

## ShutdownError

| Occurrence | Disposition | Owning registry constant |
| --- | --- | --- |
| `sc-observe/src/lib.rs`: `Observability::shutdown` | Named compatibility: retained; typed counterpart is `shutdown_typed` | `sc_observability::error_codes::LOGGER_WRITER_DEGRADED`, `LOGGER_SHUTDOWN_TIMED_OUT` |
| `sc-observability-otlp/src/lib.rs`: `Telemetry::shutdown`, flush/export conversion helpers | Named compatibility: retained; typed counterpart is `shutdown_typed` | `sc_observability_otlp::error_codes::TELEMETRY_FLUSH_FAILED`, `TELEMETRY_INCOMPLETE_SPAN_DROPPED` |

Typed production constructors: `ShutdownFailure::telemetry_flush`,
`incomplete_spans`, `writer_degraded`, `timed_out`. Verified by
`error_registry_parity.rs::shutdown_failure_matches_owning_registry`.

## ProjectionError

| Occurrence | Disposition | Owning registry constant |
| --- | --- | --- |
| `sc-observe/src/lib.rs`: custom `LogProjector`/`SpanProjector`/`MetricProjector` fixtures and routing | Named compatibility: open legacy traits retained unchanged; typed counterparts are `TypedLogProjector`/`TypedSpanProjector`/`TypedMetricProjector` entered via `legacy_*_projector`/`typed_*_projector` adapters | `sc_observe::error_codes::OBSERVATION_ROUTING_FAILURE` |
| `sc-observability-otlp/src/projectors.rs`: built-in log/trace/metric exporter projectors and telemetry conversion | Named compatibility: retained; typed adapters enter the same registration surface | `sc_observability_otlp::error_codes::TELEMETRY_SHUTDOWN`, `TELEMETRY_EXPORT_FAILED`, `TELEMETRY_SPAN_ASSEMBLY_FAILED` |

Typed production constructors: `ProjectionFailure::telemetry_closed`,
`telemetry_export`, `span_assembly`, `routing`. Verified by
`error_registry_parity.rs::projection_failure_matches_owning_registry`.

## SubscriberError

| Occurrence | Disposition | Owning registry constant |
| --- | --- | --- |
| `sc-observe/src/lib.rs`: public `ObservationSubscriber` trait, custom subscriber fixtures, routing | Named compatibility: open legacy trait retained unchanged; typed counterpart is `TypedObservationSubscriber` entered via `legacy_subscriber`/`typed_subscriber` | `sc_observe::error_codes::OBSERVATION_ROUTING_FAILURE` |

Typed production constructor: `SubscriberFailure::routing`. Verified by
`error_registry_parity.rs::subscriber_failure_matches_owning_registry`.

## LogSinkError

| Occurrence | Disposition | Owning registry constant |
| --- | --- | --- |
| `sc-observability/src/lib.rs`: public `LogSink` trait (`write`/`flush`) | Named compatibility: open legacy trait retained unchanged; typed counterpart is `typed::TypedLogSink` entered via `legacy_sink`/`typed_sink` | `sc_observability::error_codes::LOGGER_SINK_WRITE_FAILED` |
| `sc-observability/src/sinks.rs`: built-in file/console sink write/flush/maintenance | Named compatibility: retained `LogSink` impls backed by the same typed implementation | `sc_observability::error_codes::LOGGER_MAINTENANCE_FAILED` |
| `sc-observability/src/sinks.rs`: feature-gated `fault-injection` sink path | Named compatibility: retained, feature-invariant `FaultInjected` kind | `sc_observability::error_codes::LOGGER_SINK_FAULT_INJECTED` (`fault-injection` feature) |

Typed production constructors: `LogSinkFailure::write`, `maintenance`,
`fault_injected`. `write`/`maintenance` are verified by
`error_registry_parity.rs::log_sink_failure_matches_owning_registry`.
`fault_injected`'s owning code is gated behind `sc-observability`'s
`fault-injection` feature; that case is verified in-crate by
`sc-observability/src/sinks.rs`'s
`sinks::tests::fault_injected_failure_matches_owning_registry` (run with
`cargo test -p sc-observability --features fault-injection`) instead of
adding a dev-only feature edge to `sc-observability-otlp` that would drift
`scripts/ci/validate_dependency_bans.sh`'s and
`validate_repo_boundaries.sh`'s allowed dev-dependency baseline.

## ExportError

| Occurrence | Disposition | Owning registry constant |
| --- | --- | --- |
| `sc-observability-otlp/src/lib.rs`: three crate-private traits `LogExporter::export_logs`, `TraceExporter::export_spans`, `MetricExporter::export_metrics`; built-in log/trace/metric exporters; export/health/shutdown paths | Typed production: all three private traits are authored directly against `Result<(), ExportFailure>`, and no `ExportError` type exists anywhere in this crate to be a compatibility form of (the public `TelemetryError::ExportFailure` variant is that unrelated, retained legacy wrapper's own variant name, not a use of the nine-family `ExportError`/`ExportFailure` pair); no public exporter API is added | `sc_observability_otlp::error_codes::TELEMETRY_EXPORT_FAILED` |

Typed production constructor: `ExportFailure::export`. Verified by
`error_registry_parity.rs::export_failure_matches_owning_registry`.

`sc-observability-log`, `sc-observability-log-macros`, and
`sc-observability-log-consumer-check` are the B.1 immutable BTIT import
(`docs/plans/phase-b/import-provenance.json` pins their exact source bytes).
Of the nine legacy families, the bridge only touches `IdentityError`,
`InitError`, `EventError` (via `TryLogError`'s `.try_log_with_outcome`
compatibility path), and `FlushError` — each occurrence is listed as an
explicit disposition row in that family's table above, not excluded here. The
bridge also defines its own crate-local `InitError`/`FlushError`/
`ShutdownError` enums in `sc-observability-log/src/error.rs` (distinct types,
same names as the deprecated core wrappers); those are bridge-domain types,
not occurrences of the nine legacy families, and are out of this inventory's
scope. There is no bridge call site using the deprecated `Logger::log` or
`Logger::log_with_outcome` methods. `ShutdownError`, `ProjectionError`,
`SubscriberError`, `LogSinkError`, and `ExportError` have no bridge
occurrences at all.
