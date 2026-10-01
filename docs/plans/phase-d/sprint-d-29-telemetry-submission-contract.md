# d-29: telemetry submission contract

## Plan metadata

- Wave: 5.1 (wave-5 contract)
- Layer: 1 of the wave-5 stack (d-29 → d-33 → d-30 → d-31 → d-32)
- Assignee / model: aobs / astra (difficulty: hard)
- Closure: `contract`
- Target boundary: `BOUNDARY-ScObservabilityTypes` (wave-5 contract: the neutral signal types and the submission contract in `sc_observability_types::otlp`)
- Branch: `sprint/d-29-telemetry-submission-contract`
- Worktree: `/Users/randlee/github/sc-observability-worktrees/sprint/d-29-telemetry-submission-contract`
- PR target: `integrate/phase-d`
- Blocked by: `obs-d-26-sanity`, `obs-d-28-sanity`
- Requirements: PHD-002, PHD-003, PHD-004, PHD-005, PHD-006, PHD-007, PHD-008, PHD-009, PHD-010, PHD-013
- ADRs: ADR-018, ADR-019, ADR-020, ADR-021
- Owned paths:
  - `Cargo.toml`
  - `Cargo.lock`
  - `policy/otlp-transport.toml`
  - `policy/deny-durable-store.toml`
  - `boundaries/sc-observability-types/types.toml`
  - `boundaries/sc-observability-otlp/otlp.toml`
  - `boundaries/sc-observability-py/python.toml`
  - `boundaries/sc-otel-cli/**`
  - `crates/sc-observability-types/Cargo.toml` (feature `test-double` only)
  - `crates/sc-observability-types/src/otlp/**`
  - `crates/sc-observability-types/tests/otlp_signals_contract.rs`
  - `crates/sc-observability-types/tests/otlp_submission_contract.rs`
  - `crates/sc-observability-types/tests/fixtures/otlp_submission/**`
  - `crates/sc-observability-otlp/Cargo.toml`
  - `crates/sc-observability-otlp/src/lib.rs` (one registration hunk only; see the d-18 note)
  - `crates/sc-observability-otlp/src/durable/mod.rs` (staged stub; handed off to d-33)
  - `crates/sc-observability-otlp/src/durable/schema.sql`
  - `crates/sc-observability-otlp/src/durable/config_file.rs` (`.sc/telemetry.yaml` loader)
  - `crates/sc-otel-cli/Cargo.toml`
  - `crates/sc-otel-cli/src/main.rs` (staged stub; handed off to d-31)
  - `bindings/python/sc-observability-py/Cargo.toml`
  - `docs/architecture.md` (ADR-021, §6 rows, binding edges)
  - `docs/requirements.md` (PHD-005–013)
  - `docs/api-approvals/phase-d-wave5-telemetry-submission.json` (new file; see Closeout gate)
  - `docs/plans/phase-d/sprint-d-29-telemetry-submission-contract.md`

## Relations

- `must_follow` d-26: consumes the compatible OTLP config adapters
  (`OtelConfig`, `ExporterBackend::SyncHttp`, `SyncHttpRetryPolicy`) that
  `TelemetryClientConfig` resolves into.
- `must_follow` d-28: consumes the 1.x release/compat baseline that the
  additive public surface is checked against.
- d-33, d-30 and d-31 `must_follow` d-29. They consume the `TelemetryClient`
  trait, the envelope/receipt/error types, `schema.sql`, the test double and
  the golden fixtures.
- Shared-file note: `crates/sc-observability-otlp/src/lib.rs` is also in the
  in-flight d-18 fence. d-29 adds only
  `#[cfg(feature = "durable-store")] pub mod durable;`. If d-18 is unmerged,
  d-29 merges forward d-18's pushed head before every round. No DAG edge is
  added. The same rule covers the new approval file under
  `docs/api-approvals/**`, which is also in the d-18 fence: d-29 only adds a
  file there (lead ruling P2, 2026-10-01).

## Goal

Fix every wave-5 interface before any implementation, so that d-33, d-30 and
d-31 run in parallel without renegotiating. The signal types and envelope
canonicalization are fully implemented here, because they are the contract
both front ends must share. The store, drain and export are not implemented.

## Deliverables

1. Pin the protocol: workspace `opentelemetry-proto = "=0.33.0"` (already
   pinned), which vendors upstream **opentelemetry-proto v1.10.0**. Enable its
   `profiles` feature only where an encoder needs it; d-29 adds no prost
   dependency to the sync-http path. Commit the field inventory below as the
   doc comment of `crates/sc-observability-types/src/otlp/signals/mod.rs`.
   [PHD-013]
2. Add the neutral signal types in `crates/sc-observability-types/src/otlp/signals/`,
   declared from `src/otlp/mod.rs`, with validating constructors, serde and
   `From` conversions from `LogEvent`, `SpanRecord<SpanEnded>`, v2
   `MetricRecord`, `OtlpResource` and `OtlpInstrumentationScope`. Add no
   variant or field to any existing type. [PHD-005, PHD-006, PHD-013]
3. Add module `sc_observability_types::otlp::submission` (declared from
   `src/otlp/mod.rs`, not `src/lib.rs`; no new crate, lead ruling P1) with the
   submission contract: `SubmissionInput`, `SubmissionEnvelope`,
   `from_input` canonicalization, receipts, status, the error enums and their
   codes, `TelemetryClientConfig` and its precedence resolver, and the
   `TelemetryClient` trait. [PHD-005, PHD-007, PHD-008, PHD-010]
4. Add the `InMemoryTelemetryClient` test double and the public conformance
   suite `testing::conformance::run_all::<C: TelemetryClient>` behind the
   new `sc-observability-types` feature `test-double`, at the manifest's
   `allowed_test_double_paths`
   (`crates/sc-observability-types/src/otlp/submission/testing/**`). Add the
   golden fixture set at
   `crates/sc-observability-types/tests/fixtures/otlp_submission/golden/`.
   [PHD-009, PHD-010, PHD-013]
5. Commit `crates/sc-observability-otlp/src/durable/schema.sql` and the
   versioning policy, plus the staged `durable/mod.rs` stub declaring
   `DurableTelemetryClient`. Its methods return
   `AdmissionError::StoreUnavailable`, never `todo!()`. Implement
   `durable/config_file.rs` in full: `load_telemetry_file`, which parses
   `.sc/telemetry.yaml` with `serde-saphyr` into `TelemetryFileConfig`.
   [PHD-007, PHD-008, PHD-010]
6. Update manifests and allowlists: the `sc-otel-cli` workspace member, the
   `rusqlite =0.40.2` and `serde-saphyr =1.3.0` pins, the `durable-store`
   feature, the `sc-observability-types` `test-double` feature, the
   `otlp-telemetry` Python feature (with `test-hooks` extended by
   `sc-observability-types/test-double` for the d-30 double wheel), the
   `sc-otel-cli` skeleton with `publish = false` (dependencies
   `sc-observability-types` and `sc-observability-otlp` with `durable-store`;
   feature `test-double = ["sc-observability-types/test-double"]`), the new `sc-otel-cli`
   manifest and the edited types/otlp/python manifests, and the `policy/otlp-transport.toml` row, all per
   the boundary map in `docs/plans/telemetry-python-cli.md`. Run the
   dependency and license audit (cargo-deny with
   `policy/deny-durable-store.toml`) over the `durable-store` graph. [PHD-003, PHD-007, PHD-009, PHD-010]
7. Commit the normative text: ADR-021 Accepted (capability matrix, storage,
   layering, ownership, platform matrix, limits), PHD-005–013 amendments, the
   §6 crate row for `sc-otel-cli`, and the feature-gated
   `sc-observability-py → sc-observability-otlp` binding edge.
   [PHD-002, PHD-003, PHD-013]
8. Add contract tests: `crates/sc-observability-types/tests/otlp_signals_contract.rs`
   and `crates/sc-observability-types/tests/otlp_submission_contract.rs`
   (golden fixtures, precedence, error codes, test-double conformance).
   [PHD-005, PHD-006, PHD-013]
9. Prepare `docs/api-approvals/phase-d-wave5-telemetry-submission.json`
   listing every wave-5 public addition (the `otlp::signals` and
   `otlp::submission` items, `sc-observability-otlp::durable`, the
   `durable-store`, `test-double` and `otlp-telemetry` features). The user
   signs it at d-29 closeout. [PHD-002]

## This Sprint Does Not Close

- The store, drain, lease, export and per-variant OTLP encoding. d-33 owns
  them.
- Python bindings and the wheel feature in `pyproject.toml`. d-30 owns them.
- CLI argument parsing, output and exit-code behavior. d-31 owns them.
- Installed front-end submission, viewer readback, the sanity importer, and
  the D18/D9 re-run. d-32 owns them.

## Design

### Protocol pin and field inventory

Pin: Rust crate `opentelemetry-proto =0.33.0`, upstream tag `v1.10.0`.
Profiles are `opentelemetry.proto.profiles.v1development`, exported to
`/v1development/profiles`.

| Proto message (v1.10.0) | Fields | Neutral type |
| --- | --- | --- |
| `common.AnyValue` | `string_value`, `bool_value`, `int_value`, `double_value`, `array_value`, `kvlist_value`, `bytes_value` | `AnyValue::{String, Bool, Int, Double, Array, KvList, Bytes}` (plus `UInt`; see the policy below) |
| `common.AnyValue` | `string_value_strindex` | `AnyValue::StringIndex(StringIndex)`: an index into `ProfilesDictionary.string_table`, valid only inside a profiles payload |
| `common.KeyValue` | `key`, `value` | `KeyValues` entry `(String, AnyValue)`; duplicate keys rejected |
| `common.KeyValue` | `key_strindex` | `AttributeKey::Index(StringIndex)`, under the same profiles-only rule |
| `common.InstrumentationScope` | `name`, `version`, `attributes`, `dropped_attributes_count` | `InstrumentationScope` |
| `common.EntityRef` | `schema_url`, `type`, `id_keys`, `description_keys` | `EntityRef` |
| `resource.Resource` | `attributes`, `dropped_attributes_count`, `entity_refs` | `Resource` |
| `Resource*`/`Scope*` wrappers | `schema_url` | `Resource.schema_url`, `InstrumentationScope.schema_url` |
| `logs.LogRecord` | `time_unix_nano`, `observed_time_unix_nano`, `severity_number`, `severity_text`, `body`, `attributes`, `dropped_attributes_count`, `flags`, `trace_id`, `span_id`, `event_name` | `LogPoint` |
| `trace.Span` | `trace_id`, `span_id`, `trace_state`, `parent_span_id`, `flags`, `name`, `kind`, `start_time_unix_nano`, `end_time_unix_nano`, `attributes`, `dropped_attributes_count`, `events`, `dropped_events_count`, `links`, `dropped_links_count`, `status` | `SpanPoint` |
| `trace.Span.Event` | `time_unix_nano`, `name`, `attributes`, `dropped_attributes_count` | `SpanEventPoint` |
| `trace.Span.Link` | `trace_id`, `span_id`, `trace_state`, `attributes`, `dropped_attributes_count`, `flags` | `SpanLinkPoint` |
| `trace.Status` | `message`, `code` | `SpanStatusPoint` |
| `metrics.Metric` | `name`, `description`, `unit`, `metadata`, `data` | `MetricStream` |
| `metrics.Gauge` / `Sum` / `Histogram` / `ExponentialHistogram` / `Summary` | `data_points`, `aggregation_temporality`, `is_monotonic` | `MetricData::{Gauge, Sum, Histogram, ExponentialHistogram, Summary}` |
| `metrics.NumberDataPoint` | `attributes`, `start_time_unix_nano`, `time_unix_nano`, `as_double`/`as_int`, `exemplars`, `flags` | `NumberPoint` |
| `metrics.HistogramDataPoint` | `attributes`, `start_time_unix_nano`, `time_unix_nano`, `count`, `sum`, `bucket_counts`, `explicit_bounds`, `exemplars`, `flags`, `min`, `max` | `HistogramDataPoint` |
| `metrics.ExponentialHistogramDataPoint` | `attributes`, `start_time_unix_nano`, `time_unix_nano`, `count`, `sum`, `scale`, `zero_count`, `positive`, `negative`, `flags`, `exemplars`, `min`, `max`, `zero_threshold` | `ExponentialHistogramDataPoint`, `ExponentialBuckets` |
| `metrics.SummaryDataPoint` | `attributes`, `start_time_unix_nano`, `time_unix_nano`, `count`, `sum`, `quantile_values`, `flags` | `SummaryDataPoint`, `ValueAtQuantile` |
| `metrics.Exemplar` | `filtered_attributes`, `time_unix_nano`, `as_double`/`as_int`, `span_id`, `trace_id` | `Exemplar` |
| `profiles.ProfilesDictionary` | `mapping_table`, `location_table`, `function_table`, `link_table`, `string_table`, `attribute_table`, `stack_table` | `ProfilesDictionary` |
| `profiles.Profile` | `sample_type`, `samples`, `time_unix_nano`, `duration_nano`, `period_type`, `period`, `profile_id`, `dropped_attributes_count`, `original_payload_format`, `original_payload`, `attribute_indices` | `Profile` |
| `profiles.Sample`, `ValueType`, `Mapping`, `Stack`, `Location`, `Line`, `Function`, `Link`, `KeyValueAndUnit` | all fields | same-named neutral structs |
| any `double` field | NaN / ±Inf | `OtlpDouble` carries every IEEE-754 value, including NaN and ±Inf; see the non-finite policy |
| `tracez.proto` | all | excluded — out of scope (lead ruling P5, 2026-10-01): zPages, not an OTLP payload |
| `collector.*.Export*ServiceRequest/Response` | all | not neutral: built and parsed by the d-33 sync-http encoder |

Enumerations: `SeverityNumber` 0–24, `SpanKind` including `Unspecified`, and
`StatusCode::{Unset, Ok, Error}`. `AggregationTemporality::Unspecified` is
rejected as invalid input, because the spec forbids it on Sum and Histogram.
`DataPointFlags(u32)` keeps `NO_RECORDED_VALUE`. Span and log flags are kept
as `u32`.

### Neutral signal types (`sc_observability_types::otlp::signals`)

All new types are `#[non_exhaustive]` and constructed through `try_new` or
builders. Released exhaustive types are not extended.

```rust
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "data", rename_all = "snake_case")]
pub enum AnyValue {
    String(String),
    Bool(bool),
    Int(i64),
    /// Accepted as input; see the uint policy.
    #[serde(rename = "uint")]
    UInt(u64),
    Double(OtlpDouble),
    /// Serialized as lowercase hex in neutral JSON.
    Bytes(Vec<u8>),
    /// `string_value_strindex`; profiles payloads only.
    StringIndex(StringIndex),
    Array(Vec<AnyValue>),
    KvList(KeyValues),
}

/// Ordered; duplicate keys rejected by `try_from_iter`.
#[non_exhaustive]
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct KeyValues(Vec<(AttributeKey, AnyValue)>);

/// `key` or `key_strindex`. `Index` is valid only inside a profiles payload.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(untagged)]
pub enum AttributeKey { Name(String), Index(StringIndex) }

/// Index into `ProfilesDictionary.string_table` (proto `int32`, never negative).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct StringIndex(i32);

/// Any IEEE-754 double. Neutral and OTLP/JSON encode finite values as JSON
/// numbers and non-finite values as the proto-JSON strings "NaN", "Infinity"
/// and "-Infinity". Equality is bitwise, so NaN round trips compare equal.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct OtlpDouble(f64);

#[non_exhaustive]
pub struct Resource {
    pub attributes: KeyValues,
    pub dropped_attributes_count: u32,
    pub entity_refs: Vec<EntityRef>,
    pub schema_url: Option<String>,
}
#[non_exhaustive]
pub struct EntityRef { pub schema_url: Option<String>, pub r#type: String,
    pub id_keys: Vec<String>, pub description_keys: Vec<String> }
#[non_exhaustive]
pub struct InstrumentationScope {
    pub name: String,
    pub version: Option<String>,
    pub attributes: KeyValues,
    pub dropped_attributes_count: u32,
    pub schema_url: Option<String>,
}
impl From<OtlpResource> for Resource { /* field move */ }
impl From<OtlpInstrumentationScope> for InstrumentationScope { /* field move */ }

#[non_exhaustive]
pub struct ResourceRecord<T> { pub resource: Resource, pub scope: InstrumentationScope, pub record: T }

/// W3C tracestate; validated (≤32 list members, key/value grammar, ≤512 bytes).
#[non_exhaustive]
pub struct TraceState(String);
impl TraceState { pub fn try_new(value: impl Into<String>) -> Result<Self, SignalValidationError>; }

#[non_exhaustive]
pub struct LogPoint {
    pub time: Option<Timestamp>,
    pub observed_time: Timestamp,
    pub severity_number: SeverityNumber,
    pub severity_text: Option<String>,
    pub event_name: Option<String>,
    pub body: Option<AnyValue>,
    pub attributes: KeyValues,
    pub dropped_attributes_count: u32,
    pub flags: u32,
    pub trace_id: Option<TraceId>,
    pub span_id: Option<SpanId>,
}

#[non_exhaustive]
pub struct SpanPoint {
    pub trace_id: TraceId,
    pub span_id: SpanId,
    pub trace_state: Option<TraceState>,
    pub parent_span_id: Option<SpanId>,
    pub flags: u32,
    pub name: String,
    pub kind: SpanKindPoint,
    pub start_time: Timestamp,
    pub end_time: Timestamp,
    pub attributes: KeyValues,
    pub dropped_attributes_count: u32,
    pub events: Vec<SpanEventPoint>,
    pub dropped_events_count: u32,
    pub links: Vec<SpanLinkPoint>,
    pub dropped_links_count: u32,
    pub status: SpanStatusPoint,
}
#[non_exhaustive] pub struct SpanEventPoint { pub time: Timestamp, pub name: String,
    pub attributes: KeyValues, pub dropped_attributes_count: u32 }
#[non_exhaustive] pub struct SpanLinkPoint { pub trace_id: TraceId, pub span_id: SpanId,
    pub trace_state: Option<TraceState>, pub attributes: KeyValues,
    pub dropped_attributes_count: u32, pub flags: u32 }
#[non_exhaustive] pub struct SpanStatusPoint { pub code: StatusCode, pub message: Option<String> }

#[non_exhaustive]
pub struct MetricStream {
    pub name: MetricName,
    pub description: Option<String>,
    pub unit: Option<String>,
    pub metadata: KeyValues,
    pub data: MetricData,
}
#[non_exhaustive]
pub enum MetricData {
    Gauge { points: Vec<NumberPoint> },
    Sum { points: Vec<NumberPoint>, temporality: AggregationTemporality, monotonic: bool },
    Histogram { points: Vec<HistogramDataPoint>, temporality: AggregationTemporality },
    ExponentialHistogram { points: Vec<ExponentialHistogramDataPoint>, temporality: AggregationTemporality },
    Summary { points: Vec<SummaryDataPoint> },
}
#[non_exhaustive] pub enum NumberValue { Int(i64), Double(OtlpDouble) }
#[non_exhaustive] pub struct DataPointFlags(u32);
#[non_exhaustive]
pub struct NumberPoint { pub attributes: KeyValues, pub start_time: Option<Timestamp>,
    pub time: Timestamp, pub value: NumberValue, pub exemplars: Vec<Exemplar>, pub flags: DataPointFlags }
#[non_exhaustive]
pub struct HistogramDataPoint { pub attributes: KeyValues, pub start_time: Option<Timestamp>,
    pub time: Timestamp, pub count: u64, pub sum: Option<OtlpDouble>, pub bucket_counts: Vec<u64>,
    pub explicit_bounds: Vec<OtlpDouble>, pub exemplars: Vec<Exemplar>, pub flags: DataPointFlags,
    pub min: Option<OtlpDouble>, pub max: Option<OtlpDouble> }
#[non_exhaustive] pub struct ExponentialBuckets { pub offset: i32, pub bucket_counts: Vec<u64> }
#[non_exhaustive]
pub struct ExponentialHistogramDataPoint { pub attributes: KeyValues, pub start_time: Option<Timestamp>,
    pub time: Timestamp, pub count: u64, pub sum: Option<OtlpDouble>, pub scale: i32,
    pub zero_count: u64, pub zero_threshold: OtlpDouble, pub positive: ExponentialBuckets,
    pub negative: ExponentialBuckets, pub exemplars: Vec<Exemplar>, pub flags: DataPointFlags,
    pub min: Option<OtlpDouble>, pub max: Option<OtlpDouble> }
#[non_exhaustive] pub struct ValueAtQuantile { pub quantile: OtlpDouble, pub value: OtlpDouble }
#[non_exhaustive]
pub struct SummaryDataPoint { pub attributes: KeyValues, pub start_time: Option<Timestamp>,
    pub time: Timestamp, pub count: u64, pub sum: OtlpDouble,
    pub quantile_values: Vec<ValueAtQuantile>, pub flags: DataPointFlags }
#[non_exhaustive]
pub struct Exemplar { pub filtered_attributes: KeyValues, pub time: Timestamp,
    pub value: NumberValue, pub trace_id: Option<TraceId>, pub span_id: Option<SpanId> }

#[non_exhaustive]
pub struct ProfilesDictionary {
    pub mapping_table: Vec<Mapping>, pub location_table: Vec<Location>,
    pub function_table: Vec<Function>, pub link_table: Vec<ProfileLink>,
    pub string_table: Vec<String>, pub attribute_table: Vec<KeyValueAndUnit>,
    pub stack_table: Vec<Stack>,
}
#[non_exhaustive]
pub struct Profile {
    pub sample_type: Option<ValueType>, pub samples: Vec<Sample>, pub time: Timestamp,
    pub duration_nanos: u64, pub period_type: Option<ValueType>, pub period: i64,
    pub profile_id: [u8; 16], pub dropped_attributes_count: u32,
    pub original_payload_format: Option<String>, pub original_payload: Vec<u8>,
    pub attribute_indices: Vec<i32>,
}
// Sample, ValueType, Mapping, Stack, Location, Line, Function, ProfileLink and
// KeyValueAndUnit mirror the v1.10.0 fields listed in the inventory.
impl ProfilesDictionary {
    /// Index 0 of every table is the required zero-value entry; every index
    /// in `profiles` must be in bounds for its table.
    pub fn validate_references(&self, profiles: &[ResourceRecord<Profile>])
        -> Result<(), SignalValidationError>;
}
```

Point validation (all in `try_new`):

- Histogram: increasing finite bounds, `bucket_counts.len() == bounds.len() + 1`,
  and the bucket sum equals `count`.
- Exponential histogram: `zero_count` plus the positive and negative bucket
  sums equals `count`; `scale` is in `-10..=20`.
- Summary: quantiles are in `[0, 1]` and strictly increasing.
- Sum: a monotonic sum is never negative.
- `start_time <= time`, and a delta interval is nonempty.

Errors are `SignalValidationError` (`#[non_exhaustive]`, `Box<ErrorContext>`,
code prefix `SC_OBSERVABILITY_TYPES_SIGNAL_`).

**uint policy.** OTLP `AnyValue` has only `int64`. `AnyValue::UInt(v)` with
`v <= i64::MAX` encodes as `intValue`. A larger `v` fails envelope validation
with `SubmissionError::ValueOutOfRange { path }` before admission. It is never
coerced to a double or a string.

**Non-finite policy.** Every proto `double` maps to `OtlpDouble` and accepts
NaN, `Infinity` and `-Infinity` (lead ruling P5). The only exceptions are the
data-model ordering rules above: histogram `explicit_bounds` must be strictly
increasing (so NaN is rejected there, while a final `Infinity` is allowed),
quantiles must be in `[0, 1]`, and `zero_threshold` must be non-negative.
Those return `SignalValidationError`, never a silent drop or coercion.

**String-index policy.** `AnyValue::StringIndex` and `AttributeKey::Index` are
kept as indices, never resolved to strings, so they round-trip unchanged.
Outside a profiles payload they fail envelope validation with
`SubmissionError::Validation { path }`. Inside one,
`ProfilesDictionary::validate_references` bounds-checks them against
`string_table`.

**bytes.** `AnyValue::Bytes` serializes as lowercase hex in the neutral JSON.
d-33 encodes it as base64 `bytesValue` in OTLP/JSON. Profile IDs, trace IDs
and span IDs use the existing hex newtypes.

### Submission contract (`sc_observability_types::otlp::submission`)

```rust
use super::signals; // sibling module in sc_observability_types::otlp

#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct EnvelopeVersion(u16);
impl EnvelopeVersion { pub const CURRENT: Self = Self(1); }

/// Caller-stable idempotency key (1..=256 bytes UTF-8); duplicate admission returns
/// the original receipt with `duplicate = true`.
#[non_exhaustive] pub struct RecordKey(String);
/// UUIDv7 text assigned at admission.
#[non_exhaustive] pub struct SubmissionId(String);

/// Front-end input: IDs optional, span `end_time` XOR `duration_nanos`.
#[non_exhaustive]
#[derive(Debug, Clone, Deserialize)]
pub struct SubmissionInput {
    pub version: EnvelopeVersion,
    pub record_key: Option<RecordKey>,
    pub resource: Option<signals::Resource>,
    pub scope: Option<signals::InstrumentationScope>,
    pub logs: Vec<LogInput>,
    pub spans: Vec<SpanInput>,
    pub metrics: Vec<signals::MetricStream>,
    pub profiles: Option<ProfilesInput>,
}

/// Canonical, validated, durable form.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SubmissionEnvelope {
    pub version: EnvelopeVersion,
    pub record_key: Option<RecordKey>,
    pub logs: Vec<signals::ResourceRecord<signals::LogPoint>>,
    pub spans: Vec<signals::ResourceRecord<signals::SpanPoint>>,
    pub metrics: Vec<signals::ResourceRecord<signals::MetricStream>>,
    pub profiles: Option<ProfilesSubmission>,
}
#[non_exhaustive]
pub struct ProfilesSubmission {
    pub dictionary: signals::ProfilesDictionary,
    pub profiles: Vec<signals::ResourceRecord<signals::Profile>>,
}

pub trait IdSource { fn trace_id(&mut self) -> TraceId; fn span_id(&mut self) -> SpanId; }

impl SubmissionEnvelope {
    /// Single shared validation/correlation path used by Python and the CLI.
    /// - Rejects version > CURRENT (UnsupportedVersion).
    /// - A paired log+span without IDs receives one generated trace/span ID;
    ///   supplied inconsistent IDs -> CorrelationConflict.
    /// - end_time and duration_nanos both present and disagreeing -> TimingConflict.
    /// - A log without actual start time never becomes a span.
    /// - At least one signal present, else EmptySubmission.
    pub fn from_input(input: SubmissionInput, ids: &mut dyn IdSource)
        -> Result<Self, SubmissionError>;
    pub fn from_json(json: &str, ids: &mut dyn IdSource) -> Result<Self, SubmissionError>;
    /// Deterministic canonical JSON (sorted keys, UTC RFC 3339 nanos) - the golden-fixture form.
    pub fn to_canonical_json(&self) -> String;
    pub fn signals(&self) -> SignalSet;
}

#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Signal { Logs, Traces, Metrics, Profiles }

#[non_exhaustive]
pub struct AdmissionReceipt {
    pub submission_id: SubmissionId,
    pub record_key: Option<RecordKey>,
    pub admitted_at: Timestamp,
    pub signals: Vec<Signal>,
    pub duplicate: bool,
}

#[non_exhaustive]
pub enum DeliveryState {
    Pending,
    Claimed { holder: String, attempts: u32 },
    RetryScheduled { attempts: u32, next_attempt_at: Timestamp, last_error: ErrorCode },
    Delivered { at: Timestamp, attempts: u32 },
    Failed { attempts: u32, error: ErrorCode },        // terminal backend rejection (e.g. 400)
    EvictedByDiskBound { at: Timestamp },               // only under DiskBoundPolicy::EvictOldest
}
#[non_exhaustive]
pub struct DeliveryStatus { pub submission_id: SubmissionId, pub signals: Vec<(Signal, DeliveryState)> }

#[non_exhaustive]
pub struct StoreStatus {
    pub schema_version: u32,
    pub store_bytes: u64,
    pub max_store_bytes: u64,
    pub pending: SignalCounts,
    pub retry_scheduled: SignalCounts,
    pub delivered_retained: SignalCounts,
    pub failed: SignalCounts,
    pub evicted_by_disk_bound: u64,
    pub rejected_by_disk_bound: u64,
    pub unreadable_newer_envelopes: u64,
    pub lease: Option<LeaseInfo>,
    pub submissions: Vec<DeliveryStatus>, // only for StatusQuery::Submissions
}
#[non_exhaustive] pub enum StatusQuery { Summary, Submissions(Vec<SubmissionId>), RecordKeys(Vec<RecordKey>) }
#[non_exhaustive]
pub struct FlushReport { pub delivered: SignalCounts, pub still_pending: SignalCounts, pub failed: SignalCounts }
```

Supporting types, all `#[non_exhaustive]`:

```rust
pub enum ExporterBackendId { SyncHttp, OpenTelemetrySdk }       // mirrors otlp's ExporterBackend
pub enum Representation { Log, Span, Gauge, Sum, Histogram, ExponentialHistogram, Summary, Exemplar, Profile }
pub struct SignalSet(/* bitset of Signal */);
pub struct SignalCounts { pub logs: u64, pub traces: u64, pub metrics: u64, pub profiles: u64 }
pub struct LeaseInfo { pub holder: String, pub expires_at: Timestamp }
pub struct Secret(String);                                       // redacted Debug/Display
pub struct ConfigOverrides { pub service_name: Option<String>, pub endpoint: Option<String>,
    pub auth_header: Option<Secret>, pub store_path: Option<PathBuf>, pub max_store_bytes: Option<u64>,
    pub disk_bound_policy: Option<DiskBoundPolicy>, pub flush_deadline: Option<Duration> }
pub struct SyncHttpRetryPolicyDto { pub max_retries: Option<u32>, pub initial_backoff_ms: Option<u64>,
    pub max_backoff_ms: Option<u64>, pub retry_sequence_timeout_ms: Option<u64> } // maps onto otlp SyncHttpRetryPolicy
/// Input forms: as LogPoint/SpanPoint but trace/span IDs optional, observed_time
/// optional (defaults to admission time), span end_time XOR duration_nanos, and an
/// optional per-record resource/scope overriding the envelope default.
pub struct LogInput { /* ... */ }
pub struct SpanInput { /* ... */ }
pub struct ProfilesInput { pub dictionary: signals::ProfilesDictionary,
    pub profiles: Vec<signals::Profile> }
```

### Errors and codes

Every variant carries `context: Box<ErrorContext>`, matching the d-12
pattern. The codes are new `ErrorCode::new_static` constants in
`sc_observability_types::otlp::submission::error_codes`, plus an enumerable registry.

```rust
#[non_exhaustive]
pub enum SubmissionError {
    InvalidJson { context },          // SC_OBSERVABILITY_SUBMIT_INVALID_JSON
    UnsupportedVersion { found: EnvelopeVersion, context }, // SC_OBSERVABILITY_SUBMIT_UNSUPPORTED_VERSION
    EmptySubmission { context },      // SC_OBSERVABILITY_SUBMIT_EMPTY
    Validation { path: String, context }, // SC_OBSERVABILITY_SUBMIT_VALIDATION
    ValueOutOfRange { path: String, context }, // SC_OBSERVABILITY_SUBMIT_VALUE_OUT_OF_RANGE
    CorrelationConflict { context },  // SC_OBSERVABILITY_SUBMIT_CORRELATION_CONFLICT
    TimingConflict { context },       // SC_OBSERVABILITY_SUBMIT_TIMING_CONFLICT
    DictionaryReference { path: String, context }, // SC_OBSERVABILITY_SUBMIT_DICTIONARY_REFERENCE
}
#[non_exhaustive]
pub enum AdmissionError {
    StoreUnavailable { context },     // SC_OBSERVABILITY_ADMIT_STORE_UNAVAILABLE
    DiskBoundExceeded { context },    // SC_OBSERVABILITY_ADMIT_DISK_BOUND
    Persistence { context },          // SC_OBSERVABILITY_ADMIT_PERSISTENCE
    SchemaTooNew { found: u32, supported: u32, context }, // SC_OBSERVABILITY_ADMIT_SCHEMA_TOO_NEW
    Closed { context },               // SC_OBSERVABILITY_ADMIT_CLOSED
}
#[non_exhaustive]
pub enum DeliveryError {
    DeadlineExceeded { report: FlushReport, context }, // SC_OBSERVABILITY_DELIVERY_DEADLINE
    TerminalFailure { report: FlushReport, context },  // SC_OBSERVABILITY_DELIVERY_FAILED
}
#[non_exhaustive]
pub enum TelemetryConfigError {
    ConfigFile { path: PathBuf, context },   // SC_OBSERVABILITY_TELEMETRY_CONFIG_FILE
    MissingField { field: &'static str, context }, // SC_OBSERVABILITY_TELEMETRY_CONFIG_MISSING
    InvalidField { field: &'static str, context }, // SC_OBSERVABILITY_TELEMETRY_CONFIG_INVALID
    UnsupportedCombination { backend: ExporterBackendId, signal: Signal,
        representation: Representation, context }, // SC_OBSERVABILITY_TELEMETRY_UNSUPPORTED
}
#[non_exhaustive]
pub enum TelemetryClientError {
    Submission(SubmissionError),
    Admission(AdmissionError),
    Delivery(DeliveryError),
    Config(TelemetryConfigError),
}
impl TelemetryClientError { pub fn code(&self) -> &ErrorCode; pub fn exit_code(&self) -> u8; }
```

### Configuration and precedence

```rust
#[non_exhaustive]
pub struct TelemetryClientConfig {
    pub service_name: String,
    pub endpoint: String,                  // OTLP/HTTP base, e.g. http://localhost:4318
    pub auth_header: Option<Secret>,       // never from checked-in YAML; never Debug-printed
    pub store_path: PathBuf,
    pub max_store_bytes: u64,              // default 256 MiB
    pub disk_bound_policy: DiskBoundPolicy, // default RejectNew
    pub delivered_retention: Duration,     // default 24 h
    pub record_key_retention: Duration,    // default 30 days
    pub emit_flush_deadline: Duration,     // default 5 s (CLI emit)
    pub flush_deadline: Duration,          // default 30 s
    pub lease_duration: Duration,          // default 30 s, renewed every 10 s
    pub sync_http_retry: Option<SyncHttpRetryPolicyDto>,
}
#[non_exhaustive] pub enum DiskBoundPolicy { RejectNew, EvictOldest }

#[non_exhaustive]
pub struct ConfigSources<'a> {
    pub explicit: &'a ConfigOverrides,         // caller args / CLI flags
    pub file: Option<&'a TelemetryFileConfig>, // parsed telemetry.yaml
    pub env: &'a dyn Fn(&str) -> Option<String>,
}
/// Precedence per field: explicit > telemetry.yaml > environment > built-in default.
/// Environment is read only for OTEL_EXPORTER_OTLP_ENDPOINT, OTEL_SERVICE_NAME and
/// SC_OTEL_AUTH_HEADER (auth only from env or explicit). A field required with no
/// default (store_path) yields MissingField. Relative YAML paths resolve against
/// the YAML file's directory.
pub fn resolve_config(sources: ConfigSources<'_>) -> Result<TelemetryClientConfig, TelemetryConfigError>;

/// Core keys of telemetry.yaml; serde `Deserialize`, unknown keys ignored.
/// `base_dir` is the YAML file's directory, set by the loader.
#[non_exhaustive]
pub struct TelemetryFileConfig {
    pub base_dir: PathBuf,
    pub service: Option<String>,
    pub otlp: Option<FileOtlp>,     // endpoint, timeout_ms
    pub store: Option<FileStore>,   // path, max_bytes, disk_bound_policy, delivered_retention_hours
}

// crates/sc-observability-otlp/src/durable/config_file.rs (feature durable-store)
pub fn load_telemetry_file(path: &Path) -> Result<TelemetryFileConfig, TelemetryConfigError>;
```

`sc-observability-types` has no YAML dependency. The YAML parser is
`serde-saphyr =1.3.0` (MIT OR Apache-2.0, MSRV 1.89), used only by
`load_telemetry_file` under `durable-store`. `serde_yaml` is deprecated
upstream. On `feat/qa-sanity-telemetry-config`, nothing parses
`.sc/telemetry.yaml` yet; the d-32 Python importer reads `sources[]` with the
repository's existing PyYAML (`yaml.safe_load`). Both the CLI `--config` flag
and Python `Telemetry(config=...)` go through `load_telemetry_file`, so the
Rust side needs its own parser (lead ruling P4).

The keys read from telemetry.yaml are `service`, `otlp.endpoint`,
`otlp.timeout_ms`, `store.path`, `store.max_bytes`, `store.disk_bound_policy`
and `store.delivered_retention_hours`. Every other key (`team`, `github.*`,
`sources[]`) belongs to the consumer and is ignored by the core without error.

### TelemetryClient

```rust
pub trait TelemetryClient: Send + Sync {
    fn open(config: TelemetryClientConfig) -> Result<Self, TelemetryClientError> where Self: Sized;
    /// Validates (already canonical), durably commits, then returns the receipt.
    fn emit(&self, envelope: SubmissionEnvelope) -> Result<AdmissionReceipt, TelemetryClientError>;
    /// Delivers rows admitted before the call, up to `deadline`.
    fn flush(&self, deadline: Duration) -> Result<FlushReport, TelemetryClientError>;
    /// Flush, stop the drain worker, release the lease. Idempotent.
    fn shutdown(&self, deadline: Duration) -> Result<FlushReport, TelemetryClientError>;
    fn status(&self, query: StatusQuery) -> Result<StoreStatus, TelemetryClientError>;
}
```

`InMemoryTelemetryClient` (`sc-observability-types` feature `test-double`) implements this trait with
the same receipt, duplicate-key and status semantics. It records envelopes
and has scripted delivery outcomes (`deliver_all`, `fail_next(Signal, ErrorCode)`).
It is the only double. `testing::conformance::run_all` holds the trait-level
cases. The double passes them in `tests/otlp_submission_contract.rs`, and d-33 runs the same
function against `DurableTelemetryClient`.

**Python GIL release.** These calls release the GIL: `emit` (durable commit),
`flush`, `shutdown`, `status` and `open` (store open and migration check).

### CLI contract (`sc-otel`)

Subcommands: `emit` (stdin JSON or `--log/--span/--metric/--profile` flags,
then a bounded flush), `validate` (prints the canonical envelope, no store),
`flush`, and `status`. Global flags: `--config <telemetry.yaml>`,
`--store <path>`, `--endpoint <url>` and `--output json|text`
(default `json`).

| Exit | Meaning | Source |
| --- | --- | --- |
| 0 | Success. For `emit`: admitted, and delivered within `emit_flush_deadline`, or admitted with `--no-flush`. | - |
| 1 | Unexpected internal error | panic guard |
| 2 | Usage error (bad flags) | argument parser |
| 3 | Invalid input | `SubmissionError::*` |
| 4 | Configuration error or unsupported combination | `TelemetryConfigError::*` |
| 5 | Admission failure (nothing admitted) | `AdmissionError::*` |
| 6 | Admitted but delivery not completed (pending remains) | `DeliveryError::DeadlineExceeded` |
| 7 | Admitted but a signal failed terminally | `DeliveryError::TerminalFailure` |

JSON output schema (`sc-otel.result/v1`), one object on stdout:

```json
{
  "schema": "sc-otel.result/v1",
  "command": "emit",
  "exit_code": 6,
  "state": "admitted_pending",
  "receipt": {"submission_id": "0192...", "record_key": "sanity:run-1:sanity-llm",
              "admitted_at": "2026-10-01T12:00:00.000000000Z", "signals": ["logs","traces"], "duplicate": false},
  "flush": {"delivered": {"logs": 1}, "still_pending": {"traces": 1}, "failed": {}},
  "status": null,
  "envelope": null,
  "error": {"code": "SC_OBSERVABILITY_DELIVERY_DEADLINE", "message": "..."}
}
```

`state` is one of `validated`, `admitted_delivered`, `admitted_pending`,
`admitted_failed`, `rejected` or `status`. Credentials never appear in output
or errors.

### Store schema and versioning (`durable/schema.sql`)

```sql
PRAGMA user_version = 1;
CREATE TABLE store_meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);
CREATE TABLE submissions (
  submission_id TEXT PRIMARY KEY,
  record_key TEXT UNIQUE,
  envelope_version INTEGER NOT NULL,
  envelope BLOB NOT NULL,              -- SubmissionEnvelope::to_canonical_json
  envelope_bytes INTEGER NOT NULL,
  admitted_at_unix_nano INTEGER NOT NULL
);
CREATE TABLE signal_deliveries (
  submission_id TEXT NOT NULL REFERENCES submissions(submission_id) ON DELETE CASCADE,
  signal TEXT NOT NULL CHECK (signal IN ('logs','traces','metrics','profiles')),
  state TEXT NOT NULL CHECK (state IN ('pending','claimed','retry','delivered','failed','evicted')),
  attempts INTEGER NOT NULL DEFAULT 0,
  claimed_by TEXT,
  claim_expires_at_unix_nano INTEGER,
  next_attempt_at_unix_nano INTEGER,
  last_error_code TEXT,
  delivered_at_unix_nano INTEGER,
  PRIMARY KEY (submission_id, signal)
);
CREATE INDEX signal_deliveries_ready ON signal_deliveries (signal, state, next_attempt_at_unix_nano);
CREATE TABLE record_keys (record_key TEXT PRIMARY KEY, submission_id TEXT NOT NULL,
  admitted_at_unix_nano INTEGER NOT NULL);
CREATE TABLE drain_lease (id INTEGER PRIMARY KEY CHECK (id = 1), holder TEXT NOT NULL,
  acquired_at_unix_nano INTEGER NOT NULL, expires_at_unix_nano INTEGER NOT NULL);
CREATE TABLE store_counters (name TEXT PRIMARY KEY, value INTEGER NOT NULL);
```

Versioning policy:

- The schema version is `PRAGMA user_version`. Opening a store with a newer
  version returns `AdmissionError::SchemaTooNew` (reject-newer). The store is
  never modified in that case.
- An older version migrates forward inside one transaction. v1 has no
  predecessor.
- A row whose `envelope_version` is newer than `EnvelopeVersion::CURRENT` is
  never deleted. It is skipped and counted in `unreadable_newer_envelopes`.
- Connections use `journal_mode=WAL`, `synchronous=FULL` and
  `busy_timeout=5000`.

### Platform matrix and dependency audit

`rusqlite =0.40.2` (`bundled`, default features off; it builds
`libsqlite3-sys 0.38.2` from source), behind `durable-store`. The audit
config `policy/deny-durable-store.toml` exists because the repository has no
cargo-deny config, and this is the first bundled C dependency in a released
crate. It allows the licenses in the `durable-store` graph (SQLite is public
domain; rusqlite and libsqlite3-sys are MIT), denies yanked crates and
advisories, and bans duplicate `libsqlite3-sys`. The same graph includes
`serde-saphyr =1.3.0` (MIT OR Apache-2.0), the `.sc/telemetry.yaml` parser
(lead ruling P4). Its consumers are this
sprint's audit criterion and the d-32 re-run. It is retired once a
workspace-wide cargo-deny config covers the same graph.

Platforms: linux x86_64 and aarch64, macOS x86_64 and arm64, windows x86_64
and arm64 (the d-10 wheel target). The CLI builds with `cargo build`; the
abi3-py310 wheels build with the `otlp-telemetry` feature. d-29 proves the
contract by compiling (`cargo check --features durable-store`) on the CI
matrix. d-30 proves the wheel matrix.

## Acceptance criteria

- [ ] boundary:BOUNDARY-ScObservabilityTypes (D1, D2):
  `cargo test -p sc-observability-types --test otlp_signals_contract --locked`
  runs nonzero cases for: every `AnyValue` variant's serde round trip,
  including bytes, `uint` and `StringIndex`; `OtlpDouble` round trips of NaN,
  `Infinity` and `-Infinity` through serde JSON (asserting the proto-JSON
  strings) in `AnyValue`, `NumberValue`, histogram `sum`/`min`/`max`, summary
  values and exemplars; NaN rejected in `explicit_bounds`; each of the five metric point forms with
  temporality and monotonicity validation; exemplars;
  `ProfilesDictionary::validate_references` accepting a valid set and rejecting
  an out-of-range index, including an out-of-range `StringIndex` and
  `AttributeKey::Index`; `TraceState` grammar; and `From` conversions from
  `LogEvent`, `SpanRecord<SpanEnded>`, v2 `MetricRecord`, `OtlpResource` and
  `OtlpInstrumentationScope`.
- [ ] boundary:BOUNDARY-ScObservabilityTypes (D3, D4):
  `cargo test -p sc-observability-types --features test-double --locked --test otlp_submission_contract`
  passes every golden fixture. Each `input.json` canonicalizes to its
  `expected.envelope.json`, or fails with the code in `expected.error.json`.
  Fixtures include `uint_over_i64_max`, `correlation_conflict`,
  `timing_conflict`, `unsupported_version`, `strindex_outside_profiles`,
  `non_finite_doubles`, `paired_log_span_generated_ids`
  (placeholder `$GENERATED_TRACE_ID` equality) and one fixture per signal and
  point form.
- [ ] boundary:BOUNDARY-ScObservabilityTypes (D3): the precedence tests
  show, per field, explicit > YAML > env > default; auth only from explicit
  input or env; `store_path` missing gives `MissingField`; and the error-code
  registry is unique.
- [ ] boundary:BOUNDARY-ScObservabilityTypes (D5):
  `cargo test -p sc-observability-otlp --features durable-store --locked --lib durable::config_file`
  loads the committed `.sc/telemetry.yaml`. Its unit tests show that unknown
  consumer keys (`team`, `github.*`, `sources[]`) are ignored, that a relative
  `store.path` resolves against the file's directory, and that malformed YAML
  returns `TelemetryConfigError`.
- [ ] boundary:BOUNDARY-ScObservabilityTypes (D4): the test-double
  conformance cases cover duplicate record key → original receipt with
  `duplicate = true`, a `fail_next` scripted outcome reported in `status`, and
  idempotent `shutdown`.
- [ ] boundary:BOUNDARY-ScObservabilityTypes (D5, D6):
  `cargo check --workspace --all-features --locked`,
  `cargo check -p sc-observability-otlp --features durable-store --locked`,
  `cargo check -p sc-observability-py --features otlp-telemetry --locked` and
  `cargo check -p sc-otel-cli --locked` pass. The `durable/` and CLI stubs
  contain no `todo!` or `unimplemented!`. `schema.sql` loads into an empty
  SQLite database in the contract test.
- [ ] boundary:BOUNDARY-ScObservabilityTypes (D6):
  the scoped cargo-deny audit in Required validation passes for the
  `durable-store` graph. `bash scripts/ci/validate_repo_boundaries.sh`
  and `bash scripts/ci/validate_dependency_bans.sh` pass with the new and
  edited manifests. `cargo tree -p sc-otel-cli -e normal` contains no
  `sc-observe`, `pyo3` or `tokio` runtime feature `rt-multi-thread`.
- [ ] boundary:ADR-021 (D7): ADR-021 is Accepted, and PHD-005, PHD-010 and
  ADR-021 Signals name profiles. `bash scripts/ci/validate_docs_consistency.sh`
  passes.

## Required validation

```sh
cargo fmt --check --all
cargo clippy --all-targets --all-features -- -D warnings
cargo check --workspace --all-features --locked
cargo test -p sc-observability-types --test otlp_signals_contract --locked
cargo test -p sc-observability-types --features test-double --locked --test otlp_submission_contract
cargo test -p sc-observability-otlp --features durable-store --locked --lib durable::config_file
cargo deny --manifest-path crates/sc-observability-otlp/Cargo.toml --features durable-store check --config policy/deny-durable-store.toml licenses bans advisories
bash scripts/ci/validate_repo_boundaries.sh
bash scripts/ci/validate_dependency_bans.sh
bash scripts/ci/validate_docs_consistency.sh
```

## Closeout gate

The user signs `docs/api-approvals/phase-d-wave5-telemetry-submission.json`
(D9) before d-29 closes (lead ruling P6). This is a d-29 closeout gate, not a
plan blocker: d-33, d-30 and d-31 may start from the sanity-passed contract
while the signature is pending. d-32 re-runs the D18 gate against the signed
record.

## Handoffs

- To d-33 (wave 5.2): `crates/sc-observability-otlp/src/durable/mod.rs`,
  staged by d-29 and owned by d-33 from wave 5.2. `schema.sql` stays
  read-only; a change to it is a contract change routed to the lead.
- To d-31 (wave 5.2): `crates/sc-otel-cli/src/main.rs`, staged by d-29 and
  owned by d-31. `crates/sc-otel-cli/Cargo.toml` stays d-29's.
- Wave-5.2 sprints add no dependency and do not edit `Cargo.lock`. A needed
  dependency is a contract defect, routed to the lead.
