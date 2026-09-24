---
id: D.7
status: proposed
branch: feature/phase-d-7-otlp-http-json-restore
base: develop
release_train: '2.0'
---

# D.7 — Restore real HTTP/JSON OTLP export

## Goal and dependency

Restore a regressed, working capability from the pre-split `agent-team-mail`
repository into this repository's `sc-observability-otlp` crate. The source
was a 920-line hand-built HTTP/JSON exporter using `reqwest::blocking`, not the
official `opentelemetry` crate: it posted real payloads to `/v1/logs`,
`/v1/traces`, and `/v1/metrics`; handled authorization headers, CA bundles,
timeouts, and bounded retries; and powered Grafana/LogQL dashboard workflows.
The current crate has no HTTP transport dependency and wires
`Telemetry::new_typed` to `NoopLogExporter`, `NoopTraceExporter`, and
`NoopMetricExporter`, making this a restoration, not a deferred v1 choice.

D.7 `must_follow`s D.4 and is part of its 2.0 train because the OTLP-correct
signal models require breaking public type changes. The scratchpad legacy clone
is evidence-only: extract behavior and tests during implementation, but do not
retain a dependency on its path or copy ATM-specific types/attributes.

## Deliverables

1. **Spec-correct source models first.** Extend the current neutral shared
   types and projectors so a completed span contains a typed `SpanKind`, trace
   flags including sampled state, and typed links; extend metrics so histogram
   points carry explicit bounds, bucket counts, count, and sum rather than
   encoding a single `f64` as a synthetic one-bucket histogram. Define serde,
   validation, conversion, and 2.0 migration dispositions for every changed
   type. Preserve non-histogram counter/gauge behavior and reject malformed
   histogram invariants before export.
2. **Concrete exporter selection.** Add a private real HTTP/JSON exporter
   using `reqwest::blocking` and instantiate it when transport is enabled with
   `OtlpProtocol::HttpJson`, a valid endpoint, and at least one enabled signal.
   Its endpoint normalization must append/select exactly `/v1/logs`,
   `/v1/traces`, and `/v1/metrics` without duplicate suffixes. Disabled
   telemetry may remain no-op; enabled telemetry must never silently use a
   no-op exporter. `HttpBinary` and `Grpc` are not implemented by this restore:
   construction must return a typed unsupported-protocol failure when enabled,
   rather than claim functional transport.
3. **Transport parity adapted to current `OtelConfig`.** Honor the current
   typed endpoint, `AuthHeader`, CA file, insecure TLS flag, timeout,
   `max_retries`, and initial/max backoff. Parse a validated header name/value
   without logging credentials. Retry only bounded transient transport/HTTP
   failures, preserve fail-open `Telemetry` health and dropped-export accounting
   after exhaustion, and do not sleep/hold the telemetry runtime lock while
   performing network I/O.
4. **OTLP/HTTP JSON encoding.** Encode resource attributes, scope metadata,
   log severity/body/attributes, trace parent/status/kind/flags/links/events,
   and counter/gauge/histogram points into collector-valid JSON. Use the
   current `TelemetryConfig.service_name`/resource and neutral records rather
   than legacy ATM fields. Document exact timestamp, integer-as-string, and
   attribute-value conversion rules. A missing required OTLP field is a test
   failure, not a permissive best effort.
5. **Real collector evidence and docs.** Add hermetic loopback collector
   integration tests that capture and validate HTTP method/path, content type,
   authorization, each signal's JSON envelope, retry behavior, CA-bundle
   handling, failure health, and no-network disabled behavior. Restore
   repository-owned Grafana dashboard/LogQL recipes only after rewriting them
   to the current neutral resource/attribute schema; do not revive ATM-specific
   labels as a compatibility fiction. Include a runnable local collector smoke
   recipe and a redacted evidence record.
6. **Contracts and migration.** Update OTLP-001–022, architecture, public API
   inventory, 2.0 release notes, and migration guide to say that HTTP/JSON is
   implemented, `HttpBinary`/`Grpc` are explicitly unsupported, no exporter
   is silently no-op when enabled, and the signal model has changed. Add public
   consumer and wire-fixture coverage for the new models and exporter behavior.

## Acceptance criteria

- With `enabled=true`, `HttpJson`, a valid endpoint, and configured signals,
  `Telemetry::new_typed` installs real exporters and a loopback collector
  receives valid POSTs to all three `/v1/*` endpoints; this is demonstrated
  through the public runtime/projector path, not direct private exporter calls.
- Disabled telemetry performs no HTTP request; enabled `HttpBinary`/`Grpc`
  fail construction with a stable typed error and are never represented as
  successful no-op export.
- Captured trace JSON contains kind, sampled trace flags, parent/link/event
  data and status; captured histogram JSON contains bounds, counts, count, and
  sum from a valid source point. Counter/gauge semantics remain correct.
- Authorization is sent but never appears in debug/error/docs evidence; custom
  CA, timeout, bounded retry/backoff, transient failure exhaustion, health,
  dropped count, flush, and idempotent shutdown all have direct fixtures.
- The 2.0 model/migration/API docs and restored dashboard recipes use current
  service/resource schema and contain no stale ATM-only fields or scratchpad
  path dependency.

## Required validation

- `cargo test -p sc-observability-types -p sc-observability-otlp --locked`,
  including model negative cases, loopback collector integration fixtures, and
  retry/CA/auth redaction cases.
- `cargo test --workspace --locked`, clippy with warnings denied, rustdoc,
  public-API/semver validation against the declared 1.x baseline, and docs
  consistency.
- A local collector smoke command that emits logs/traces/metrics through
  `TelemetryProjectors`, captures all three JSON requests, validates them, and
  writes only redacted evidence; retain exact command and source SHA.

## Non-closure

Do not add the official `opentelemetry` SDK merely to claim parity, implement
gRPC or HTTP/protobuf, add Python OTEL binding work (#88), or publish a
registry release. Those protocols and #88 need separately authorized sprints.
