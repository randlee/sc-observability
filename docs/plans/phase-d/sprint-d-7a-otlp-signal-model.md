---
id: D.7a
status: complete
branch: feature/phase-d-7a-otlp-signal-model
base: develop
worktree: /Users/randlee/github/sc-observability-worktrees/feature/phase-d-7a-otlp-signal-model
depends_on: [D.4]
relation: must_follow
owned_docs: [docs/requirements.md, docs/architecture.md, docs/api-design.md]
release_train: '2.0'
recommended_agent: rust-developer
recommended_model: deep-reasoning
---

# D.7a — OTLP 2.0 signal model

## Goal and dependency

Define the spec-correct neutral signal model that both real exporters consume.
D.7a `must_follow`s D.4 because these public model changes ship only in the
accepted 2.0 train. D.7b and D.7c may not invent transport-local substitutes.

## Public contract

The implementation may refine names during API review, but it must preserve
this discriminated shape and information content:

```rust
pub enum SpanKind { Internal, Server, Client, Producer, Consumer }

pub struct TraceFlags(u8); // exposes sampled() and preserves known W3C bits

pub struct SpanLink {
    pub trace_id: TraceId,
    pub span_id: SpanId,
    pub flags: TraceFlags,
    pub attributes: serde_json::Map<String, serde_json::Value>,
}

pub enum AggregationTemporality { Delta, Cumulative }

pub enum MetricValue {
    Gauge(f64),
    Sum {
        value: f64,
        monotonic: bool,
        temporality: AggregationTemporality,
        start_time: SystemTime,
    },
    Histogram {
        point: HistogramPoint,
        temporality: AggregationTemporality,
        start_time: SystemTime,
    },
}

pub struct HistogramPoint {
    explicit_bounds: Vec<f64>,
    bucket_counts: Vec<u64>,
    count: u64,
    sum: f64,
}

impl HistogramPoint {
    pub fn try_new(/* fields above */) -> Result<Self, MetricModelError>;
    // read-only accessors
}
```

`SpanRecord` carries `SpanKind` and links; `TraceContext` carries trace flags.
`MetricRecord` carries `MetricValue` rather than the current `MetricKind` plus
single `f64` combination. Exact serde names and constructors are frozen in the
2.0 API approval before implementation completion.
`HistogramPoint` deserializes through a validated `TryFrom` representation so
serde cannot construct an invalid value. A link contains its own ids/flags and
never embeds `TraceContext`, eliminating two sources of truth.

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
   histograms; reject inconsistent interval/time combinations.
4. Record the breaking 1.x-to-2.0 source/serde migration, public API approval,
   requirements changes, and exhaustive inventory of construction/match sites.
5. Inventory and migrate every consumer in `sc-observability-types`,
   `sc-observability-dto` (`TraceContextDto` included), `sc-observe`,
   `sc-observability`, `sc-observability-otlp`, binding runtime, generated
   Python/TypeScript models, examples, and public fixtures.
6. Add `InvalidHistogram`, invalid temporality, and invalid interval failures
   to the central error inventory with stable codes and remediation.

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
- Workspace clippy/rustdoc and the D.4 controlled 1.4.1-to-2.0 public API
  comparison/rebaseline mechanism.
- An inventory gate proves every old `MetricRecord.value` construction and
  match has a recorded migration disposition.

## Non-closure

No network exporter, SDK dependency, collector smoke test, or dashboard work.
