# SC-Observability Public API Checklist

**Status**: Draft for review
**Purpose**: Track the intended public API so implementation does not invent or
change the public surface opportunistically.

## 1. Usage

Each item uses one of these markers:

- `[ ]` pending
- `[~]` designed, not implemented
- `[x]` finalized and implemented

Internal-only items should be called out explicitly rather than kept in the
public list silently.

## 2. `sc-observability-types`

### Finalized Public Types

Note:
- Error types are defined in `sc-observability-types` (TYP-030) and
  re-exported by their respective crates.
- Per-crate `error_codes.rs` files (SRC-001/SRC-002) hold `ErrorCode` string
  constants, which are separate from error type definitions.
- Concrete health report types are centralized in `sc-observability-types` per
  the SSOT ruling.
- Shared constants are centralized here as SSOT per requirements.

- [x] `ErrorCode`
- [x] `error_codes` — per-crate stable `ErrorCode` constants registry
  (SRC-001/SRC-002)
- [x] `constants` — `sc-observability-types/src/constants.rs` (SSOT for all
  shared cross-crate constants per TYP-031)
- [x] `ValueValidationError`
- [x] `ToolName`
- [x] `EnvPrefix`
- [x] `ServiceName`
- [x] `TargetCategory`
- [x] `ActionName`
- [x] `MetricName`
- [x] `RecoverableSteps`
- [x] `Remediation`
- [x] `Diagnostic`
- [x] `DiagnosticInfo`
- [x] `DiagnosticSummary`
- [x] `ErrorContext`
- [x] `IdentityError`
- [x] `Level`
- [x] `LevelFilter`
- [x] `ProcessIdentity`
- [x] `ProcessIdentityPolicy` — intentionally no serde; runtime policy only
- [x] `ProcessIdentityResolver`
- [x] `TraceId`
- [x] `SpanId`
- [x] `TraceContext`
- [x] `StateTransition`
- [x] `Observation<T>`
- [x] `LogEvent`
- [x] `SpanStatus`
- [x] `SpanStarted`
- [x] `SpanEnded`
- [x] `SpanRecord<S>`
- [x] `SpanEvent`
- [x] `SpanSignal`
- [x] `MetricKind`
- [x] `MetricRecord`
- [x] `LoggingHealthState`
- [x] `SinkHealthState`
- [x] `SinkHealth`
- [x] `LoggingHealthReport`
- [x] `ObservationHealthState`
- [x] `ObservabilityHealthReport`
- [x] `TelemetryHealthState`
- [x] `ExporterHealthState`
- [x] `ExporterHealth`
- [x] `TelemetryHealthReport`
- [x] `ObservationSubscriber<T>`
- [x] `ObservationFilter<T>`
- [x] `LogProjector<T>`
- [x] `SpanProjector<T>`
- [x] `MetricProjector<T>`
- [x] `SubscriberRegistration<T>` — intentionally no serde; construction-time registration only
- [x] `ProjectionRegistration<T>` — intentionally no serde; construction-time registration only
- [x] `InitError`
- [x] `EventError`
- [x] `FlushError`
- [x] `ShutdownError`
- [x] `ProjectionError`
- [x] `SubscriberError`
- [x] `LogSinkError`
- [x] `ExportError`
- [x] `ObservationError`
- [x] `TelemetryError`

### Finalized Public Rules

- span lifecycle is typestate-only on producer-facing APIs
- `Diagnostic` always carries remediation
- timestamps are UTC-only
- trace correlation uses `TraceId` / `SpanId`
- ATM metadata is not part of the core schema

## 3. `sc-observability`

### Finalized Public Types

- [x] `error_codes` — per-crate stable `ErrorCode` constants registry
  (SRC-001/SRC-002)
- [x] `LoggerConfig`
- [x] `RotationPolicy`
- [x] `RetentionPolicy`
- [x] `RedactionPolicy`
- [x] `Redactor`
- [x] `Logger`
- [x] `JsonlFileSink`
- [x] `ConsoleSink`
- [x] `LogSink`
- [x] `LogFilter`
- [x] `SinkRegistration`

Internal-only:

- [x] `LogEmitter` — crate-local sealed logging injection trait (LOG-024;
  `architecture.md` §3.2; internal-only, not part of public API)

### Finalized Public Rules

- `LoggerConfig.service_name` is required
- file sink is enabled by default
- console sink is disabled by default
- sink failures are fail-open

## 4. `sc-observe`

### Finalized Public Types

- [x] `error_codes` — per-crate stable `ErrorCode` constants registry
  (SRC-001/SRC-002)
- [x] `ObservabilityConfig`
- [x] `ObservabilityBuilder`
- [x] `Observability`

Internal-only:

- [x] `ObservationEmitter<T>` — crate-local sealed observation injection trait
  (OBS-025; `architecture.md` §3.3; internal-only, not part of public API)

### Finalized Public Rules

- `ObservationEmitter<T>` is intentionally per-type
- registration is construction-time only
- routing order is deterministic
- `ObservabilityConfig` does not own OTLP configuration

## 5. `sc-observability-otlp`

### Finalized Public Types

- [x] `error_codes` — per-crate stable `ErrorCode` constants registry
  (SRC-001/SRC-002)
- [x] `TelemetryConfig`
- [x] `TelemetryConfigBuilder`
- [x] `Telemetry`
- [x] `OtlpProtocol`
- [x] `OtelConfig`
- [x] `LogsConfig`
- [x] `TracesConfig`
- [x] `MetricsConfig`
- [x] `ResourceAttributes`
- [x] `SpanAssembler`
- [x] `CompleteSpan`
- [x] `LogExporter`
- [x] `TraceExporter`
- [x] `MetricExporter`

Internal-only:

- [~] `SpanEmitter`
- [~] `MetricEmitter`

Note:
- sealed/crate-local per `architecture.md` §3.4 and OTLP-022

### Finalized Public Rules

- `TelemetryConfig` is application-constructed
- OTLP attaches via projector registration
- invalid OTLP config fails at `Telemetry::new(...)`
- `TelemetryError::Shutdown` is returned after shutdown

## 6. API Freeze Gates

API freeze is progressive by crate and sprint, not global at Sprint 1.

- Sprint 1 closes only when the `sc-observability-types` public API is frozen
  for that crate.
- Sprint 2 closes only when the `sc-observability` public API is frozen for
  that crate.
- Sprint 3 closes only when the `sc-observe` public API is frozen for that
  crate.
- Sprint 4 closes only when the `sc-observability-otlp` public API is frozen
  for that crate.
- Sprint 6 / pre-release closes only when all four crate API surfaces are
  confirmed finalized together.

At each crate freeze gate:

- every public item for the crate must be either implemented or explicitly
  marked `internal-only`
- any newly introduced public type or trait must be added here first
- the API docs, requirements, and implementation must all agree on names and
  signatures for that crate
