# SC-Observability Public API Checklist

**Status**: Draft for review
**Purpose**: Track the intended public API so implementation does not invent or
change the public surface opportunistically.

## 1. Usage

Each item should be marked during implementation as one of:

- `finalized`
- `implemented`
- `internal-only`

No item should move from `finalized` back to provisional without an explicit
design change.

## 2. `sc-observability-types`

### Finalized Public Types

- `ErrorCode`
- `ValueValidationError`
- `ToolName`
- `EnvPrefix`
- `ServiceName`
- `TargetCategory`
- `ActionName`
- `MetricName`
- `RecoverableSteps`
- `Remediation`
- `Diagnostic`
- `DiagnosticInfo`
- `DiagnosticSummary`
- `ErrorContext`
- `IdentityError`
- `Level`
- `LevelFilter`
- `ProcessIdentity`
- `ProcessIdentityPolicy`
- `ProcessIdentityResolver`
- `TraceId`
- `SpanId`
- `TraceContext`
- `StateTransition`
- `Observation<T>`
- `LogEvent`
- `SpanStatus`
- `SpanStarted`
- `SpanEnded`
- `SpanRecord<S>`
- `SpanEvent`
- `SpanSignal`
- `MetricKind`
- `MetricRecord`
- `LoggingHealthState`
- `SinkHealthState`
- `SinkHealth`
- `LoggingHealthReport`
- `ObservationHealthState`
- `ObservabilityHealthReport`
- `TelemetryHealthState`
- `ExporterHealthState`
- `ExporterHealth`
- `TelemetryHealthReport`
- `CompleteSpan`
- `ObservationSubscriber<T>`
- `ObservationFilter<T>`
- `LogProjector<T>`
- `SpanProjector<T>`
- `MetricProjector<T>`
- `SubscriberRegistration<T>`
- `ProjectionRegistration<T>`
- `InitError`
- `EventError`
- `FlushError`
- `ShutdownError`
- `ProjectionError`
- `SubscriberError`
- `LogSinkError`
- `ExportError`
- `ObservationError`
- `TelemetryError`

### Finalized Public Rules

- span lifecycle is typestate-only on producer-facing APIs
- `Diagnostic` always carries remediation
- timestamps are UTC-only
- trace correlation uses `TraceId` / `SpanId`
- ATM metadata is not part of the core schema

## 3. `sc-observability`

### Finalized Public Types

- `LoggerConfig`
- `RotationPolicy`
- `RetentionPolicy`
- `RedactionPolicy`
- `Redactor`
- `Logger`
- `LogEmitter`
- `LogSink`
- `LogFilter`
- `SinkRegistration`

### Finalized Public Rules

- `LoggerConfig.service_name` is required
- file sink is enabled by default
- console sink is disabled by default
- sink failures are fail-open

## 4. `sc-observe`

### Finalized Public Types

- `ObservabilityConfig`
- `ObservabilityBuilder`
- `Observability`
- `ObservationEmitter<T>`

### Finalized Public Rules

- `ObservationEmitter<T>` is intentionally per-type
- registration is construction-time only
- routing order is deterministic
- `ObservabilityConfig` does not own OTLP configuration

## 5. `sc-observability-otlp`

### Finalized Public Types

- `TelemetryConfig`
- `TelemetryConfigBuilder`
- `Telemetry`
- `OtlpProtocol`
- `OtelConfig`
- `LogsConfig`
- `TracesConfig`
- `MetricsConfig`
- `ResourceAttributes`
- `SpanAssembler`
- `LogExporter`
- `TraceExporter`
- `MetricExporter`
- `SpanEmitter`
- `MetricEmitter`

### Finalized Public Rules

- `TelemetryConfig` is application-constructed
- OTLP attaches via projector registration
- invalid OTLP config fails at `Telemetry::new(...)`
- `TelemetryError::Shutdown` is returned after shutdown

## 6. API Freeze Gate

Before the first implementation sprint is declared complete:

- every item above must be either implemented or explicitly marked
  `internal-only`
- any newly introduced public type or trait must be added here first
- the API docs, requirements, and implementation must all agree on names and
  signatures
