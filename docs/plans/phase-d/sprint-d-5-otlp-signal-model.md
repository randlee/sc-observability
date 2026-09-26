# d-5: OTLP 2.0 signal model

## Plan metadata

- Wave: 2
- Branch: `sprint/d-5-otlp-signal-model`
- PR target: `sprint/d-12-c-types`
- Blocked by: `obs-d-12-sanity`
- Owned paths:
  - `crates/sc-observability-otlp/src/projectors.rs`
  - `crates/sc-observability-otlp/tests/error_registry_parity.rs`

## Goal and dependency

Define the spec-correct neutral signal model that both real exporters consume.
It must follow D.4: D.4 owns the 2.0 version bump, break approval, and
canonical errors used here. D.6 and D.8 may not invent transport-local
substitutes.


## Deliverables

1. Add the public types, constructors/accessors, serde contract, validation
   errors, rustdoc, and re-exports required by the contract above.
2. Migrate span assembly/projectors so start/event/end processing preserves
   kind, flags, links, status, timing, attributes, and diagnostics without
   creating OTLP transport dependencies in lower crates.
3. Migrate metric projectors and fixtures to `MetricValue`; enforce histogram
   invariants: `bucket_counts.len() == explicit_bounds.len() + 1`, finite
   ordered bounds, `sum(bucket_counts) == count`, finite sum, and no invalid
   negative count representation.
   Preserve aggregation temporality and data-point start time for sums and
   histograms; `Delta` requires an explicit start time no later than the point
   timestamp, while `Cumulative` permits `Timestamp::UNIX_EPOCH` or an earlier
   explicit start. Reject inconsistent intervals.
4. Record the breaking 1.x-to-2.0 source/serde migration in
   `docs/migration.md`, the public API approval, and the OTLP-020/OTLP-021
   requirements changes.
5. Migrate consumers in `sc-observability-types`,
   `sc-observability-dto` (`TraceContextDto` included), `sc-observe`,
   `sc-observability`, `sc-observability-otlp`, binding runtime, generated
   Python/TypeScript models, examples, and public fixtures.
6. Add `InvalidHistogram`, invalid temporality, and invalid interval failures
   to the D.5-owned `MetricModelError` rows of the central error inventory
   with stable codes and remediation. D.6 owns only transport, lifecycle, and
   configuration rows.


## Non-closure

No network exporter, SDK dependency, collector smoke test, or dashboard work.


## Design

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


## Owned Paths and Exact Targets

- `crates/**`
- `bindings/**`
- `examples/**`
- `release/public-api-policy.json`
- `scripts/ci/fixtures/**`
- `docs/api-approvals/d-5-*.json`
- `docs/requirements.md`
- `docs/architecture.md`
- `docs/api-design.md`
- `docs/migration.md`

These are edit fences for the deliverables above, including their tests and
public API approval where listed; reading dependencies does not claim ownership.
New modules stay inside the listed crate fences. No unrelated changes are authorized.



## Acceptance criteria

## Acceptance criteria

- All known call sites use the new types; no production histogram is represented
  by a single scalar or synthesized one-bucket placeholder.
- Span assembly round-trips kind, sampled state, links, events, parent, status,
  and timing through start/end completion.
- Valid zero/one/many-bucket histograms round-trip; every malformed invariant
  above returns a stable typed failure before export.
- `sc-observability-types` and projectors remain transport/SDK independent.
- API approval and migration docs identify every intentional 2.0 break.


## Required validation

- Focused type serde/negative tests and span-assembly/projector tests.
- `cargo test -p sc-observability-types -p sc-observability-dto -p sc-observe -p sc-observability-binding-runtime -p sc-observability-otlp --locked`.
- Workspace clippy/rustdoc and the reviewed 1.4.1-to-2.0 public API
  comparison/rebaseline mechanism.


