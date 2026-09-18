---
id: B.1d
status: complete
branch: feature/phase-b-1d-telemetry-prep
base: feature/phase-b-1-copy
worktree: /Users/randlee/github/sc-observability-worktrees/feature/phase-b-1d-telemetry-prep
---

# B.1d — Typed telemetry configuration and lifecycle failures

## Goal and dependencies

`must_follow` B.1c for observation/projector integration and B.1a for neutral
failure values. B.1e `must_follow` this sprint: warnings begin only after every
replacement runtime path is implemented and validated.

For every `must_follow`, merge pushed parent development into the child before
every development/fix round; the parent PR merges before child completion.
No listed related sprint is `parallel_safe`: shared neutral contracts, runtime
call sites or release artifacts intersect.

## Deliverables (authoritative)

Every listed deliverable must land production-ready for this sprint's stated
scope. Completion requires evidence for every numbered item, including its code,
documentation and validation artifacts; partial completion leaves the sprint open.

1. Implement the typed methods below over the existing configuration, span
   assembly, exporter and lifecycle implementation. Convert internal exporter
   failures to ExportFailure and retain compatibility at public legacy boundaries.
   Existing exporter traits are crate-private; do not invent public exporter APIs.
2. Migrate built-in telemetry projectors to the B.1a typed contracts with existing
   trait interoperability. Keep TelemetryError and emit method signatures intact.
3. Add the telemetry inventory and old/new parity fixtures covering all signal
   families and lifecycle/error cases below.

## Contract

```rust
impl OtlpEndpoint {
    pub fn new_typed(value: impl Into<String>) -> Result<Self, InitFailure>;
}
impl AuthHeader {
    pub fn new_typed(value: impl Into<String>) -> Result<Self, InitFailure>;
}
impl TelemetryConfigBuilder {
    pub fn build_typed(self) -> Result<TelemetryConfig, InitFailure>;
}
impl SpanAssembler {
    pub fn push_typed(&mut self, signal: SpanSignal)
        -> Result<Option<CompleteSpan>, EventFailure>;
}
impl Telemetry {
    pub fn new_typed(config: TelemetryConfig) -> Result<Self, InitFailure>;
    pub fn flush_typed(&self) -> Result<(), FlushFailure>;
    pub fn shutdown_typed(&self) -> Result<(), ShutdownFailure>;
}
```

Keep emitted TelemetryError variants, config fields, endpoint/header encoding,
health fields and existing emit methods unchanged. Internal log/trace/metric
exporter methods retain inputs and success values, replacing only their private
ExportError with ExportFailure. Preserve metadata/source when mapping an exporter
failure to flush/shutdown/projection context. The existing repeated-shutdown and
incomplete-span policies remain authoritative. In particular flush_typed keeps
the existing fail-open exporter behavior (success plus health reporting), while
the first shutdown reports final export failure. Do not introduce new error
returns into old methods. This is not an exporter rebuild.

## Acceptance criteria (authoritative)

- AC1: All listed recommended methods use typed failures with exact parity to
  legacy behavior and preserve the source data retained by the original
  operation; conversion does not manufacture lost native sources.
- AC2: Log, trace and metric exporter/route failures use typed production values;
  both existing projector interfaces and typed adapters remain usable.
- AC3: Old serialized errors/config values and public enum exhaustiveness remain
  unchanged. No legacy API is removed or silently assigned different semantics.

## Required validation (authoritative)

Run formatting, `cargo test --locked -p sc-observability-otlp --all-targets`,
workspace clippy/doctests and public API diff/semver/docs checks. Paired fixtures
cover empty/invalid endpoint and auth header; protocol/config mismatch; zero
export timeout/batch/flush interval; inverted retry bounds; no enabled signals;
each exporter failing then recovering; span event/end without start; missing
assembly state; incomplete-span shutdown; flush failure; custom exporter code;
shutdown final flush failure; post-shutdown emits; repeated shutdown; telemetry
projector forwarding; and source/context retention. Record `handoff-b-1d.md`.

## Paths to delete

None. Existing APIs, representations, registrations and compatibility paths remain.

## Non-closure

No new exporter transport, public callback ABI, TelemetryError conversion,
legacy API removal, publication, bridge redesign or warning activation.
