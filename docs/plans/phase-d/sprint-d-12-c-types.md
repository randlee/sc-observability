# d-12: Types and OTLP contract

## Plan metadata

- Wave: 1
- Branch: `sprint/d-12-c-types`
- PR target: `integrate/phase-d`
- Blocked by: `obs-phase-d-plan-qa`
- Owned paths:
  - `crates/sc-observability-types/src/errors.rs`
  - `crates/sc-observability-types/src/events.rs`
  - `crates/sc-observability-types/src/metric.rs`
  - `crates/sc-observability-types/src/span.rs`
  - `crates/sc-observability-types/src/validation.rs`
  - `crates/sc-observability-types/tests/neutral_contracts.rs`
  - `crates/sc-observability-otlp/src/lib.rs`
  - `crates/sc-observability-otlp/Cargo.toml`
  - `crates/sc-observability-types/src/diagnostic.rs`
  - `crates/sc-observability-types/src/lib.rs`
  - `crates/sc-observability-types/src/process.rs`
  - `crates/sc-observability-types/src/projection.rs`
  - `docs/architecture.md`
  - `docs/requirements.md`
  - `docs/api-design.md`

## Deliverables

1. Add the nine `#[non_exhaustive]` typed error enums carrying `Box<ErrorContext>` from D.4, with the public 2.0 error contract.
2. Add neutral signal model types, serde contracts, and validation errors from D.5.
3. Add `PositiveDuration`, `LifecycleBounds`, `RetryPolicy`, `BoundedPercent`, transport bounds, lifecycle config fields, the exporter trait, `ExporterSet`, factory, and fake exporter fixture from D.6.
4. Record ADR-017, supersede only the conflicting ADR-012 portion, and align requirements/architecture/API design.
5. Hoist OTLP `lib.rs` module declarations and Cargo feature declarations required by all implementation modules.

```rust
pub trait Exporter: Send + Sync { fn export(&self, batch: &[Signal]) -> Result<(), ExportError>; }
pub struct ExporterSet { /* configured exporters */ }
```

## This Sprint Does Not Close

No production exporter adapter, signal projection, wrapper migration, or public API composition is implemented here.


## Design

## D.4 canonical error-enum inventory

D12 owns the nine public, `#[non_exhaustive]` error enums. Each named variant carries `Box<ErrorContext>`; callers preserve the diagnostic source rather than constructing a tuple wrapper. These are the only migration targets used by D14–D17.

```rust
#[non_exhaustive] pub enum IdentityError { Process { context: Box<ErrorContext> } }
#[non_exhaustive] pub enum InitError { Configuration { context: Box<ErrorContext> }, Runtime { context: Box<ErrorContext> } }
#[non_exhaustive] pub enum EventError { Validation { context: Box<ErrorContext> }, Routing { context: Box<ErrorContext> } }
#[non_exhaustive] pub enum FlushError { Drain { context: Box<ErrorContext> } }
#[non_exhaustive] pub enum ShutdownError { Timeout { context: Box<ErrorContext> }, Drain { context: Box<ErrorContext> } }
#[non_exhaustive] pub enum ProjectionError { Projection { context: Box<ErrorContext> } }
#[non_exhaustive] pub enum SubscriberError { Subscriber { context: Box<ErrorContext> } }
#[non_exhaustive] pub enum LogSinkError { Write { context: Box<ErrorContext> }, Flush { context: Box<ErrorContext> } }
#[non_exhaustive] pub enum ExportError { Transport { context: Box<ErrorContext> }, Lifecycle { context: Box<ErrorContext> } }
```

`sc-observability-types/src/diagnostic.rs`, `lib.rs`, `process.rs`, and `projection.rs` expose the supporting context, re-exports, process, and projection signatures. `typed.rs` is the D13 logging-contract owner because it defines `LogFailure` and `TryLogFailure`.

## Public contract

The implementation may refine names during API review, but it must preserve
this discriminated shape and information content:

```rust
#[non_exhaustive]
pub enum SpanKind { Internal, Server, Client, Producer, Consumer }

#[non_exhaustive]
pub struct TraceFlags(u8); // exposes sampled() and preserves known W3C bits

#[non_exhaustive]
pub struct SpanLink {
    pub trace_id: TraceId,
    pub span_id: SpanId,
    pub flags: TraceFlags,
    pub attributes: Attributes,
}

#[non_exhaustive]
pub enum AggregationTemporality { Delta, Cumulative }

#[non_exhaustive]
pub enum MetricValue {
    Gauge(FiniteF64),
    Sum {
        value: FiniteF64,
        monotonic: bool,
        temporality: AggregationTemporality,
        start_time: Timestamp,
    },
    Histogram {
        point: HistogramPoint,
        temporality: AggregationTemporality,
        start_time: Timestamp,
    },
}

pub struct HistogramPoint {
    explicit_bounds: Vec<f64>,
    bucket_counts: Vec<u64>,
    count: u64,
    sum: FiniteF64,
}

impl HistogramPoint {
    pub fn try_new(/* fields above */) -> Result<Self, MetricModelError>;
    // read-only accessors
}
```

`Attributes` and `FiniteF64` are neutral validated types owned by
`sc-observability-types`; this model does not introduce a `serde_json` runtime
dependency in lower crates. `SpanRecord` carries `SpanKind` and links;
`TraceContext` carries trace flags.
`MetricRecord` carries `MetricValue` rather than the current `MetricKind` plus
single `f64` combination. Exact serde names and constructors are frozen in the
2.0 API approval before implementation completion.
`HistogramPoint` deserializes through a validated `TryFrom` representation so
serde cannot construct an invalid value. A link contains its own ids/flags and
never embeds `TraceContext`, eliminating two sources of truth.
`MetricValue`, `TraceFlags`, and `SpanLink` are `#[non_exhaustive]` public
types; consumers must use their constructors/accessors or wildcard matching
rather than depend on exhaustive future shape.




## Backend and trait contract

```rust
#[non_exhaustive]
pub enum ExporterBackend {
    OpenTelemetrySdk,
    LegacyHttpJson, // reserved; D.8 makes this backend operational
}

#[non_exhaustive]
pub struct OtelConfig {
    pub backend: ExporterBackend,
    pub protocol: OtlpProtocol,
    // endpoint/auth/TLS and timeout fields remain explicit;
    pub legacy_retry: Option<LegacyRetryPolicy>,
}

#[non_exhaustive]
pub struct LegacyRetryPolicy { /* legacy-only retry wire fields below */ }

impl OtelConfig {
    pub fn new(backend: ExporterBackend, protocol: OtlpProtocol) -> Self;
}

type LifecycleFuture = Pin<
    Box<dyn Future<Output = Result<(), ExportError>> + Send + 'static>
>;

pub(crate) trait ExporterLifecycle: Send + Sync {
    fn blocking_preflight(&self) -> Result<(), ExportError>;
    fn flush_async(&self) -> LifecycleFuture;
    fn shutdown_async(&self) -> LifecycleFuture;
    fn flush_blocking(&self) -> Result<(), ExportError>;
    fn shutdown_blocking(&self) -> Result<(), ExportError>;
}

pub(crate) trait LogExporter: Send + Sync {
    fn export_logs(&self, batch: &[LogEvent]) -> Result<(), ExportError>;
}

pub(crate) trait TraceExporter: Send + Sync {
    fn export_spans(&self, batch: &[CompleteSpan]) -> Result<(), ExportError>;
}

pub(crate) trait MetricExporter: Send + Sync {
    fn export_metrics(&self, batch: &[MetricRecord]) -> Result<(), ExportError>;
}

pub(crate) struct ExporterSet {
    logs: Arc<dyn LogExporter>,
    traces: Arc<dyn TraceExporter>,
    metrics: Arc<dyn MetricExporter>,
    lifecycle: Arc<dyn ExporterLifecycle>,
}
```

Both backends construct the same `ExporterSet`; `Telemetry` stores only these
trait objects. `ExporterBackend` is consumed by construction/injection and is
never branched on by emit, flush, or shutdown. Selecting `LegacyHttpJson`
before D.8 returns a stable typed unsupported-backend error. An enabled
configuration never silently installs a no-op exporter.

The factory validates this closed matrix before allocating providers/workers:

| Backend | Valid protocol | Required feature/runtime | Invalid result |
| --- | --- | --- | --- |
| disabled (transport disabled) | none | none | the sole no-network disabled implementation |
| `OpenTelemetrySdk` | SDK-supported gRPC or HTTP/protobuf | `otlp-sdk`; entered caller Tokio runtime | stable unsupported-protocol/runtime error |
| `LegacyHttpJson` | `HttpJson` only | `legacy-http-json`; plain-thread construction | reserved typed error until D.8 |

Delete public/production `Noop*Exporter` fallbacks; disabled construction is an
explicit private disabled set and an enabled selection can never reach it.
Every existing `OtelConfig` field receives one disposition: endpoint,
headers/auth, CA/TLS and `timeout_ms` map to the SDK/legacy builders;
`debug_local_export` is a separate diagnostic mirror outside exporter
selection; `insecure_skip_verify` is either implemented by the backend with an
explicit security warning or rejected at construction—never ignored.
`timeout_ms` covers the entire legacy HTTP request, including connect, TLS,
request write, response headers, and response read. Endpoint and header/auth
values are validated before provider/worker construction; malformed endpoints,
invalid header syntax, and forbidden credential placement return named
construction failures without retaining secret values.

### D.6-owned validated transport contract

D.6 is the sole owner of every transport-bound field, default, validation,
and configuration error. The 2.0 wire surface uses direct shared transport
fields plus a grouped `legacy_retry` object:

| Field | Applicability | Default when absent |
| --- | --- | --- |
| `timeout_ms` | both backends; maps to request/export timeout | `3_000` |
| `lifecycle_flush_timeout_ms` | both backends | `30_000` |
| `lifecycle_shutdown_timeout_ms` | both backends | `30_000` |
| `queue_capacity` | both backends; bounded admission queue | `1_024` |
| `legacy_retry.max_retries` | legacy only, optional on wire | `3` |
| `legacy_retry.initial_backoff_ms` | legacy only, optional on wire | `250` |
| `legacy_retry.max_backoff_ms` | legacy only, optional on wire | `5_000` |
| `legacy_retry.retry_sequence_timeout_ms` | legacy only, optional on wire | `30_000` |
| `legacy_retry.retry_after_cap_ms` | legacy only, optional on wire | `5_000` |
| `legacy_retry.retry_jitter_percent` | legacy only, optional on wire | `20` |

`queue_capacity` is validated as `1..=65_536`; records are split before
admission at `512` records or `1 MiB`. A 413 is terminal for that split batch,
which is counted once as failed/dropped rather than retried as a larger batch.
`shutdown_async_typed` has one drain budget: it starts at shutdown entry and
covers cancellation, the in-flight request, barrier, and worker join. On
expiry it returns `LifecycleTimeout` with remaining admitted work accounted.

```rust
#[non_exhaustive]
pub struct TelemetryHealth {
    pub queue_depth: usize,
    pub queue_capacity: usize,
    pub worker_state: WorkerState,
    pub last_terminal_failure: Option<Diagnostic>,
    pub last_success: Option<Timestamp>,
}
```

For `OpenTelemetrySdk`, the three shared timeout fields map to SDK lifecycle /
export construction. Any explicit legacy-only field—including the pre-existing
`max_retries`, `initial_backoff_ms`, and `max_backoff_ms`—returns
`ConfigFieldNotApplicable`. Nothing is ignored. This 2.0 optional-field change
and its migration from the former unconditional retry defaults are documented.

Defaults are resolved **before** validation. Each resolved value retains
`ValueOrigin::{Default, Explicit}` so an error identifies both the offending
field and whether a conflicting peer was defaulted. Partial overrides are
therefore deterministic and reviewable.

All raw serialized millisecond/percent fields are converted exactly once:

```rust
#[non_exhaustive]
pub enum OtlpConfigField {
    Endpoint,
    Header,
    Timeout,
    LifecycleFlushTimeout,
    LifecycleShutdownTimeout,
    QueueCapacity,
    MaxRetries,
    InitialBackoff,
    MaxBackoff,
    RetrySequenceTimeout,
    RetryAfterCap,
    RetryJitterPercent,
}

#[non_exhaustive]
pub enum ValueOrigin { Default, Explicit }

#[non_exhaustive]
pub struct ResolvedField<T> {
    pub field: OtlpConfigField,
    pub value: T,
    pub origin: ValueOrigin,
}

#[non_exhaustive]
pub enum OtlpConfigTarget { Disabled, Backend(ExporterBackend) }

pub(crate) struct PositiveDuration(Duration);

impl PositiveDuration {
    fn try_from_millis(field: OtlpConfigField, value: u64)
        -> Result<Self, ConfigFailure>;
}

pub(crate) struct LifecycleBounds {
    flush: PositiveDuration,
    shutdown: PositiveDuration,
}

pub(crate) struct BoundedPercent(u8); // checked 0..=100

pub(crate) struct RetryPolicy {
    max_retries: u32,
    initial_backoff: PositiveDuration,
    max_backoff: PositiveDuration,
    sequence_timeout: PositiveDuration,
    retry_after_cap: PositiveDuration,
    jitter: BoundedPercent,
}

pub(crate) struct ValidatedTransportBounds {
    request_timeout: PositiveDuration,
    lifecycle: LifecycleBounds,
    backend: BackendTransportBounds,
}

pub(crate) enum BackendTransportBounds {
    Disabled,
    Sdk,
    Legacy(RetryPolicy),
}

impl ValidatedTransportBounds {
    fn try_from_config(config: &OtelConfig) -> Result<Self, ConfigFailure>;
}
```

`TelemetryHealth`, `OtlpConfigField`, `ValueOrigin`, `ResolvedField`, and
`OtlpConfigTarget` are `#[non_exhaustive]` public types so their 2.0 contracts
can add fields or variants without a further breaking release.

The constructor derives `Disabled` from `config.enabled == false`; otherwise
it derives the backend only from `config.backend`.
`LifecycleBounds` holds checked positive flush/shutdown durations;
`RetryPolicy` holds `max_retries`, checked initial/max/sequence/Retry-After
durations, and `BoundedPercent(0..=100)`. `BackendTransportBounds` makes legacy
retry state unrepresentable for SDK. Validation, using checked arithmetic, is:

- every millisecond duration is positive and convertible to `Duration`;
- `lifecycle_shutdown_timeout_ms >= timeout_ms`;
- `lifecycle_flush_timeout_ms >= timeout_ms`;
- `queue_capacity` is in `1..=65_536`, otherwise `InvalidQueueCapacity`;
- for legacy, `max_backoff_ms >= initial_backoff_ms`;
- for legacy, `retry_sequence_timeout_ms >= timeout_ms`;
- for legacy, `0 < retry_after_cap_ms <= retry_sequence_timeout_ms`;
- for legacy, `retry_jitter_percent <= 100`;
- reject every explicit field inapplicable to disabled transport or the
  selected backend with `ConfigFieldNotApplicable`;
- reject a requested insecure verification override when the selected backend
  does not explicitly support it with `InsecureTransportRejected`.
- validate endpoint URL syntax, header/auth syntax, and credential placement;
  otherwise return `InvalidEndpoint` or `InvalidHeader` with a redacted,
  field-only payload.

Checks execute in exactly this listed order and return the first failure; they
are not aggregated. Within the first bullet, fields are checked in the wire
table's top-to-bottom order. This makes every multi-violation diagnostic
deterministic.

Both factories receive only `ValidatedTransportBounds` and cannot inspect or
reparse raw fields. Deadlines use a monotonic injectable clock and start when
the public operation is admitted.

Construction order is fixed: resolve defaults and create `ResolvedField`
values; run the ordered validation list above; build
`ValidatedTransportBounds`; then check feature/backend/protocol availability.
Thus a malformed legacy config fails deterministically before D.6's reserved
`UnsupportedBackend`. Disabled transport still validates explicitly supplied
shared fields, rejects every explicit legacy-only retry field with
`ConfigFieldNotApplicable { target: OtlpConfigTarget::Disabled, .. }`, yields
`BackendTransportBounds::Disabled`, and never constructs a network
provider/worker. SDK inapplicability instead records
`OtlpConfigTarget::Backend(ExporterBackend::OpenTelemetrySdk)`.

Configuration variants carry reviewable payloads:

```rust
ZeroDuration { field: OtlpConfigField, origin: ValueOrigin }
DurationOverflow { field: OtlpConfigField, raw: u64, origin: ValueOrigin }
InvalidBoundOrdering {
    lower: ResolvedField<u64>,
    upper: ResolvedField<u64>,
}
InvalidJitterPercent { field: OtlpConfigField, raw: u8, origin: ValueOrigin }
ConfigFieldNotApplicable { field: OtlpConfigField, target: OtlpConfigTarget }
InsecureTransportRejected { backend: ExporterBackend }
InvalidEndpoint { field: OtlpConfigField }
InvalidHeader { field: OtlpConfigField }
TransportConstructionFailed { backend: ExporterBackend, source: Diagnostic }
```

### D.6 stable failure inventory

This is D.6's complete transport/lifecycle/configuration stable-error
inventory. D.6 exclusively owns these variants, codes, owning types, mappings,
and documentation; D.5 separately owns its `MetricModelError` rows, and later
sprints consume both tables without adding or restating rows. Configuration
rows are construction-only `ConfigFailure` variants. Every runtime/lifecycle
variant except `Shutdown` is owned by D.4's canonical `ExportError`; `Shutdown`
is owned by `TelemetryError`. Construction never uses an additional wrapper variant: it
returns the named `ConfigFailure` rows below. A legacy async-context failure
during construction is the redacted `Diagnostic` source of
`TransportConstructionFailed`; the same condition during synchronous lifecycle
is `BlockingBackendInAsyncContext` directly. Emit/lifecycle façades convert
runtime failures to `TelemetryError`, D.4's canonical `FlushError`, or D.4's
canonical `ShutdownError` without changing their stable code or typed source.
In particular, `QueueFull` is created as an
`ExportError`; emit converts it through `From<ExportError> for
TelemetryError`, while lifecycle converts the same source through the
corresponding typed lifecycle failure.

| Variant | Stable code | Owning error type | Cause | Recovery | Redaction | Retryability |
| --- | --- | --- | --- | --- | --- | --- |
| `ZeroDuration` | `OTLP_CONFIG_ZERO_DURATION` | `ConfigFailure` | required duration is zero | provide a positive value | field/value only | after config correction |
| `DurationOverflow` | `OTLP_CONFIG_DURATION_OVERFLOW` | `ConfigFailure` | milliseconds cannot convert safely | reduce the field | field/value only | after config correction |
| `InvalidBoundOrdering` | `OTLP_CONFIG_BOUND_ORDER` | `ConfigFailure` | resolved ordering rule fails | correct the named explicit/defaulted fields | field/value/origin only | after config correction |
| `InvalidJitterPercent` | `OTLP_CONFIG_JITTER_PERCENT` | `ConfigFailure` | jitter exceeds 100 | use `0..=100` | field/value only | after config correction |
| `InvalidQueueCapacity` | `OTLP_CONFIG_QUEUE_CAPACITY` | `ConfigFailure` | queue capacity is outside `1..=65_536` | choose a bounded capacity | field/value only | after config correction |
| `ConfigFieldNotApplicable` | `OTLP_CONFIG_FIELD_NOT_APPLICABLE` | `ConfigFailure` | field is inapplicable to disabled transport or the selected backend | omit it, enable transport, or select its applicable backend | field/closed target only | after config correction |
| `InsecureTransportRejected` | `OTLP_CONFIG_INSECURE_TRANSPORT_REJECTED` | `ConfigFailure` | selected backend does not implement the requested insecure verification override | disable the override or choose an explicitly supporting backend | backend only | after config correction |
| `InvalidEndpoint` | `OTLP_CONFIG_INVALID_ENDPOINT` | `ConfigFailure` | endpoint URL syntax is invalid | provide a valid endpoint URL | field only | after config correction |
| `InvalidHeader` | `OTLP_CONFIG_INVALID_HEADER` | `ConfigFailure` | header/auth syntax or credential placement is invalid | correct the header/auth configuration | field only | after config correction |
| `TransportConstructionFailed` | `OTLP_TRANSPORT_CONSTRUCTION_FAILED` | `ConfigFailure` | CA/auth/client/provider/legacy-worker initialization failed | correct the bounded typed source and reconstruct | bounded typed source; never path contents, credentials, header values, or response bodies | after config/environment correction |
| `UnsupportedBackend` | `OTLP_UNSUPPORTED_BACKEND` | `ConfigFailure` | feature/backend unavailable | enable/select a supported backend | enum values only | after build/config correction |
| `UnsupportedProtocol` | `OTLP_UNSUPPORTED_PROTOCOL` | `ConfigFailure` | protocol invalid for backend | select a matrix-supported protocol | enum values only | after config correction |
| `TokioRuntimeRequired` | `OTLP_TOKIO_RUNTIME_REQUIRED` | `ConfigFailure` | SDK construction lacks an entered Tokio runtime | construct inside the host runtime | no dynamic data | after entering a runtime |
| `BlockingBackendInAsyncContext` | `OTLP_BLOCKING_BACKEND_IN_ASYNC_CONTEXT` | `ExportError` | legacy synchronous lifecycle entered Tokio; construction preserves this condition as the redacted source of `TransportConstructionFailed` | use a plain thread or async lifecycle | no dynamic data | in a supported context |
| `AsyncLifecycleRequired` | `OTLP_ASYNC_LIFECYCLE_REQUIRED` | `ExportError` | SDK synchronous completion requested | await the typed async operation | no dynamic data | through async lifecycle |
| `RuntimeTerminated` | `OTLP_RUNTIME_TERMINATED` | `ExportError` | host runtime ended before completion | keep the runtime alive through awaited shutdown | bounded state/counts | with a live replacement runtime/instance |
| `LifecycleTimeout` | `OTLP_LIFECYCLE_TIMEOUT` | `ExportError` | monotonic lifecycle deadline elapsed | inspect terminal health and transport/provider | duration/state only | operation-specific |
| `QueueFull` | `OTLP_QUEUE_FULL` | `ExportError` | bounded admission queue saturated | preserve fail-open behavior and inspect health | capacity/depth only | yes, later admission |
| `WorkerTerminated` | `OTLP_WORKER_TERMINATED` | `ExportError` | SDK dispatcher or legacy worker terminated unexpectedly | correct the terminal cause and construct a new instance | bounded typed source; no credentials | only with a new instance |
| `ShutdownCancelledRetry` | `OTLP_SHUTDOWN_CANCELLED_RETRY` | `ExportError` | shutdown cancelled a retryable pre-barrier legacy sequence | inspect terminal health; resend only if duplicates are acceptable | attempt/count only | caller decision; duplicates possible |
| `RetryDeadlineExhausted` | `OTLP_RETRY_DEADLINE_EXHAUSTED` | `ExportError` | no legacy sequence budget remains | increase the validated sequence bound or restore collector health | budget/attempt only | new operation after recovery |
| `NonRetryableHttpStatus` | `OTLP_HTTP_STATUS_TERMINAL` | `ExportError` | collector returned a non-retryable HTTP status | correct request/auth/config before retrying | status/category only; no body/headers | after cause correction |
| `RetryAttemptsExhausted` | `OTLP_RETRY_ATTEMPTS_EXHAUSTED` | `ExportError` | legacy maximum attempts ended before success | restore collector health or adjust the validated policy | attempt/count only | new operation after recovery |
| `TerminalExportFailure` | `OTLP_EXPORT_TERMINAL` | `ExportError` | SDK or legacy provider returned a terminal export failure | inspect the preserved source and collector state | bounded typed source; no credentials | source-dependent |
| `Shutdown` | `OTLP_TELEMETRY_SHUTDOWN` | `TelemetryError` | emit was attempted after shutdown began | construct a new telemetry instance | no dynamic data | only on a new instance |

## OTLP crate registration contract

`crates/sc-observability-otlp/src/lib.rs` declares the implementation modules once for all consumers and `Cargo.toml` declares the features that select them:

```rust
mod config;
mod constants;
mod assembly;
mod error_codes;
mod projectors;
#[cfg(feature = "otlp-sdk")] mod sdk;
#[cfg(feature = "legacy-http-json")] mod legacy_http_json;
```

```toml
[features]
otlp-sdk = []
legacy-http-json = []
```

## Implementation targets

- `docs/architecture.md`, `docs/requirements.md`, and `docs/api-design.md`: record ADR-017, the scoped ADR-012 supersession, and the same accepted decision (deliverable 4).

## Acceptance criteria

- `cargo test -p sc-observability-types --test neutral_contracts` passes and names all nine `#[non_exhaustive]` enums with `Box<ErrorContext>` (deliverable 1).
- `cargo test -p sc-observability-types` passes the neutral signal serde/validation cases (deliverable 2).
- `cargo check -p sc-observability-otlp --all-features` compiles the declared modules and feature gates (deliverables 3–4).
- `rg "ADR-017|ADR-012" docs/architecture.md docs/requirements.md docs/api-design.md` shows the recorded decision (deliverable 4).
