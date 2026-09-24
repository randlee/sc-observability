---
id: D.7
status: proposed
branch: feature/phase-d-7-otlp-export-restore
base: develop
release_train: '2.0'
---

# D.7 — Restore real OTLP export through SDK and synchronous paths

## Goal and dependency

Restore a regressed real collector pipeline in `sc-observability-otlp` through
**both** implementation paths. The official SDK path is required for
Tokio-hosted consumers; the synchronous HTTP/JSON path is required for
compatibility with the real pre-split exporter and for environments that do not
host Tokio. `atm-core` is a downstream consumer: no `atm-core` code, fixture,
worktree, or PR is owned by this sprint.

The pre-split `agent-team-mail` source was a 920-line `reqwest::blocking`
HTTP/JSON exporter that posted to `/v1/logs`, `/v1/traces`, and `/v1/metrics`,
handled auth headers, CA bundles, timeouts and bounded retry, and supported
Grafana/LogQL dashboards. The current crate has no HTTP transport dependency
and constructs three `Noop*Exporter`s. This is a regression restoration, not
a deferred v1 scope decision.

D.7 `must_follow`s D.4 and belongs to the 2.0 train: spec-correct signals need
breaking type/migration changes. The scratchpad legacy clone is evidence-only;
extract behavior/tests now but retain no dependency on its path or ATM-specific
data schema.

## Why both paths are in scope

### A. Official OpenTelemetry SDK path — required primary path

Adopt `opentelemetry-otlp`, `opentelemetry`, and `opentelemetry_sdk` for
Tokio-hosted consumers.

- The SDK supplies native models for `SpanKind`, trace sampling, links, and
  proper histogram points instead of recreating all OTLP semantics by hand.
- Its default gRPC transport through `tonic` requires Tokio. The larger
  footprint—protobuf generation, tonic and Tokio—and reconciliation with SDK
  provider/batch-processor lifecycle remain real implementation work.
- The existing synchronous `Telemetry::emit_log`/`emit_span`/`emit_metric`,
  buffering, health and flush/shutdown API must remain the public façade. D.7
  specifies an explicit adapter/ownership model rather than leaking SDK
  providers into callers or allowing two independent batch owners.

### B. Restored synchronous HTTP/JSON path — required companion path

Port the legacy `reqwest::blocking` exporter into the current neutral model.

- It is a proven route to `/v1/logs`, `/v1/traces`, and `/v1/metrics`, keeps a
  synchronous `reqwest` footprint, and fits directly below today's facade.
- It requires manual source-model work for kind, sampling, links and real
  histogram data. The legacy implementation has no historical ADR or QA record
  justifying its deviation: its own old plan anticipated the official SDK, but
  the hand-rolled implementation went unflagged across four QA review passes.
- It must not become a weaker, untested fallback; the same public signal and
  failure contracts apply to both paths.

## Deliverables

1. **Spec-correct 2.0 signal contract.** Extend shared neutral types/projectors
   with a typed `SpanKind`, sampled trace flags, typed span links, and histogram
   points containing explicit bounds, bucket counts, count and sum. Define
   serde, validation, conversions, and migration dispositions. Preserve valid
   counter/gauge behavior; reject malformed histogram invariants before either
   exporter sees them.
2. **One public façade, explicit exporter mode.** Add a typed transport/mode
   selection to `OtelConfig` that selects the SDK-backed path or synchronous
   HTTP/JSON path without exposing provider ownership to callers. Valid enabled
   configuration plus an enabled signal must install a real selected exporter;
   disabled telemetry alone may be no-op. No enabled configuration may silently
   retain a no-op exporter. Unsupported protocol/mode combinations fail
   construction with a stable typed error.
3. **SDK implementation for Tokio hosts.** Integrate `opentelemetry`,
   `opentelemetry_sdk`, and `opentelemetry-otlp` with required features locked
   in the workspace. Map the public façade's batches into the selected SDK
   providers/exporters, use the host's Tokio runtime without creating an
   uncontrolled second runtime, and define flush/shutdown ownership so SDK
   processor flush occurs exactly once. Support the SDK-selected OTLP protocol
   and collector endpoint with an in-repository Tokio-hosted consumer fixture.
4. **Synchronous HTTP/JSON implementation.** Port the legacy behavior using
   `reqwest::blocking`, adapted to current `TelemetryConfig.service_name`,
   resources and neutral signals. Normalize endpoint suffixes exactly once;
   honor `AuthHeader`, CA file, insecure TLS flag, timeout and bounded retry/
   exponential backoff. Do not log secrets or hold the telemetry runtime lock
   during blocking network/retry work.
5. **Cross-path conformance suite.** Run both modes against hermetic loopback
   collectors. Validate request method/path/content type/authorization,
   resource/scope metadata, log severity/body/attributes, trace parent/status/
   kind/flags/links/events, and counter/gauge/histogram data. Assert equivalent
   observable signal semantics from a shared fixture corpus while allowing the
   expected protocol/wire transport differences.
6. **Lifecycle and failure parity.** For both modes test disabled no-network,
   invalid config, unsupported selection, transient and terminal collector
   failures, bounded retries, custom CA, timeout, health state, dropped-export
   counts, fail-open flush, and idempotent shutdown. SDK async work must not
   block a Tokio worker; synchronous work must remain outside async executor
   critical paths or use a documented bridge that preserves caller semantics.
7. **Consumer-neutral evidence and observability docs.** Add an in-repository
   Tokio-hosted consumer fixture exercising the SDK path. Restore Grafana
   dashboard/LogQL recipes only after translating them to current neutral
   resource/attribute schema; never revive ATM-only labels as a compatibility
   fiction. Include runnable local collector smoke commands and redacted
   receipts for both paths. A downstream `atm-core` integration is optional
   post-merge consumer evidence, never a deliverable or gate owned here.
8. **Contracts and migration.** Update OTLP-001–022, architecture, public API
   inventory, 2.0 release notes, migration guide, and dependency/license
   inventory. Document both modes, their intended runtime environments,
   protocol support, chosen owner model, no-enabled-noop rule, and all source
   model changes.

## Acceptance criteria

- The public `Telemetry` facade sends logs, traces and metrics to real
  loopback collectors through **both** SDK/Tokio and synchronous HTTP/JSON
  modes. The SDK proof uses an in-repository Tokio-hosted consumer fixture.
- A shared corpus yields equivalent resource/signal semantics in both modes:
  trace kind/sampled flags/parent/links/events/status and histogram bounds,
  bucket counts, count and sum are all present and correct.
- Neither mode creates a silent no-op for enabled telemetry; unsupported
  protocol/mode combinations fail at construction with a stable typed error.
- SDK batching/provider shutdown and synchronous retry/flush each occur once
  under the facade's documented ownership rules; neither blocks or deadlocks
  the host runtime, and failure health/dropped accounting is consistent.
- No credentials appear in `Debug`, errors, docs or retained evidence. Every
  dashboard recipe/current contract refers only to current neutral schema.
- The 2.0 migration and API approval explicitly cover the new signal models,
  added dependencies, dual exporter modes, and consumer impact.

## Required validation

- `cargo test -p sc-observability-types -p sc-observability-otlp --locked`
  with source-model negative cases, dual-mode loopback collectors, retry/CA/
  auth-redaction and lifecycle cases.
- Tokio-hosted in-repository SDK fixture; synchronous external-consumer-style
  fixture with no Tokio runtime; shared cross-path conformance corpus.
- `cargo test --workspace --locked`, clippy with warnings denied, rustdoc,
  public-API/semver validation against the declared 1.x baseline, dependency/
  license inventory validation, and docs consistency.
- Retain exact source SHA and redacted local collector receipts for all three
  signals in both modes.

## Non-closure

No Python OTEL binding work (#88), registry publication, or silent removal of
either supported path. Additional protocols beyond those explicitly selected
and tested by the two implementations require a later authorized sprint.
