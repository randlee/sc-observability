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

- [~] `ErrorCode`
- [~] `constants` — `sc-observability-types/src/constants.rs` (SSOT for all
  shared cross-crate constants per TYP-031)
- [~] `ValueValidationError`
- [~] `ToolName`
- [~] `EnvPrefix`
- [~] `ServiceName`
- [~] `TargetCategory`
- [~] `ActionName`
- [~] `MetricName`
- [~] `RecoverableSteps`
- [~] `Remediation`
- [~] `Diagnostic`
- [~] `DiagnosticInfo`
- [~] `DiagnosticSummary`
- [~] `ErrorContext`
- [~] `IdentityError`
- [~] `Level`
- [~] `LevelFilter`
- [~] `ProcessIdentity`
- [~] `ProcessIdentityPolicy`
- [~] `ProcessIdentityResolver`
- [~] `TraceId`
- [~] `SpanId`
- [~] `TraceContext`
- [~] `StateTransition`
- [~] `Observation<T>`
- [~] `LogEvent`
- [~] `SpanStatus`
- [~] `SpanStarted`
- [~] `SpanEnded`
- [~] `SpanRecord<S>`
- [~] `SpanEvent`
- [~] `SpanSignal`
- [~] `MetricKind`
- [~] `MetricRecord`
- [~] `LoggingHealthState`
- [~] `SinkHealthState`
- [~] `SinkHealth`
- [~] `LoggingHealthReport`
- [~] `ObservationHealthState`
- [~] `ObservabilityHealthReport`
- [~] `TelemetryHealthState`
- [~] `ExporterHealthState`
- [~] `ExporterHealth`
- [~] `TelemetryHealthReport`
- [~] `ObservationSubscriber<T>`
- [~] `ObservationFilter<T>`
- [~] `LogProjector<T>`
- [~] `SpanProjector<T>`
- [~] `MetricProjector<T>`
- [~] `SubscriberRegistration<T>`
- [~] `ProjectionRegistration<T>`
- [~] `InitError`
- [~] `EventError`
- [~] `FlushError`
- [~] `ShutdownError`
- [~] `ProjectionError`
- [~] `SubscriberError`
- [~] `LogSinkError`
- [~] `ExportError`
- [~] `ObservationError`
- [~] `TelemetryError`

### Finalized Public Rules

- span lifecycle is typestate-only on producer-facing APIs
- `Diagnostic` always carries remediation
- timestamps are UTC-only
- trace correlation uses `TraceId` / `SpanId`
- ATM metadata is not part of the core schema

## 3. `sc-observability`

### Finalized Public Types

- [~] `error_codes` — per-crate stable `ErrorCode` constants registry
  (SRC-001/SRC-002)
- [~] `LoggerConfig`
- [~] `RotationPolicy`
- [~] `RetentionPolicy`
- [~] `RedactionPolicy`
- [~] `Redactor`
- [~] `Logger`
- [~] `LogEmitter`
- [~] `LogSink`
- [~] `LogFilter`
- [~] `SinkRegistration`

### Finalized Public Rules

- `LoggerConfig.service_name` is required
- file sink is enabled by default
- console sink is disabled by default
- sink failures are fail-open

## 4. `sc-observe`

### Finalized Public Types

- [~] `error_codes` — per-crate stable `ErrorCode` constants registry
  (SRC-001/SRC-002)
- [~] `ObservabilityConfig`
- [~] `ObservabilityBuilder`
- [~] `Observability`
- [~] `ObservationEmitter<T>`

### Finalized Public Rules

- `ObservationEmitter<T>` is intentionally per-type
- registration is construction-time only
- routing order is deterministic
- `ObservabilityConfig` does not own OTLP configuration

## 5. `sc-observability-otlp`

### Finalized Public Types

- [~] `error_codes` — per-crate stable `ErrorCode` constants registry
  (SRC-001/SRC-002)
- [~] `TelemetryConfig`
- [~] `TelemetryConfigBuilder`
- [~] `Telemetry`
- [~] `OtlpProtocol`
- [~] `OtelConfig`
- [~] `LogsConfig`
- [~] `TracesConfig`
- [~] `MetricsConfig`
- [~] `ResourceAttributes`
- [~] `SpanAssembler`
- [~] `CompleteSpan`
- [~] `LogExporter`
- [~] `TraceExporter`
- [~] `MetricExporter`

Internal-only:

- `SpanEmitter`
- `MetricEmitter`

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
