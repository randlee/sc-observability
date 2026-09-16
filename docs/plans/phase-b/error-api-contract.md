---
status: proposed
owner: sc-observability
---

# Additive error migration contract (#92)

This contract is normative QA input for B.1a–B.1e. Every declaration is new unless
explicitly identified as retained. Published wrappers, tuple-field visibility,
trait methods/bounds/sealing, enum exhaustiveness and Serde encodings remain
unchanged. There is no scheduled removal or breaking release. The bridge's
accepted public contract is a separate pre-copy authority and is not replaced by
these core names.

## Failure model and exhaustive inventory

`sc_observability_types::typed::ClassifiedError: DiagnosticInfo` has associated `Kind: Copy + Eq`,
`fn kind(&self) -> Self::Kind`, and `fn context(&self) -> &ErrorContext`.
Implement it for all nine legacy wrappers and all nine new failures. Do not add
methods to `DiagnosticInfo` or change its seal. Downstream custom errors are
wrapped at the existing open resolver/projector/subscriber/sink boundaries;
this new trait does not promise downstream implementability beyond that seal.

For each row, implement a distinct public `XFailure` struct with private
`kind: XFailureKind` and `context: Box<ErrorContext>`. `XFailureKind` is a new
`#[non_exhaustive]` `Debug + Copy + Clone + Eq + PartialEq` enum with the listed
unit variants and `Unclassified`. New failures implement Debug, PartialEq, Display,
std::error::Error and DiagnosticInfo, retaining the ErrorContext as their source.
PartialEq compares stored kind and the existing ErrorContext equality; this does
not introduce Clone or compare backtrace identity.
They are not Clone and do not derive Serialize/Deserialize. Kinds are matched as
discriminated values; adding kinds does not force exhaustive downstream updates
because consumers must retain a fallback. Never add non_exhaustive to an old enum.

Codes below give exact existing string values and their registry constant names;
no existing constant moves or changes ownership. The neutral adapter compares
those stable values without introducing runtime-crate dependencies. Registry
parity tests run at workspace level to catch drift. This table fixes the planned built-in mapping; source drift is an explicit
review finding, never permission for an implementer to change public contracts.

| Legacy → recommended | Kind → canonical code constant |
| --- | --- |
| IdentityError → IdentityFailure | ResolutionFailed → `SC_OBSERVABILITY_TYPES_IDENTITY_RESOLUTION_FAILED` (`IDENTITY_RESOLUTION_FAILED`) |
| InitError → InitFailure | LoggerInitialization → `SC_OBSERVABILITY_LOGGER_INIT_FAILED` (`LOGGER_INIT_FAILED`); ObservationInitialization → `SC_OBSERVE_INIT_FAILED` (`OBSERVABILITY_INIT_FAILED`); InvalidTelemetryConfig → `SC_OBSERVABILITY_OTLP_INVALID_CONFIG` (`TELEMETRY_INVALID_CONFIG`); InvalidProtocol → `SC_OBSERVABILITY_OTLP_INVALID_PROTOCOL` (`TELEMETRY_INVALID_PROTOCOL`); ExporterInitialization → `SC_OBSERVABILITY_OTLP_EXPORTER_INIT_FAILED` (`TELEMETRY_EXPORTER_INIT_FAILED`); IdentityResolution → `SC_OBSERVABILITY_TYPES_IDENTITY_RESOLUTION_FAILED` (`IDENTITY_RESOLUTION_FAILED`) |
| EventError → EventFailure | InvalidEvent → `SC_OBSERVABILITY_LOGGER_INVALID_EVENT` (`LOGGER_INVALID_EVENT`); Closed → `SC_OBSERVABILITY_LOGGER_SHUTDOWN` (`LOGGER_SHUTDOWN`); QueueFull → `SC_OBSERVABILITY_LOGGER_QUEUE_FULL` (`LOGGER_QUEUE_FULL`); WriterDegraded → `SC_OBSERVABILITY_LOGGER_WRITER_DEGRADED` (`LOGGER_WRITER_DEGRADED`); ShutdownTimedOut → `SC_OBSERVABILITY_LOGGER_SHUTDOWN_TIMED_OUT` (`LOGGER_SHUTDOWN_TIMED_OUT`); SpanAssembly → `SC_OBSERVABILITY_OTLP_SPAN_ASSEMBLY_FAILED` (`TELEMETRY_SPAN_ASSEMBLY_FAILED`) |
| FlushError → FlushFailure | LoggerFlush → `SC_OBSERVABILITY_LOGGER_FLUSH_FAILED` (`LOGGER_FLUSH_FAILED`); WriterDegraded → `SC_OBSERVABILITY_LOGGER_WRITER_DEGRADED` (`LOGGER_WRITER_DEGRADED`); ObservationFlush → `SC_OBSERVE_FLUSH_FAILED` (`OBSERVABILITY_FLUSH_FAILED`); TelemetryFlush → `SC_OBSERVABILITY_OTLP_FLUSH_FAILED` (`TELEMETRY_FLUSH_FAILED`); Closed → `SC_OBSERVABILITY_OTLP_TELEMETRY_SHUTDOWN` (`TELEMETRY_SHUTDOWN`) |
| ShutdownError → ShutdownFailure | TelemetryFlush → `SC_OBSERVABILITY_OTLP_FLUSH_FAILED` (`TELEMETRY_FLUSH_FAILED`); IncompleteSpans → `SC_OBSERVABILITY_OTLP_INCOMPLETE_SPAN_DROPPED` (`TELEMETRY_INCOMPLETE_SPAN_DROPPED`); WriterDegraded → `SC_OBSERVABILITY_LOGGER_WRITER_DEGRADED` (`LOGGER_WRITER_DEGRADED`); TimedOut → `SC_OBSERVABILITY_LOGGER_SHUTDOWN_TIMED_OUT` (`LOGGER_SHUTDOWN_TIMED_OUT`) |
| ProjectionError → ProjectionFailure | TelemetryClosed → `SC_OBSERVABILITY_OTLP_TELEMETRY_SHUTDOWN` (`TELEMETRY_SHUTDOWN`); TelemetryExport → `SC_OBSERVABILITY_OTLP_EXPORT_FAILED` (`TELEMETRY_EXPORT_FAILED`); SpanAssembly → `SC_OBSERVABILITY_OTLP_SPAN_ASSEMBLY_FAILED` (`TELEMETRY_SPAN_ASSEMBLY_FAILED`); Routing → `SC_OBSERVE_OBSERVATION_ROUTING_FAILURE` (`OBSERVATION_ROUTING_FAILURE`) |
| SubscriberError → SubscriberFailure | Routing → `SC_OBSERVE_OBSERVATION_ROUTING_FAILURE` (`OBSERVATION_ROUTING_FAILURE`) |
| LogSinkError → LogSinkFailure | Write → `SC_OBSERVABILITY_LOGGER_SINK_WRITE_FAILED` (`LOGGER_SINK_WRITE_FAILED`); Maintenance → `SC_OBSERVABILITY_LOGGER_MAINTENANCE_FAILED` (`LOGGER_MAINTENANCE_FAILED`); FaultInjected → `SC_OBSERVABILITY_LOGGER_SINK_FAULT_INJECTED` (`LOGGER_SINK_FAULT_INJECTED`) |
| ExportError → ExportFailure | Export → `SC_OBSERVABILITY_OTLP_EXPORT_FAILED` (`TELEMETRY_EXPORT_FAILED`) |

`FaultInjected` is always present in the new kind enum (feature-invariant API);
its built-in producing path remains feature-gated. Custom legacy codes retain
all diagnostic information with `Unclassified`, including codes known in another
family. A diagnostic code identifies a stable category, not every detailed
failure cause; details/remediation distinguish causes within that category.

## Exact new declarations and constructors

The following are signature declarations, with implementation bodies omitted.
All fields shown are private. Each family additionally has the identical builder
methods shown after these declarations; their return `Self` is the owning family.

```rust
pub trait ClassifiedError: DiagnosticInfo {
    type Kind: Copy + Eq;
    fn kind(&self) -> Self::Kind;
    fn context(&self) -> &ErrorContext;
}
```

```rust
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdentityFailureKind {
    ResolutionFailed,
    Unclassified,
}
pub struct IdentityFailure {
    kind: IdentityFailureKind,
    context: Box<ErrorContext>,
}
impl IdentityFailure {
    pub fn resolution_failed(message: impl Into<String>, remediation: Remediation) -> Self;
    pub fn from_context(context: Box<ErrorContext>) -> Self;
    pub fn into_context(self) -> Box<ErrorContext>;
}
impl From<IdentityError> for IdentityFailure { /* move context */ }
impl From<IdentityFailure> for IdentityError { /* move context */ }
```

```rust
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InitFailureKind {
    LoggerInitialization,
    ObservationInitialization,
    InvalidTelemetryConfig,
    InvalidProtocol,
    ExporterInitialization,
    IdentityResolution,
    Unclassified,
}
pub struct InitFailure {
    kind: InitFailureKind,
    context: Box<ErrorContext>,
}
impl InitFailure {
    pub fn logger_initialization(message: impl Into<String>, remediation: Remediation) -> Self;
    pub fn observation_initialization(message: impl Into<String>, remediation: Remediation) -> Self;
    pub fn invalid_telemetry_config(message: impl Into<String>, remediation: Remediation) -> Self;
    pub fn invalid_protocol(message: impl Into<String>, remediation: Remediation) -> Self;
    pub fn exporter_initialization(message: impl Into<String>, remediation: Remediation) -> Self;
    pub fn identity_resolution(message: impl Into<String>, remediation: Remediation) -> Self;
    pub fn from_context(context: Box<ErrorContext>) -> Self;
    pub fn into_context(self) -> Box<ErrorContext>;
}
impl From<InitError> for InitFailure { /* move context */ }
impl From<InitFailure> for InitError { /* move context */ }
```

```rust
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventFailureKind {
    InvalidEvent,
    Closed,
    QueueFull,
    WriterDegraded,
    ShutdownTimedOut,
    SpanAssembly,
    Unclassified,
}
pub struct EventFailure {
    kind: EventFailureKind,
    context: Box<ErrorContext>,
}
impl EventFailure {
    pub fn invalid_event(message: impl Into<String>, remediation: Remediation) -> Self;
    pub fn closed(message: impl Into<String>, remediation: Remediation) -> Self;
    pub fn queue_full(message: impl Into<String>, remediation: Remediation) -> Self;
    pub fn writer_degraded(message: impl Into<String>, remediation: Remediation) -> Self;
    pub fn shutdown_timed_out(message: impl Into<String>, remediation: Remediation) -> Self;
    pub fn span_assembly(message: impl Into<String>, remediation: Remediation) -> Self;
    pub fn from_context(context: Box<ErrorContext>) -> Self;
    pub fn into_context(self) -> Box<ErrorContext>;
}
impl From<EventError> for EventFailure { /* move context */ }
impl From<EventFailure> for EventError { /* move context */ }
```

```rust
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlushFailureKind {
    LoggerFlush,
    WriterDegraded,
    ObservationFlush,
    TelemetryFlush,
    Closed,
    Unclassified,
}
pub struct FlushFailure {
    kind: FlushFailureKind,
    context: Box<ErrorContext>,
}
impl FlushFailure {
    pub fn logger_flush(message: impl Into<String>, remediation: Remediation) -> Self;
    pub fn writer_degraded(message: impl Into<String>, remediation: Remediation) -> Self;
    pub fn observation_flush(message: impl Into<String>, remediation: Remediation) -> Self;
    pub fn telemetry_flush(message: impl Into<String>, remediation: Remediation) -> Self;
    pub fn closed(message: impl Into<String>, remediation: Remediation) -> Self;
    pub fn from_context(context: Box<ErrorContext>) -> Self;
    pub fn into_context(self) -> Box<ErrorContext>;
}
impl From<FlushError> for FlushFailure { /* move context */ }
impl From<FlushFailure> for FlushError { /* move context */ }
```

```rust
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShutdownFailureKind {
    TelemetryFlush,
    IncompleteSpans,
    WriterDegraded,
    TimedOut,
    Unclassified,
}
pub struct ShutdownFailure {
    kind: ShutdownFailureKind,
    context: Box<ErrorContext>,
}
impl ShutdownFailure {
    pub fn telemetry_flush(message: impl Into<String>, remediation: Remediation) -> Self;
    pub fn incomplete_spans(message: impl Into<String>, remediation: Remediation) -> Self;
    pub fn writer_degraded(message: impl Into<String>, remediation: Remediation) -> Self;
    pub fn timed_out(message: impl Into<String>, remediation: Remediation) -> Self;
    pub fn from_context(context: Box<ErrorContext>) -> Self;
    pub fn into_context(self) -> Box<ErrorContext>;
}
impl From<ShutdownError> for ShutdownFailure { /* move context */ }
impl From<ShutdownFailure> for ShutdownError { /* move context */ }
```

```rust
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectionFailureKind {
    TelemetryClosed,
    TelemetryExport,
    SpanAssembly,
    Routing,
    Unclassified,
}
pub struct ProjectionFailure {
    kind: ProjectionFailureKind,
    context: Box<ErrorContext>,
}
impl ProjectionFailure {
    pub fn telemetry_closed(message: impl Into<String>, remediation: Remediation) -> Self;
    pub fn telemetry_export(message: impl Into<String>, remediation: Remediation) -> Self;
    pub fn span_assembly(message: impl Into<String>, remediation: Remediation) -> Self;
    pub fn routing(message: impl Into<String>, remediation: Remediation) -> Self;
    pub fn from_context(context: Box<ErrorContext>) -> Self;
    pub fn into_context(self) -> Box<ErrorContext>;
}
impl From<ProjectionError> for ProjectionFailure { /* move context */ }
impl From<ProjectionFailure> for ProjectionError { /* move context */ }
```

```rust
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubscriberFailureKind {
    Routing,
    Unclassified,
}
pub struct SubscriberFailure {
    kind: SubscriberFailureKind,
    context: Box<ErrorContext>,
}
impl SubscriberFailure {
    pub fn routing(message: impl Into<String>, remediation: Remediation) -> Self;
    pub fn from_context(context: Box<ErrorContext>) -> Self;
    pub fn into_context(self) -> Box<ErrorContext>;
}
impl From<SubscriberError> for SubscriberFailure { /* move context */ }
impl From<SubscriberFailure> for SubscriberError { /* move context */ }
```

```rust
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogSinkFailureKind {
    Write,
    Maintenance,
    FaultInjected,
    Unclassified,
}
pub struct LogSinkFailure {
    kind: LogSinkFailureKind,
    context: Box<ErrorContext>,
}
impl LogSinkFailure {
    pub fn write(message: impl Into<String>, remediation: Remediation) -> Self;
    pub fn maintenance(message: impl Into<String>, remediation: Remediation) -> Self;
    pub fn fault_injected(message: impl Into<String>, remediation: Remediation) -> Self;
    pub fn from_context(context: Box<ErrorContext>) -> Self;
    pub fn into_context(self) -> Box<ErrorContext>;
}
impl From<LogSinkError> for LogSinkFailure { /* move context */ }
impl From<LogSinkFailure> for LogSinkError { /* move context */ }
```

```rust
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportFailureKind {
    Export,
    Unclassified,
}
pub struct ExportFailure {
    kind: ExportFailureKind,
    context: Box<ErrorContext>,
}
impl ExportFailure {
    pub fn export(message: impl Into<String>, remediation: Remediation) -> Self;
    pub fn from_context(context: Box<ErrorContext>) -> Self;
    pub fn into_context(self) -> Box<ErrorContext>;
}
impl From<ExportError> for ExportFailure { /* move context */ }
impl From<ExportFailure> for ExportError { /* move context */ }
```

```rust
// The following methods are implemented on each of the nine new failure types.
pub fn cause(self, cause: impl Into<String>) -> Self;
pub fn docs(self, docs: impl Into<String>) -> Self;
pub fn detail(self, key: impl Into<String>, value: serde_json::Value) -> Self;
pub fn source(self, source: Box<dyn std::error::Error + Send + Sync + 'static>) -> Self;
```

Each named constructor sets its table's canonical code and stores the kind
without code parsing. Compatibility `from_context` alone classifies codes.
Builder methods never alter the code/kind. Legacy `ClassifiedError::kind` performs
the same total lookup; new failures return their stored kind. Unknown values
never become success, lose sources or trigger logging. Conversions move the
original Box, rather than reconstructing it, so timestamps, source Arc and
captured backtrace survive. Native failure values return `context` by borrow;
`into_context` consumes self and returns that same Box.

## Neutral extension contracts (B.1a)

ClassifiedError, all five new extension traits and their adapter functions live only in the new
`sc_observability_types::typed` module; no root or runtime-root re-exports are
added for them. The logger-specific TypedLogSink/adapters live only in
`sc_observability::typed`. Existing published root exports stay unchanged.
Compile unchanged glob-import consumers, including first-party concrete sinks
and projectors, to prove no new trait-method ambiguity. New consumers explicitly
import the typed module; mixed migration fixtures may qualify a trait call.

Add these open object-safe traits with precisely the existing Send + Sync and
Observable bounds and the improved return errors:

```rust
pub trait TypedProcessIdentityResolver: Send + Sync {
    fn resolve(&self) -> Result<ProcessIdentity, IdentityFailure>;
}
pub trait TypedObservationSubscriber<T: Observable>: Send + Sync {
    fn observe(&self, observation: &Observation<T>) -> Result<(), SubscriberFailure>;
}
pub trait TypedLogProjector<T: Observable>: Send + Sync {
    fn project_logs(&self, observation: &Observation<T>) -> Result<Vec<LogEvent>, ProjectionFailure>;
}
pub trait TypedSpanProjector<T: Observable>: Send + Sync {
    fn project_spans(&self, observation: &Observation<T>) -> Result<Vec<SpanSignal>, ProjectionFailure>;
}
pub trait TypedMetricProjector<T: Observable>: Send + Sync {
    fn project_metrics(&self, observation: &Observation<T>) -> Result<Vec<MetricRecord>, ProjectionFailure>;
}
pub fn legacy_identity(value: Arc<dyn TypedProcessIdentityResolver>) -> Arc<dyn ProcessIdentityResolver>;
pub fn typed_identity(value: Arc<dyn ProcessIdentityResolver>) -> Arc<dyn TypedProcessIdentityResolver>;
pub fn legacy_subscriber<T: Observable>(value: Arc<dyn TypedObservationSubscriber<T>>) -> Arc<dyn ObservationSubscriber<T>>;
pub fn typed_subscriber<T: Observable>(value: Arc<dyn ObservationSubscriber<T>>) -> Arc<dyn TypedObservationSubscriber<T>>;
pub fn legacy_log_projector<T: Observable>(value: Arc<dyn TypedLogProjector<T>>) -> Arc<dyn LogProjector<T>>;
pub fn typed_log_projector<T: Observable>(value: Arc<dyn LogProjector<T>>) -> Arc<dyn TypedLogProjector<T>>;
pub fn legacy_span_projector<T: Observable>(value: Arc<dyn TypedSpanProjector<T>>) -> Arc<dyn SpanProjector<T>>;
pub fn typed_span_projector<T: Observable>(value: Arc<dyn SpanProjector<T>>) -> Arc<dyn TypedSpanProjector<T>>;
pub fn legacy_metric_projector<T: Observable>(value: Arc<dyn TypedMetricProjector<T>>) -> Arc<dyn MetricProjector<T>>;
pub fn typed_metric_projector<T: Observable>(value: Arc<dyn MetricProjector<T>>) -> Arc<dyn TypedMetricProjector<T>>;
```

Adapters are private wrapper structs returned through these functions, not
blanket impls that would overlap downstream implementations. They invoke exactly
once and only map the error value with From. Existing policy variants and
registrations accept the legacy adapter, avoiding new required fields/variants.
The old traits remain supported indefinitely for old consumers.

## Compatibility and warning policy (B.1e)

Deprecate the nine old wrapper types and only the old entry points whose complete
recommended counterpart appears in B.1b–B.1d. Each note names its exact replacement
and the migration guide. Do not deprecate DiagnosticInfo, ErrorContext,
Diagnostic, ErrorCode, existing kinded errors, old open traits, registrations,
config fields, legacy infallible LoggerBuilder::build, or constructors that
have no equivalent replacement. build_typed adds recoverable startup failure
reporting without altering the old infallible build contract. Old traits
remain documented as supported interoperability boundaries; typed traits and
explicit adapters are recommended for new implementations.

B.1e selects the next available minor workspace version greater than the runtime
prerequisite release and substitutes it for V in #[deprecated(since = "V", ...)].
B.2 publishes that exact version; no placeholder may remain in implementation.
Warnings are the only intended compatibility effect. Downstream -D warnings or
-D deprecated can fail by caller policy; document narrow temporary lint allowances,
not workspace-wide suppression. Legacy examples/adapter internals and the imported
bridge may retain narrowly explained deprecated uses where required by its
accepted signatures. This is implementation-only accommodation; B.1 import
provenance remains historical and unchanged.

Serialized legacy wrappers and existing enums retain byte-compatible fixtures.
New native failures have no wire representation here. B.3 explicitly converts
kind + diagnostic data into its versioned DTO result; native sources/backtraces
never cross language boundaries. No new operational panic/throw/raise path is
introduced; old and new runtime methods execute a common implementation with
unchanged lifecycle and admission semantics.


## Source ownership and retained boundaries

| Source boundary | Implementation owner and disposition |
| --- | --- |
| types `errors.rs`, `diagnostic.rs` | B.1a: new failure/classification model; legacy wrapper/DiagnosticInfo definitions retained |
| types `process.rs`, `projection.rs` | B.1a: new typed traits and explicit adapters; old policies/registrations/traits unchanged |
| logger `builder.rs` construction | B.1b: new_typed, typed initialization production; old new adapts |
| logger `runtime.rs` construction/prepare_event/validate_event/admission/flush | B.1b: listed typed methods; legacy methods adapt; emit retains its conditional-flush policy |
| logger `maintenance.rs` flush control and `sinks.rs` write/flush/maintenance/fault paths | B.1b: typed internal failures and TypedLogSink; existing health/source behavior retained |
| logger `lib.rs` LogError/TryLogError and compatibility conversion | B.1b: retain old enums; add conversions to/from neutral new enums; health/query/follow/shutdown remain unchanged |
| sc-observe `lib.rs` config derivation, constructor/builder, flush/shutdown, route callbacks | B.1c: exact typed methods and neutral adapters; emit/ObservationError/registration API unchanged |
| otlp `config.rs` endpoint/header/build/validation | B.1d: exact typed constructors and typed internal validation |
| otlp `assembly.rs` push | B.1d: push_typed and typed internal span failures; old push adapts |
| otlp `lib.rs` construction/flush/shutdown/private exporters | B.1d: exact typed methods, private ExportFailure production; emit/TelemetryError unchanged |
| otlp `projectors.rs` callback adapters | B.1d: typed projector implementation plus retained legacy impls; existing fluent constructors unchanged |
| copied bridge and B.P1 runtime-level public operations | No signature, variant, lifecycle or diagnostic-contract migration; B.1e narrow internal allowances only |
| ValueValidationError, QueryError, ObservationError, TelemetryError, LogError, TryLogError | Existing published contracts retained; no wrapper conversion/deprecation except new separate logger admission failures |

Existing fail-open behavior stays explicit: telemetry `flush()` currently records
export failures in health while returning success; `flush_typed()` preserves
that behavior. Its first shutdown surfaces the final export failure, and repeated
shutdown remains successful. Observation flush after stop remains successful;
its shutdown does not acquire new failure cases. Conversion preserves whatever
source/context the original operation retained; it does not claim to recover a
native source previously reduced to a health summary. `DiagnosticSummary` has no
remediation; never infer missing remediation from its message. New binding
operational failures use the separate B.P1 `OperationDiagnostic` contract.
