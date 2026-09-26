# d-5: OTLP 2.0 signal model

## Plan metadata

- Wave: 8
- Branch: `sprint/d-5-otlp-signal-model`
- PR target: `sprint/d-4-error-enums-2-0`
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

1. Migrate span assembly/projectors so start/event/end processing preserves
   kind, flags, links, status, timing, attributes, and diagnostics without
   creating OTLP transport dependencies in lower crates.

2. Migrate metric projectors and fixtures to `MetricValue`; enforce histogram
   invariants: `bucket_counts.len() == explicit_bounds.len() + 1`, finite
   ordered bounds, `sum(bucket_counts) == count`, finite sum, and no invalid
   negative count representation.
   Preserve aggregation temporality and data-point start time for sums and
   histograms; `Delta` requires an explicit start time no later than the point
   timestamp, while `Cumulative` permits `Timestamp::UNIX_EPOCH` or an earlier
   explicit start. Reject inconsistent intervals.

3. Record the breaking 1.x-to-2.0 source/serde migration in
   `docs/migration.md`, the public API approval, and the OTLP-020/OTLP-021
   requirements changes.

4. Migrate consumers in `sc-observability-types`,
   `sc-observability-dto` (`TraceContextDto` included), `sc-observe`,
   `sc-observability`, `sc-observability-otlp`, binding runtime, generated
   Python/TypeScript models, examples, and public fixtures.

5. Add `InvalidHistogram`, invalid temporality, and invalid interval failures
   to the D.5-owned `MetricModelError` rows of the central error inventory
   with stable codes and remediation. D.6 owns only transport, lifecycle, and
   configuration rows.


## Non-closure

No network exporter, SDK dependency, collector smoke test, or dashboard work.


## Design



## Implementation targets

 implement or update the named contract consumer and its focused test for the corresponding numbered deliverable.\n

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


