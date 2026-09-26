---
id: D.2
status: planned
branch: sprint/d-2-host-logger-bridge
base: develop
worktree: /Users/randlee/github/sc-observability-worktrees/sprint/d-2-host-logger-bridge
depends_on: []
relation: parallel_safe
assignee: cobs
model_class: terra
requirements: ["LOG-001", "LOG-010", "LOG-015", "TYP-030", "PHB-002"]
owned_docs: ["docs/logging/d-2-host-logger-bridge.md"]
adrs: ["ADR-011", "ADR-013"]
closure_type: integration
target_boundary: "host logger attachment and facade admission"
owned_paths: ["crates/sc-observability-log/**", "crates/sc-observability-log-consumer-check/**", "docs/api-approvals/d-2-*.json", "docs/logging/d-2-host-logger-bridge.md"]
---

# D.2 — Host-owned logger bridge and event policy (#204)

## Goal and dependency

Let `sc-observability-log` route `log` macros and
`#[instrument]` records to an existing `Arc<sc_observability::Logger>` without
creating another writer, file sink, shutdown owner, or level owner. This is
additive 1.x API work; it does not change direct logger admission semantics or
the existing public `BridgeOptions` shape.

## Public contract

The existing exhaustive two-field `BridgeOptions` and
`init(LoggerConfig, BridgeOptions) -> Result<LogGuard, InitError>` signatures
remain source-compatible and retain their current derives and owned lifecycle.

```rust
pub trait BridgeEventPolicy: Send + Sync {
    fn decide(&self, event: &LogEvent) -> BridgeEventDecision;
}

#[non_exhaustive]
pub enum BridgeEventDecision {
    Admit,
    Reject(PolicyRejection),
}

#[non_exhaustive]
pub enum PolicyRejection {
    Denied,
    PayloadTooLarge,
    Invalid,
}

#[non_exhaustive]
pub struct AttachmentOptions {
    pub bridge: BridgeOptions,
    pub policy: Arc<dyn BridgeEventPolicy>,
}

pub fn attach_logger(
    logger: Arc<Logger>,
    options: AttachmentOptions,
) -> Result<LogAttachment, InitError>;

pub struct LogAttachment { /* no LevelOwner and no Logger shutdown authority */ }

impl LogAttachment {
    pub fn control(&self) -> LogControl;
    pub fn detach(self, timeout: Duration) -> Result<(), DetachError>;
}

impl AttachmentOptions {
    pub fn new(bridge: BridgeOptions, policy: Arc<dyn BridgeEventPolicy>) -> Self;
}
```

The policy inspects but cannot mutate the assembled event and returns admission
or a typed reasoned rejection. Existing `RedactionPolicy` remains the sole
redaction owner. Policy runs on the shared backend path immediately before
every `Logger::try_log`, including `LogControl::try_log`; macro, tracing, and
control entry points cannot bypass it. The trait is intentionally open for
host implementations; its one method plus non-exhaustive decision/reason
enums is the forward-compatibility decision.
The policy applies only to facade/attachment admission; a host's direct
`Logger::try_log` intentionally bypasses it. `BridgeEventPolicy` is owned by
`sc-observability-log`, which owns that facade boundary.

`LogAttachment` deliberately has no `elevate_level`, `reset_level`, or logger
shutdown method. `detach` first closes/removes the bridge slot so no new calls
can clone the logger, then waits boundedly for already-entered bridge calls to
release their references. `Drop` performs the same bounded detach but discards
the result; callers that need proof use explicit `detach`. Detach never invokes
`Logger::shutdown`. The host retains its original `Arc<Logger>` and can recover
the owned `Logger` with `Arc::try_unwrap` after successful detach.

The process-global slot has explicit `Empty`, `Owned`, `Attached`, and
`Closing` states. Owned `init` and host attachment are mutually exclusive.
The facade shim and `log::set_max_level(Trace)` are installed at most once;
the shim owns the process-global maximum while the backend filter owns actual
admission. A detached slot may be reattached through that shim, while a
foreign logger is always rejected. A saved `LogControl` after detach returns
the stable `NotInstalled` failure. Owned shutdown and attachment detach call
one `close_and_drain` primitive; detach never shuts down the host logger.

## Deliverables

1. Add the separate policy-bearing `AttachmentOptions`, `BridgeEventPolicy`,
   `BridgeEventDecision`, and non-owning `LogAttachment` contracts above. Do
   not add a field to `BridgeOptions` or alter existing `init`/`LogGuard`.
2. Implement `attach_logger` without constructing a logger or acquiring,
   cloning, or synthesizing `LevelOwner`. Coordinate slot closure and in-flight
   bridge calls so successful explicit detach releases every attachment-owned
   `Arc<Logger>` reference.
3. Apply policy on the reused `CoreLoggerBackend`/`bridge_backend` path before
   every `try_log`. Rejection records the existing `DropCause::InvalidEvent`
   accounting bucket and never calls the sink; this preserves the exhaustive
   1.x `DropCause` ABI. Do not add a second counter or redaction system.
   Policy panics are contained at the boundary.
4. Preserve current owned-init and `ForeignLoggerInstalled` behavior. Document
   facade ownership with a tracing bridge and distinguish the owned `LogGuard`
   lifecycle from the non-owning attachment lifecycle.
5. Add public integration fixtures for direct plus macro logging through one
   recording sink; allowlist/redaction, bounded-payload, rejection, panic,
   foreign-facade, concurrent detach, and ownership recovery cases.
6. Define the `#[non_exhaustive]`
   `DetachError::{Timeout, NotInstalled, ForeignLoggerInstalled}` with stable
   diagnostics; test every slot transition, reattachment, stale control,
   foreign ownership, and the one shared drain. This is the explicit TYP-030
   forward-compatibility exception for this public error.

## Acceptance criteria

- An unchanged downstream `BridgeOptions { default_action,
  parse_bracket_action }` literal and existing `init` call compile under the
  repository's 1.x semver fixture.
- A host-created logger receives direct and macro-originated records through
  one sink/writer, with no second logger or `LevelOwner` construction.
- A rejected event is absent from the sink, is visible through existing
  dropped-event accounting, and cannot bypass policy through `#[instrument]`
  or `LogControl::try_log`; existing redaction still runs exactly once.
- `LogAttachment` exposes no level mutation or shutdown authority. A public
  fixture explicitly detaches, logs directly through the host logger, proves
  `Arc::try_unwrap` succeeds, and calls the real consuming
  `Logger::shutdown`.
- Concurrent detach rejects new bridge admission, drains already-entered calls
  within the bound, and returns a typed timeout rather than leaking ownership.
- Existing owned `LogGuard`, foreign global logger, and tracing coexistence
  fixtures retain their documented behavior.
- Fixtures prove init/attach exclusion, reattachment, stable maximum-level
  ownership, stale-control rejection, and no duplicated backend/drain loop.

## Required validation

- Focused `cargo test -p sc-observability-log` macro, policy, detach,
  ownership-recovery, and foreign-facade fixtures.
- An unchanged-old-struct-literal compile fixture plus public attachment
  consumer fixture in the additive API/semver gate against published 1.4.1.
- `cargo test --workspace --locked` and
  `cargo clippy --workspace --all-targets -- -D warnings`.

## Owned Paths and Exact Targets

- `crates/sc-observability-log/**`
- `crates/sc-observability-log-consumer-check/**`
- `docs/api-approvals/d-2-*.json`
- `docs/logging/d-2-host-logger-bridge.md`

These are edit fences for the deliverables above, including their tests and
public API approval where listed; reading dependencies does not claim ownership.
New modules stay inside the listed crate fences. No unrelated changes are authorized.

Parallel-safe with the other additive logging sprints: this sprint owns its separate additive document and scoped API approval. D.4 owns linking these documents from the shared API design. No shared normative document or release baseline is edited here.

## Non-closure

No tracing redesign, global facade replacement, owner-capability duplication,
OTLP export, or #88 work.
