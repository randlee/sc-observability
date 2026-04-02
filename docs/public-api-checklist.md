# SC-Observability Public API Checklist

**Status**: Approved
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
- Per-crate `error_codes.rs` files hold `ErrorCode` string constants, which are
  separate from error type definitions.
- Concrete health report types are centralized in `sc-observability-types`.
- Shared constants are centralized here as SSOT per requirements.

- [x] `ErrorCode`
- [x] `error_codes`
- [x] `constants`
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
- [x] `ProcessIdentityPolicy`
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
- [x] `DurationMs`
- [x] `SpanRecord<S>`
- [x] `SpanEvent`
- [x] `SpanSignal`
- [x] `MetricKind`
- [x] `MetricRecord`
- [x] `LoggingHealthState`
- [x] `SinkHealthState`
- [x] `SinkHealth`
- [~] `LoggingHealthReport`
- [x] `Timestamp` (UTC-enforced public type, not a plain alias)
- [~] `LogOrder`
- [~] `LogFieldMatch`
- [~] `LogQuery`
- [~] `LogSnapshot`
- [~] `QueryError`
- [~] `QueryHealthState`
- [~] `QueryHealthReport`
- [~] `TelemetryHealthProvider`
- [x] `ObservationHealthState`
- [~] `ObservabilityHealthReport`
- [x] `TelemetryHealthState`
- [x] `ExporterHealthState`
- [x] `ExporterHealth`
- [x] `TelemetryHealthReport`
- [x] `ObservationSubscriber<T>`
- [x] `ObservationFilter<T>`
- [x] `LogProjector<T>`
- [x] `SpanProjector<T>`
- [x] `MetricProjector<T>`
- [x] `SubscriberRegistration<T>`
- [x] `ProjectionRegistration<T>`
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
- trace correlation uses `TraceId` / `SpanId`
- ATM metadata is not part of the core schema

- `Timestamp` is UTC-only and serializes in canonical UTC RFC3339 form.

## 3. `sc-observability`

### Finalized Public Types

- [x] `error_codes`
- [x] `LoggerConfig`
- [x] `RotationPolicy`
- [x] `RetentionPolicy`
- [x] `RedactionPolicy`
- [x] `Redactor`
- [x] `Logger`
- [~] `Logger::query(&self, &LogQuery) -> Result<LogSnapshot, QueryError>`
- [~] `Logger::follow(&self, LogQuery) -> Result<LogFollowSession, QueryError>`
- [~] `LogFollowSession`
- [~] `JsonlLogReader`
- [x] `JsonlFileSink`
- [x] `ConsoleSink`
- [x] `LogSink`
- [x] `LogFilter`
- [x] `SinkRegistration`

Internal-only:

- [x] `LogEmitter`

### Finalized Public Rules

- `LoggerConfig.service_name` is required
- file sink is enabled by default
- console sink is disabled by default
- sink failures are fail-open

## 4. `sc-observe`

### Finalized Public Types

- [x] `error_codes`
- [~] `ObservabilityHealthReport`
- [x] `ObservationError`
- [x] `ObservationHealthState`
- [x] `ObservabilityConfig`
- [x] `ObservabilityBuilder`
- [~] `ObservabilityBuilder::with_telemetry_health_provider(...)`
- [x] `Observability`

Internal-only:

- [x] `ObservationEmitter<T>`

### Finalized Public Rules

- `ObservationEmitter<T>` is intentionally per-type
- registration is construction-time only
- routing order is deterministic
- `ObservabilityConfig` does not own OTLP configuration

## 5. `sc-observability-otlp`

### Finalized Public Types

- [x] `error_codes`
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
- [~] `TelemetryProjectors<T>`

Internal-only:

- [x] `SpanEmitter`
- [x] `MetricEmitter`

### Finalized Public Rules

- `TelemetryConfig` is application-constructed
- OTLP attaches through shipped projector-registration helpers
- invalid OTLP config fails at `Telemetry::new(...)`
- `TelemetryError::Shutdown` is returned after shutdown

## 6. API Freeze Gates

API freeze is progressive by crate and sprint, not global at Sprint 1.

- Sprint 1 closes only when the `sc-observability-types` public API is frozen
  for that crate.
- Sprint 2 closes only when the `sc-observability` public API is frozen for
  that crate.
- Sprint 3 closes only when the `sc-observe` and
  `sc-observability-otlp` recovery-scope public APIs are frozen together.
- Sprint 4 / pre-release closes only when all four crate API surfaces are
  confirmed finalized together.

At each crate freeze gate:

- every public item for the crate must be either implemented or explicitly
  marked `internal-only`
- any newly introduced public type or trait must be added here first
- the API docs, requirements, and implementation must all agree on names and
  signatures for that crate
