# d-12: types 2.0 contract

Generated projection of `obs-d-12`; the bead is authoritative.

## Plan metadata

- Wave: 1
- Layer: 1
- Assignee / model: aobs / astra
- Relation: `root`
- Closure: `contract`
- Target boundary: sc-observability-types 2.0 contract
- Branch: `sprint/d-12-types-and-otlp-contract`
- Worktree: `/Users/randlee/github/sc-observability-worktrees/sprint/d-12-types-and-otlp-contract`
- PR target (merge order only): `integrate/phase-d`
- Blocked by: `obs-phase-d-plan-qa`
- Requirements: LAY-001, LAY-006, LAY-007, LOG-014, LOG-016, LOG-018, LOG-019, LOG-047, LOG-048, NFR-005, NFR-008, NFR-010, NFR-011, NFR-012, OBS-003, OBS-007, OBS-012, OBS-021, OTLP-005, OTLP-006, OTLP-007, OTLP-013, OTLP-021, PHB-002, PHB-003, PHB-004, PHB-005, PHB-010, PHB-012, PHB-013, PHD-001, PHD-002, SRC-001, SRC-002, SRC-003, SRC-004, SRC-005, SRC-006, TYP-001, TYP-002, TYP-003, TYP-004, TYP-005, TYP-006, TYP-007, TYP-008, TYP-009, TYP-010, TYP-011, TYP-012, TYP-013, TYP-014, TYP-015, TYP-016, TYP-017, TYP-018, TYP-019, TYP-020, TYP-021, TYP-022, TYP-023, TYP-024, TYP-030, TYP-031, TYP-039
- ADRs: ADR-002, ADR-003, ADR-005, ADR-006, ADR-011, ADR-014, ADR-015, ADR-017, ADR-018, ADR-019
- Owned paths (metadata projection):
  - `crates/sc-observability-dto/src/error_codes.rs`
  - `crates/sc-observability-log/src/error_codes.rs`
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
  - `crates/sc-observability/src/constants.rs`
  - `crates/sc-observability/src/error_codes.rs`
  - `crates/sc-observe/src/constants.rs`
  - `crates/sc-observe/src/error_codes.rs`
  - `docs/api-design.md`
  - `docs/architecture.md`
  - `docs/plans/phase-d/sprint-d-12-types-and-otlp-contract.md`
  - `docs/requirements.md`

## Goal

Close the shared types 2.0 contract so types-only consumers can start without waiting for OTLP configuration, module registration or workspace version activation. obs-d-21 consumes this artifact as a second stage of numbered wave 1.

## Deliverables

1. Maintain accepted ADR-017/018 (PR #225) and ADR-019 (PR #227), the PHB-003/004/005 historical 1.x carve-out and PHD-001/002 types/major-release records in owned normative docs. Publish the types-side API/error/model and wire-projection contract, including the existing reviewed OTLP reference specification in shared documents as a read-only handoff to obs-d-21. Retain the types crate Cargo.toml fence; declare its 2.0 surface under the v2 module path at the current package version. Hand off only its version literal for obs-d-21's atomic workspace 2.0 bump. [PHD-001/002; PHB-003/004/005; NFR-010/011/012]

2. Define the nine canonical non-exhaustive enums, MetricModelError, every ConfigFailure and ExportError variant, ErrorContext preservation and the single cause-to-variant/stable-code inventory below. Types owns these errors even when OTLP consumes them. Keep shared and companion/core registry definitions and constants in their existing dedicated modules; no OTLP runtime implementation belongs here. [TYP-001/003/004/007/030; SRC-001–006; PHD-001]

3. Define neutral span/metric contracts, checked HistogramPoint serde/temporality and DTO/schema conversion specifications consumed independently by obs-d-19/20. Own neutral_contracts.rs tests; keep transport/runtime dependencies out of types. Freeze the stable operational wire envelope before releasing either language boundary. [TYP-002/008–024; OBS-003/007; PHB-002/010/012/013]

## This Sprint Does Not Close

obs-d-21 owns OTLP exporter/config/lifecycle interfaces, stubs, Cargo.lock and atomic workspace version activation. obs-d-5–8 implement transport behavior; obs-d-18 composes artifacts and removes compatibility. No backend, collector or release claim closes here.

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

## Registry and validation amendment (ADR-019)

The single registry for types-owned ExportError, ConfigFailure and TelemetryError OTLP_* codes is crates/sc-observability-types/src/error_codes.rs, in its named otlp submodule. crates/sc-observability-otlp/src/error_codes.rs only re-exports those constants. Companion-only DetachError constants are in the bridge registry; core-only settings/registration constants are in the core registry. obs-d-12 installs all these rows; obs-d-13 specifies them without importing them in wave 1. No second string-definition registry is created.

ConfigFailure's complete payload-bearing construction variants are ZeroDuration, DurationOverflow, InvalidBoundOrdering, InvalidJitterPercent, InvalidQueueCapacity, InvalidQueueByteCapacity, ConfigFieldNotApplicable, InsecureTransportRejected, InvalidEndpoint, InvalidHeader, TransportConstructionFailed, UnsupportedBackend, UnsupportedProtocol and TokioRuntimeRequired. Each carries boxed ErrorContext; the existing source and redacted field/value/origin metadata are preserved. InvalidQueueCapacity maps only to OTLP_CONFIG_QUEUE_CAPACITY. InvalidQueueByteCapacity maps only to OTLP_CONFIG_QUEUE_BYTE_CAPACITY for zero, arithmetic overflow or values above 64 MiB; the default is 16 MiB. obs-d-21 owns the checked QueueByteCapacity type and credit arithmetic; this bead defines its typed failure result.


## Stable failure inventory


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
runtime failures to `TelemetryError`, D.12 canonical `FlushError`, or obs-d-12's
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


## Complete types-owned construction error definition

ConfigFailure is defined beside the canonical types-owned errors, not in the OTLP crate. Each non-exhaustive variant below has `context: Box<ErrorContext>`: ZeroDuration, DurationOverflow, InvalidBoundOrdering, InvalidJitterPercent, InvalidQueueCapacity, InvalidQueueByteCapacity, ConfigFieldNotApplicable, InsecureTransportRejected, InvalidEndpoint, InvalidHeader, TransportConstructionFailed, UnsupportedBackend, UnsupportedProtocol, TokioRuntimeRequired. obs-d-21 consumes these variants unchanged; its checked config may not add a parallel enum.

## Handoff to obs-d-21

The consumed artifact is the canonical types v2 error/model surface, complete stable error registry and neutral_contracts.rs test suite. obs-d-21 waits for obs-d-12-sanity, imports those types/registry constants and may run the shared tests read-only. Shared docs/requirements.md, docs/architecture.md and docs/api-design.md remain owned by obs-d-12; the already reviewed OTLP specification and section6 allowlist are handed off as read-only normative input. obs-d-21 records its exact implementation pins in its Cargo.lock and boundary manifest, not by duplicating normative records.

Handoff to obs-d-21: version literal in crates/sc-observability-types/Cargo.toml. The fence stays with obs-d-12. Under lead ruling 01M3E7Z46AA9DNNMC73BDE37JG, obs-d-21 may change only that version literal as part of its atomic workspace bump; it may not change dependencies, features or other manifest fields. obs-d-12 closes with current-version workspace pins intact, avoiding a mismatched intermediate Cargo graph.
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



## Handoff to obs-d-18 (wave 3)

obs-d-18 activates canonical exports and removes compatibility after its implementation gates. It receives these types-owned files; registry/normative records otherwise remain read-only.

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

## Handoff to obs-d-18 — project-plan row

obs-d-18 alone owns docs/project-plan.md in dev. Use: "obs-d-12 owns canonical errors, neutral models and wire projection; obs-d-21 owns OTLP contracts, registration and atomic workspace version activation; obs-d-18 owns final release baseline, approval and inventory alignment."

## Acceptance criteria

- [ ] #1: ADR acceptance records and PHB-003/004/005/PHD-001/002 agree with the reviewed major-release scope; the types manifest still resolves with the current-version workspace and explicitly records the version-literal handoff.
- [ ] #2–3: cargo test -p sc-observability-types --test neutral_contracts --locked runs nonzero canonical_error_variants_preserve_context, metric_model_failures, histogram_point_serde_rejects_invalid and stable_failure_codes cases. All ConfigFailure/ExportError variants and registry rows are owned by types; code/remediation/source survive.
- [ ] #2: zero/out-of-range record and byte capacity failures have distinct InvalidQueueCapacity/InvalidQueueByteCapacity variants and OTLP_CONFIG_QUEUE_CAPACITY/OTLP_CONFIG_QUEUE_BYTE_CAPACITY codes; this checks error definitions, not transport admission.
- [ ] #1–3: the root workspace invariant passes without obs-d-21, OTLP module stubs or production adapters. No requirement to activate the workspace 2.0 Cargo version blocks types closure.
