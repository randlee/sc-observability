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

## IdentityError

| Occurrence | Disposition | Owning registry constant |
| --- | --- | --- |
| `sc-observability-types/src/process.rs`: open `ProcessIdentityResolver::resolve` legacy signature | Named compatibility: retained; typed counterpart is `typed::TypedProcessIdentityResolver::resolve` | `sc_observability_types::error_codes::IDENTITY_RESOLUTION_FAILED` |
| `sc-observability-types/src/errors.rs`: `IdentityError` definition + `DiagnosticInfo` impl | Named compatibility: `#[deprecated]`, retained struct/impl unchanged | (definition site, not a use site) |
| `sc-observability-types/src/lib.rs`: root re-export | Named compatibility: re-exported unchanged, `#[allow(deprecated)]` scoped to the re-export | n/a |
| `sc-observability-types/src/diagnostic.rs` tests | Named compatibility: exercises retained wrapper's `DiagnosticInfo` impl directly | n/a (test) |
| `sc-observability-types/tests/neutral_contracts.rs` | Typed production: exercises `typed::IdentityFailure::resolution_failed` and both adapter directions | `sc_observability_types::error_codes::IDENTITY_RESOLUTION_FAILED` |
| `sc-observability-log/src/mapping.rs` (frozen bridge import) | Named compatibility: copied BTIT source constructs `IdentityError` directly; immutable per `import-provenance.json`; CI allows `deprecated` only for this crate (`.github/workflows/ci.yml`) | n/a (bridge-owned) |

Typed production constructor: `IdentityFailure::resolution_failed`. Verified
by `error_registry_parity.rs::identity_failure_matches_owning_registry`.

## InitError

| Occurrence | Disposition | Owning registry constant |
| --- | --- | --- |
| `sc-observability/src/builder.rs`, `runtime.rs`: `Logger::new`, `LoggerBuilder::build*` legacy constructors | Named compatibility: retained; typed counterparts are `Logger::new_typed`, `LoggerBuilder::build_typed`/`build_with_level_owner_typed` | `sc_observability::error_codes::LOGGER_INIT_FAILED` |
| `sc-observe/src/lib.rs`: `ObservabilityConfig::default_for`/`service_name`, `Observability::new`, `ObservabilityBuilder::build` | Named compatibility: retained; typed counterparts are the `*_typed` methods on the same types | `sc_observe::error_codes::OBSERVABILITY_INIT_FAILED` (and `sc_observability_types::error_codes::IDENTITY_RESOLUTION_FAILED` for the identity-resolution path) |
| `sc-observability-otlp/src/config.rs`, `lib.rs`: `TelemetryConfigBuilder::build`, `OtlpEndpoint::new`, `AuthHeader::new`, `Telemetry::new` | Named compatibility: retained; typed counterparts are `build_typed`/`new_typed` | `sc_observability_otlp::error_codes::TELEMETRY_INVALID_CONFIG`, `TELEMETRY_INVALID_PROTOCOL`, `TELEMETRY_EXPORTER_INIT_FAILED` |
| `sc-observability-log/src/handle.rs` (frozen bridge import) | Named compatibility: copied BTIT source calls `sc_observability::Logger::new` directly; immutable; CI-allowed `deprecated` for this crate only | n/a (bridge-owned) |

Typed production constructors: `InitFailure::logger_initialization`,
`observation_initialization`, `invalid_telemetry_config`,
`invalid_protocol`, `exporter_initialization`, `identity_resolution`.
Verified by `error_registry_parity.rs::init_failure_matches_owning_registry`.

## EventError

| Occurrence | Disposition | Owning registry constant |
| --- | --- | --- |
| `sc-observability/src/runtime.rs`: event validation/admission; `src/lib.rs`: `LogError`/`TryLogError` compatibility surface | Named compatibility: retained; typed counterparts are `Logger::log_typed`/`try_log_typed` returning `LogFailure`/`TryLogFailure` | `sc_observability::error_codes::LOGGER_INVALID_EVENT`, `LOGGER_SHUTDOWN`, `LOGGER_QUEUE_FULL`, `LOGGER_WRITER_DEGRADED`, `LOGGER_SHUTDOWN_TIMED_OUT` |
| `sc-observability-otlp/src/assembly.rs`: `SpanAssembler::push` | Named compatibility: retained; typed counterpart is `push_typed` | `sc_observability_otlp::error_codes::TELEMETRY_SPAN_ASSEMBLY_FAILED` |

Typed production constructors: `EventFailure::invalid_event`, `closed`,
`queue_full`, `writer_degraded`, `shutdown_timed_out`, `span_assembly`.
Verified by `error_registry_parity.rs::event_failure_matches_owning_registry`.

## FlushError

| Occurrence | Disposition | Owning registry constant |
| --- | --- | --- |
| `sc-observability/src/maintenance.rs`, `runtime.rs`: `Logger::flush` | Named compatibility: retained; typed counterpart is `flush_typed` | `sc_observability::error_codes::LOGGER_FLUSH_FAILED`, `LOGGER_WRITER_DEGRADED` |
| `sc-observe/src/lib.rs`: `Observability::flush` | Named compatibility: retained; typed counterpart is `flush_typed` | `sc_observe::error_codes::OBSERVABILITY_FLUSH_FAILED` |
| `sc-observability-otlp/src/lib.rs`: `Telemetry::flush` | Named compatibility: retained; typed counterpart is `flush_typed` | `sc_observability_otlp::error_codes::TELEMETRY_FLUSH_FAILED`, `TELEMETRY_SHUTDOWN` |

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
| `sc-observability-otlp/src/lib.rs`: open, crate-private `Exporter` trait; built-in log/trace/metric exporters; export/health/shutdown paths | Named compatibility: private exporter trait retained unchanged; internal exporter methods convert to `ExportFailure` at the production site; no public exporter API is added | `sc_observability_otlp::error_codes::TELEMETRY_EXPORT_FAILED` |

Typed production constructor: `ExportFailure::export`. Verified by
`error_registry_parity.rs::export_failure_matches_owning_registry`.

## Frozen bridge import (informational, not a disposition target)

`sc-observability-log`, `sc-observability-log-macros`, and
`sc-observability-log-consumer-check` are the B.1 immutable BTIT import
(`docs/plans/phase-b/import-provenance.json` pins their exact source bytes).
They use `IdentityError` and legacy `Logger::new`/`log`/`flush` directly and
are not migrated to typed call sites; `.github/workflows/ci.yml` runs their
clippy pass with `-A deprecated` (and every other lint still denied) instead
of editing their frozen source, per the copy's own acceptance record in
`docs/api-approvals/phase-b-log-import.md`.
