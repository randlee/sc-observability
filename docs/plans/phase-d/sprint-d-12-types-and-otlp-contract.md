# d-12: Types and OTLP contract

Generated projection of `obs-d-12`; the bead is authoritative.

## Plan metadata

- Wave: 1
- Layer: 1
- Assignee / model: lobs / luna
- Relation: `root`
- Closure: `contract`
- Target boundary: sc-observability-types and OTLP contract
- Branch: `sprint/d-12-types-and-otlp-contract`
- Worktree: `/Users/randlee/github/sc-observability-worktrees/sprint/d-12-types-and-otlp-contract`
- PR target (merge order only): `integrate/phase-d`
- Blocked by: `obs-phase-d-plan-qa`
- Requirements: LAY-001, LAY-002, LAY-003, LAY-004, LAY-005, LAY-006, LAY-007, LOG-014, LOG-016, LOG-018, LOG-019, LOG-047, LOG-048, NFR-003, NFR-004, NFR-005, NFR-008, NFR-009, NFR-010, NFR-011, NFR-012, OBS-003, OBS-007, OBS-012, OBS-015, OBS-018, OBS-021, OTLP-004, OTLP-005, OTLP-006, OTLP-007, OTLP-008, OTLP-009, OTLP-010, OTLP-011, OTLP-013, OTLP-014, OTLP-016, OTLP-018, OTLP-019, OTLP-020, OTLP-021, OTLP-022, OTLP-023, PHB-002, PHB-003, PHB-004, PHB-005, PHB-010, PHB-012, PHB-013, PHD-001, PHD-002, PHD-003, PHD-004, SRC-001, SRC-002, SRC-003, SRC-004, SRC-005, SRC-006, TYP-001, TYP-002, TYP-003, TYP-004, TYP-005, TYP-006, TYP-007, TYP-008, TYP-009, TYP-010, TYP-011, TYP-012, TYP-013, TYP-014, TYP-015, TYP-016, TYP-017, TYP-018, TYP-019, TYP-020, TYP-021, TYP-022, TYP-023, TYP-024, TYP-030, TYP-031, TYP-039
- ADRs: ADR-002, ADR-003, ADR-004, ADR-005, ADR-006, ADR-009, ADR-011, ADR-014, ADR-015, ADR-017, ADR-018, ADR-019
- Owned paths (metadata projection):
  - `Cargo.lock`
  - `Cargo.toml`
  - `bindings/python/sc-observability-py/Cargo.toml`
  - `bindings/schema-generator/Cargo.lock`
  - `bindings/schema-generator/Cargo.toml`
  - `bindings/tauri/Cargo.lock`
  - `bindings/tauri/Cargo.toml`
  - `boundaries/**`
  - `crates/sc-observability-binding-runtime/Cargo.toml`
  - `crates/sc-observability-dto/Cargo.toml`
  - `crates/sc-observability-dto/src/error_codes.rs`
  - `crates/sc-observability-log-consumer-check/Cargo.toml`
  - `crates/sc-observability-log-macros/Cargo.toml`
  - `crates/sc-observability-log/Cargo.toml`
  - `crates/sc-observability-log/src/error_codes.rs`
  - `crates/sc-observability-otlp/Cargo.toml`
  - `crates/sc-observability-otlp/src/assembly.rs`
  - `crates/sc-observability-otlp/src/config.rs`
  - `crates/sc-observability-otlp/src/constants.rs`
  - `crates/sc-observability-otlp/src/contract_tests.rs`
  - `crates/sc-observability-otlp/src/contracts.rs`
  - `crates/sc-observability-otlp/src/error_codes.rs`
  - `crates/sc-observability-otlp/src/legacy_http_json/implementation.rs`
  - `crates/sc-observability-otlp/src/legacy_http_json/mod.rs`
  - `crates/sc-observability-otlp/src/legacy_http_json/tests.rs`
  - `crates/sc-observability-otlp/src/lib.rs`
  - `crates/sc-observability-otlp/src/lifecycle.rs`
  - `crates/sc-observability-otlp/src/lifecycle_tests.rs`
  - `crates/sc-observability-otlp/src/projectors.rs`
  - `crates/sc-observability-otlp/src/sdk/implementation.rs`
  - `crates/sc-observability-otlp/src/sdk/mod.rs`
  - `crates/sc-observability-otlp/src/sdk/tests.rs`
  - `crates/sc-observability-otlp/src/testing.rs`
  - `crates/sc-observability-types/Cargo.toml`
  - `crates/sc-observability-types/src/constants.rs`
  - `crates/sc-observability-types/src/diagnostic.rs`
  - `crates/sc-observability-types/src/error_codes.rs`
  - `crates/sc-observability-types/src/errors.rs`
  - `crates/sc-observability-types/src/errors_v2.rs`
  - `crates/sc-observability-types/src/events.rs`
  - `crates/sc-observability-types/src/lib.rs`
  - `crates/sc-observability-types/src/metric.rs`
  - `crates/sc-observability-types/src/process.rs`
  - `crates/sc-observability-types/src/projection.rs`
  - `crates/sc-observability-types/src/signals_v2.rs`
  - `crates/sc-observability-types/src/span.rs`
  - `crates/sc-observability-types/src/tracing.rs`
  - `crates/sc-observability-types/src/validation.rs`
  - `crates/sc-observability-types/tests/neutral_contracts.rs`
  - `crates/sc-observability/Cargo.toml`
  - `crates/sc-observability/src/constants.rs`
  - `crates/sc-observability/src/error_codes.rs`
  - `crates/sc-observability/tests/fixtures/bp1-published-v1.2.0-baseline/Cargo.toml`
  - `crates/sc-observability/tests/fixtures/bp1-published-v1.2.0-consumer/Cargo.toml`
  - `crates/sc-observe/Cargo.toml`
  - `crates/sc-observe/src/constants.rs`
  - `crates/sc-observe/src/error_codes.rs`
  - `docs/api-design.md`
  - `docs/architecture.md`
  - `docs/plans/phase-b/evidence/b3-final/Cargo.toml`
  - `docs/plans/phase-d/sprint-d-12-types-and-otlp-contract.md`
  - `docs/requirements.md`
  - `examples/atm-adapter-example/Cargo.toml`
  - `examples/custom-sink-example/Cargo.toml`
  - `examples/log-settings/Cargo.toml`
  - `examples/otlp-legacy/Cargo.toml`
  - `examples/rust-python-logging/Cargo.toml`
  - `examples/tauri-logging/src-tauri/Cargo.toml`
  - `scripts/ci/fixtures/error-migration/legacy/Cargo.toml`
  - `scripts/ci/fixtures/error-migration/migrated/Cargo.toml`
  - `scripts/ci/fixtures/error-migration/partial/Cargo.toml`
  - `scripts/ci/validate_dependency_bans.sh`
  - `scripts/ci/validate_repo_boundaries.sh`

## Goal

Close the types/OTLP contract boundary before independent implementations. ADR acceptance is recorded by merged PR #225 (2026-09-26); verify its accepted contract when implementing.

## Deliverables

1. Record the user's acceptance of ADR-017 and ADR-018 dated 2026-09-26 via merged PR #225; update both ADR status lines, architecture status header/navigation, section 6 and boundary records before production changes. The user ruling is recorded; verify the accepted status from PR #225. Amend docs/requirements.md: explicitly scope PHB-003/004/005 to Phase B and 1.x and add PHD-001/002 for canonical errors/removal and PHD-003/004 for dual backends/config. Record ADR-019 implementation decisions and its plan-fix acceptance gate; preserve ADR-012 historical 1.x scope.

2. Add the nine canonical non-exhaustive error enums under the temporary v2 module path while retaining functioning 1.x exports, ErrorContext semantics and the single cause-to-variant mapping below. Define MetricModelError and all OTLP ConfigFailure/runtime variants before implementation beads consume them; keep each crate's codes and constants in its one registry/module.

3. Define the neutral span/metric contracts, validated HistogramPoint serde and temporal validation, and matching DTO/schema conversion specifications consumed independently by obs-d-19 and obs-d-20. Freeze stable operational wire fields and canonical error projection before either wave-2 implementation; obs-d-20 does not wait on obs-d-19 output. Keep transport dependencies out of neutral types.

4. Define crate-private exporter/lifecycle traits, ExporterSet, factory/test-double contract, positive durations, dual record/byte admission bounds, defaults, validation order and stable failure mappings. Add named contract tests in contract_tests.rs and neutral_contracts.rs.

5. Own Cargo manifests (except examples/otlp-sdk/Cargo.toml, owned by obs-d-7), Cargo.lock, workspace member/feature declarations, OTLP module registration stubs, and dependency/boundary allowlists. Declare sdk/mod.rs and legacy_http_json/mod.rs once, with separate implementation.rs and tests.rs owned by D.7/D.8. Hoist the shared lifecycle interface into contracts.rs; declare D.6 lifecycle.rs and D.18 facade composition using the contract fixture until those owners supply production behavior.

6. Apply the 2.0 workspace/package/dependency version bump and resolve Cargo.lock; update docs/requirements.md (including OTLP-005/020/021), docs/architecture.md and docs/api-design.md to this single contract. D.18 owns release notes, approvals and inventory alignment; obs-d-7 alone owns its example manifest exception; implementation beads do not edit normative docs.

## This Sprint Does Not Close

Production adapters, lifecycle implementation, local duplicate-removal migrations and final release/public API composition close in D.1–D.8/D.14–D.18; no collector or publication claim is made here.

## Design

## D.4 canonical error-enum inventory

obs-d-12 (ADR-017/018) owns the nine public, `#[non_exhaustive]` error enums. Each named variant carries `Box<ErrorContext>`; callers preserve the diagnostic source rather than constructing a tuple wrapper. These are the only migration targets used by D14–D17.

```rust
#[non_exhaustive] pub enum IdentityError { Process { context: Box<ErrorContext> } }
#[non_exhaustive] pub enum InitError { Configuration { context: Box<ErrorContext> }, Runtime { context: Box<ErrorContext> } }
#[non_exhaustive] pub enum EventError { Validation { context: Box<ErrorContext> }, Routing { context: Box<ErrorContext> } }
#[non_exhaustive] pub enum FlushError { Drain { context: Box<ErrorContext> } }
#[non_exhaustive] pub enum ShutdownError { Timeout { context: Box<ErrorContext> }, Drain { context: Box<ErrorContext> } }
#[non_exhaustive] pub enum ProjectionError { Projection { context: Box<ErrorContext> } }
#[non_exhaustive] pub enum SubscriberError { Subscriber { context: Box<ErrorContext> } }
#[non_exhaustive] pub enum LogSinkError { Write { context: Box<ErrorContext> }, Flush { context: Box<ErrorContext> } }
#[non_exhaustive] pub enum ExportError {
    Transport { context: Box<ErrorContext> },
    BlockingBackendInAsyncContext { context: Box<ErrorContext> },
    AsyncLifecycleRequired { context: Box<ErrorContext> },
    RuntimeTerminated { context: Box<ErrorContext> },
    LifecycleTimeout { context: Box<ErrorContext> },
    QueueFull { context: Box<ErrorContext> },
    WorkerTerminated { context: Box<ErrorContext> },
    ShutdownCancelledRetry { context: Box<ErrorContext> },
    RetryDeadlineExhausted { context: Box<ErrorContext> },
    NonRetryableHttpStatus { context: Box<ErrorContext> },
    RetryAttemptsExhausted { context: Box<ErrorContext> },
    TerminalExportFailure { context: Box<ErrorContext> },
}
```

`sc-observability-types/src/diagnostic.rs`, `lib.rs`, `process.rs`, and `projection.rs` expose the supporting context, re-exports, process, and projection signatures. `typed.rs` is the obs-d-13 logging-contract owner because it defines `LogFailure` and `TryLogFailure`.

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

### Validated transport contract

D.12 is the sole contract owner of every transport-bound field, default, validation,
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

`queue_capacity` counts admitted records, not batches, and is validated as `1..=65_536`. A separate checked `queue_byte_capacity` defaults to 16 MiB, has a hard 64 MiB maximum, and bounds the serialized payload bytes held by all queued/in-flight batches. Admission reserves both record and byte credits atomically; either exhausted budget returns QueueFull. Records larger than 1 MiB are rejected before enqueue; batches split at 512 records or 1 MiB. The queue cannot retain 65,536 one-MiB batches. A 413 is terminal for that split batch,
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
    QueueByteCapacity,
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
    queue_capacity: QueueCapacity,
    queue_byte_capacity: QueueByteCapacity,
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

### Stable failure inventory

This is the complete transport/lifecycle/configuration stable-error
inventory. D.12 exclusively owns these variants, codes, owning types, mappings,
and documentation; D.12 also defines the `MetricModelError` rows, and later
sprints consume both tables without adding or restating rows. Configuration
rows are construction-only `ConfigFailure` variants. Every runtime/lifecycle
variant except `Shutdown` is owned by D.12 canonical `ExportError`; `Shutdown`
is owned by `TelemetryError`. Construction never uses an additional wrapper variant: it
returns the named `ConfigFailure` rows below. A legacy async-context failure
during construction is the redacted `Diagnostic` source of
`TransportConstructionFailed`; the same condition during synchronous lifecycle
is `BlockingBackendInAsyncContext` directly. Emit/lifecycle façades convert
runtime failures to `TelemetryError`, D.12 canonical `FlushError`, or D.4's
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
| `InvalidQueueByteCapacity` | `OTLP_CONFIG_QUEUE_BYTE_CAPACITY` | `ConfigFailure` | zero, overflow or aggregate byte bound above 64 MiB | choose 1..=64 MiB (default 16 MiB) | field/value only | after config correction |
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

## Ownership and one cause-to-variant mapping

The shared nine definitions survive in sc-observability-types. obs-d-4/14/15/16 migrate call sites/tests to canonical errors without removing 1.x wrappers, classification or adapters; obs-d-18 alone removes those compatibility surfaces. The PHB-002 exception remains for companion-specific errors such as DetachError. D.1 migrates constructions in runtime.rs, D.3 in builder.rs, and D.13 in settings.rs/typed.rs. obs-d-19 migrates DTO/schema and generated models, obs-d-20 migrates language adapters; obs-d-18 composes completed artifacts and activates the OTLP facade. One file has one owner in a wave. Later owners may edit only recorded handoff paths; stable registry and normative-doc ownership stays D.12.

| Cause | Canonical variant |
| --- | --- |
| process identity validation | IdentityError::Process |
| invalid configuration before construction | InitError::Configuration |
| thread/client/provider startup failure | InitError::Runtime |
| invalid event payload | EventError::Validation |
| event routing failure | EventError::Routing |
| flush drain/export failure, including timeout at flush | FlushError::Drain |
| shutdown deadline exceeded | ShutdownError::Timeout |
| other shutdown drain/provider failure | ShutdownError::Drain |
| projection/subscriber callback failure | ProjectionError::Projection / SubscriberError::Subscriber respectively |
| sink write / flush failure | LogSinkError::Write / LogSinkError::Flush respectively |
| OTLP runtime cause | the identically named ExportError variant in the stable failure inventory |

Do not replace the existing ObservationError::{Shutdown,QueueFull,RoutingFailure} runtime guards with EventError; preserve them and migrate their nested shared sources. TelemetryError::Shutdown remains its existing runtime guard; export errors are wrapped preserving the precise ExportError variant and code. No generic ExportError::Lifecycle mapping is permitted.

ErrorContext is the existing diagnostic context, not a new bag of public fields: retain Diagnostic's code/message/remediation/docs/details plus its typed source/backtrace. Construct through the remediation-required constructor; code and remediation are mandatory. Process, route, sink, projector, config_field, queue_depth and deadline_ms are redacted bounded Diagnostic.details keys, never invented ErrorContext struct fields. Preserve the original typed source identity. ConfigFailure is the detailed OTLP construction error; Telemetry::new returns InitError::Configuration with ConfigFailure as its typed source for invalid configuration, or InitError::Runtime for actual initialization failure, satisfying OTLP-005. No second competing construction return type.

MetricModelError is defined in types as non-exhaustive InvalidHistogram, InvalidTemporality and InvalidInterval, each carrying boxed ErrorContext and a stable SC_METRIC_* registry code. A histogram validates finite ordered bounds, bucket length and sum/count before constructing; temporal validation retains the specified Delta/Cumulative start-time rules.

ValidatedTransportBounds includes checked QueueCapacity and QueueByteCapacity. Byte arithmetic is checked; each admitted record/batch releases both credits exactly once on success/drop. Accessors provide Duration/usize/u8 values and AsRef where appropriate; no DerefMut or unchecked constructor bypasses validation. Record defaults and upper bounds only in constants.rs.

## Dependency and module declaration contract

D.12 records the optional otlp-sdk feature's reviewed opentelemetry family pins and the legacy-http-json feature's reqwest =0.12.28 (blocking/json/rustls-tls, default features off), httpdate =1.0.3, getrandom and tokio rt/sync allowlist once in architecture section 6 under the ADR-019 amendment to ADR-018. Legacy requires no caller runtime and no SDK/tonic dependency; reqwest's internal Tokio graph is explicit. Existing boundary validators and Cargo feature tests enforce this; no new parallel validator framework.

The only exporter traits and ExporterSet are pub(crate), as OTLP-011 and ADR-018 require. No public Exporter, undefined Signal, public ExporterSet or facade re-export is added. D.12's factory contract and recording test doubles compile without real SDK/legacy implementations. Module declarations expose separate contract and implementation files so D.7 and D.8 are siblings. obs-d-12 registers future log-settings/legacy examples and workspace members; obs-d-7 owns examples/otlp-sdk/Cargo.toml as the explicit exception.

Contract tests are self-contained and do not require completed production adapters. Any compatibility scaffolding needed for the existing workspace is explicitly transitional contract glue, not a claimed production implementation; each retiring consumer is named above, and D.18's migration gate rejects any leftover obsolete wrapper. Contract closure requires workspace compilation, not production collector behavior.

## Handoff to obs-d-18 (wave 3)

Created/staged by obs-d-12, owned by obs-d-18 from wave 3; after this bead closes it makes no further edits. The receiver consumes the staged contract/implementation and owns production completion or final compatibility retirement.

- `crates/sc-observability-otlp/src/config.rs`
- `crates/sc-observability-otlp/src/contracts.rs`
- `crates/sc-observability-otlp/src/lib.rs`
- `crates/sc-observability-types/src/diagnostic.rs`
- `crates/sc-observability-types/src/errors.rs`
- `crates/sc-observability-types/src/errors_v2.rs`
- `crates/sc-observability-types/src/events.rs`
- `crates/sc-observability-types/src/lib.rs`
- `crates/sc-observability-types/src/metric.rs`
- `crates/sc-observability-types/src/process.rs`
- `crates/sc-observability-types/src/projection.rs`
- `crates/sc-observability-types/src/signals_v2.rs`
- `crates/sc-observability-types/src/span.rs`
- `crates/sc-observability-types/src/tracing.rs`
- `crates/sc-observability-types/src/validation.rs`
- `crates/sc-observability-types/tests/neutral_contracts.rs`

## Handoff to obs-d-8 (wave 2)

Created/staged by obs-d-12, owned by obs-d-8 from wave 2; after this bead closes it makes no further edits. The receiver consumes the staged contract/implementation and owns production completion or final compatibility retirement.

- `crates/sc-observability-otlp/src/legacy_http_json/implementation.rs`
- `crates/sc-observability-otlp/src/legacy_http_json/tests.rs`

## Handoff to obs-d-6 (wave 2)

Created/staged by obs-d-12, owned by obs-d-6 from wave 2; after this bead closes it makes no further edits. The receiver consumes the staged contract/implementation and owns production completion or final compatibility retirement.

- `crates/sc-observability-otlp/src/lifecycle.rs`
- `crates/sc-observability-otlp/src/lifecycle_tests.rs`

## Handoff to obs-d-7 (wave 2)

Created/staged by obs-d-12, owned by obs-d-7 from wave 2; after this bead closes it makes no further edits. The receiver consumes the staged contract/implementation and owns production completion or final compatibility retirement.

- `crates/sc-observability-otlp/src/sdk/implementation.rs`
- `crates/sc-observability-otlp/src/sdk/tests.rs`


## Module roots and standalone contract closure

obs-d-12 owns and stubs sdk/mod.rs and legacy_http_json/mod.rs, including their module declaration lines; obs-d-7/8 own only their respective implementation.rs and tests.rs after the staged-file handoff; examples/otlp-sdk/Cargo.toml is owned by obs-d-7. Each module root contains `mod implementation;` and `#[cfg(test)] mod tests;`. obs-d-12 creates compilable placeholders for those declared files and creates assembly.rs/projectors.rs placeholders before declaring them in lib.rs. obs-d-5 replaces its assembly/projector placeholders after contract sanity. Module roots remain read-only to implementation consumers. Contract closure runs with either wave-1 root absent; it cannot rely on obs-d-13 landing first.

## Registry and validation amendment (ADR-019)

The single registry for types-owned ExportError, ConfigFailure and TelemetryError OTLP_* codes is crates/sc-observability-types/src/error_codes.rs, in its named otlp submodule. crates/sc-observability-otlp/src/error_codes.rs only re-exports those constants. Companion-only DetachError constants are in the bridge registry; core-only settings/registration constants are in the core registry. obs-d-12 installs all these rows; obs-d-13 specifies them without importing them in wave 1. No second string-definition registry is created.

ConfigFailure's complete payload-bearing construction variants are ZeroDuration, DurationOverflow, InvalidBoundOrdering, InvalidJitterPercent, InvalidQueueCapacity, InvalidQueueByteCapacity, ConfigFieldNotApplicable, InsecureTransportRejected, InvalidEndpoint, InvalidHeader, TransportConstructionFailed, UnsupportedBackend, UnsupportedProtocol and TokioRuntimeRequired. Each carries boxed ErrorContext; the existing source and redacted field/value/origin metadata are preserved. InvalidQueueCapacity maps only to OTLP_CONFIG_QUEUE_CAPACITY. InvalidQueueByteCapacity maps only to OTLP_CONFIG_QUEUE_BYTE_CAPACITY for zero, arithmetic overflow or values above 64 MiB; the default is 16 MiB. QueueByteCapacity is a checked strong type and uses checked credit arithmetic.

## Boundary enforcement scope

Delete the proposed six additional mechanical boundary rules. Rust visibility, types and Cargo feature/dependency graphs enforce private exporter ownership and crate edges; do not duplicate them with textual signature gates. Keep the existing source/provenance and ATM-specific env/import checks, which Cargo cannot infer. If a missing provenance/env check is demonstrated, extend validate_repo_boundaries.sh or its existing underlying consumer, with the failing case, rather than create a new validator. No speculative rule or new incident claim is authorized.


## Handoff to obs-d-5

Handoff to obs-d-5: D.12 stages the validated neutral signal and MetricModelError contract plus module scaffolding; obs-d-5 consumes those artifacts and owns only OTLP assembly/projector behavior, with D.12-owned files read-only after handoff. assembly.rs and projectors.rs are created by obs-d-12 and owned by obs-d-5 from wave 2; the normative types/config/module roots stay read-only.


## Handoff to obs-d-1

obs-d-12 produces canonical errors and registry rows. obs-d-1 consumes those read-only artifacts together with obs-d-13 concrete signature specifications, binding the canonical errors/codes in its owned runtime.rs during wave 2. obs-d-13 does not compile against this new artifact in wave 1; no contract-root edge is introduced.


## Handoff to obs-d-2

obs-d-12 produces canonical errors and registry rows. obs-d-2 consumes those read-only artifacts together with obs-d-13 concrete signature specifications, binding the canonical errors/codes in its owned bridge.rs during wave 2. obs-d-13 does not compile against this new artifact in wave 1; no contract-root edge is introduced.


## Handoff to obs-d-3

obs-d-12 produces canonical errors and registry rows. obs-d-3 consumes those read-only artifacts together with obs-d-13 concrete signature specifications, binding the canonical errors/codes in its owned builder.rs during wave 2. obs-d-13 does not compile against this new artifact in wave 1; no contract-root edge is introduced.


## Handoff to obs-d-19

obs-d-12 freezes canonical error/signal and wire-projection specifications in docs/api-design.md. obs-d-19 consumes the validated contract after obs-d-12-sanity; contract source and registry files remain read-only. The same stable wire contract lets obs-d-19/20 close independently with local conversion/transport fixtures.


## Handoff to obs-d-20

obs-d-12 freezes canonical error/signal and wire-projection specifications in docs/api-design.md. obs-d-20 consumes the validated contract after obs-d-12-sanity; contract source and registry files remain read-only. The same stable wire contract lets obs-d-19/20 close independently with local conversion/transport fixtures.


## Release gate and scope

This boundary releases only its named artifact to obs-d-18 after its paired sanity check; final 2.0 semver/API approval, obsolete-wrapper removal and release inventory are obs-d-18 gates. The phase-root workspace invariant applies once to every sprint.


## Handoff to obs-d-17

Handoff to obs-d-17: the canonical cause-to-variant mapping and ErrorContext contract are consumed by the log-consumer check and named examples; consumers preserve typed variants, diagnostics, and source identity.


## Handoff to obs-d-18 — project-plan row

obs-d-18 owns docs/project-plan.md exclusively in wave 3. obs-d-12 supplies this exact contract row: "obs-d-12 owns the canonical error/signal and OTLP config/default/validation contracts and Cargo 2.0 version bump; obs-d-18 owns final release baseline, approvals, migration guidance and inventory alignment." The round-2 plan fix repairs the stale prose now; the implementation sprint keeps that row aligned with the accepted contract.

## Acceptance criteria

- [ ] Acceptance record cites merged PR #225 dated 2026-09-26; ADR-017/018, partial ADR-012 supersession, status header and boundary records agree (#1).
- [ ] `cargo test -p sc-observability-types --test neutral_contracts --locked` executes named canonical_error_variants_preserve_context, metric_model_failures and histogram_point_serde_rejects_invalid tests (#2–#3).
- [ ] `cargo test -p sc-observability-otlp --lib contract_tests --all-features --locked` executes validation_order, resolved_defaults, stable_failure_codes, record_and_byte_capacity and fake_exporter_contract tests in D.12-owned files (#4). Verify test count is nonzero.
- [ ] `cargo check --workspace --all-features --locked` and `bash scripts/ci/validate_repo_boundaries.sh` pass using contract fixtures/stubs without a production adapter (#5).
- [ ] Cargo metadata reports the 2.0 package/dependency version everywhere; Cargo.lock resolves that graph. Normative doc review confirms OTLP-005 conversion, OTLP-020/021 ownership, ADR status and the exact same contract; release documents are deliberately D.18 closure (#6).

- [ ] #1: requirements explicitly scope PHB-003/004/005 to 1.x and define PHD-001–004; ADR-019 is present with its actual acceptance status.
- [ ] #4: contract_tests::record_and_byte_capacity rejects zero/overflow/out-of-range record and byte capacities and asserts their distinct registry codes.
- [ ] #5: all declared modules exist; cargo check --workspace --all-features --locked passes without obs-d-13 or real backend implementations. The root workspace invariant also applies.
