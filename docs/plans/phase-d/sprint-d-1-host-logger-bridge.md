---
id: D.1
status: proposed
branch: feature/phase-d-1-host-logger-bridge
base: develop
---

# D.1 — Host-owned logger bridge and event policy (#204)

## Goal and dependency

After D.2 has merged, let `sc-observability-log` route `log` macros and
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
    fn apply(&self, event: LogEvent) -> BridgeEventDecision;
}

pub enum BridgeEventDecision {
    Admit(LogEvent),
    Reject(PolicyRejection),
}

pub enum PolicyRejection {
    Denied,
    PayloadTooLarge,
    Invalid,
}

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
    pub fn policy_rejections(&self) -> u64;
    pub fn detach(self, timeout: Duration) -> Result<(), DetachError>;
}
```

The policy consumes the assembled event and either returns the admitted event
(allowing bounded mutation/redaction) or a typed reasoned rejection. It runs
after macro/tracing assembly and before `Logger::try_log`.

`LogAttachment` deliberately has no `elevate_level`, `reset_level`, or logger
shutdown method. `detach` first closes/removes the bridge slot so no new calls
can clone the logger, then waits boundedly for already-entered bridge calls to
release their references. `Drop` performs the same bounded detach but discards
the result; callers that need proof use explicit `detach`. Detach never invokes
`Logger::shutdown`. The host retains its original `Arc<Logger>` and can recover
the owned `Logger` with `Arc::try_unwrap` after successful detach.

## Deliverables

1. Add the separate policy-bearing `AttachmentOptions`, `BridgeEventPolicy`,
   `BridgeEventDecision`, and non-owning `LogAttachment` contracts above. Do
   not add a field to `BridgeOptions` or alter existing `init`/`LogGuard`.
2. Implement `attach_logger` without constructing a logger or acquiring,
   cloning, or synthesizing `LevelOwner`. Coordinate slot closure and in-flight
   bridge calls so successful explicit detach releases every attachment-owned
   `Arc<Logger>` reference.
3. Apply policy after event assembly and before `try_log`. Rejection increments
   a dedicated inspectable policy-drop counter, records a bounded reason, and
   never calls the sink. Policy panics are contained at the facade boundary.
4. Preserve current owned-init and `ForeignLoggerInstalled` behavior. Document
   facade ownership with a tracing bridge and distinguish the owned `LogGuard`
   lifecycle from the non-owning attachment lifecycle.
5. Add public integration fixtures for direct plus macro logging through one
   recording sink; allowlist/redaction, bounded-payload, rejection, panic,
   foreign-facade, concurrent detach, and ownership recovery cases.

## Acceptance criteria

- An unchanged downstream `BridgeOptions { default_action,
  parse_bracket_action }` literal and existing `init` call compile under the
  repository's 1.x semver fixture.
- A host-created logger receives direct and macro-originated records through
  one sink/writer, with no second logger or `LevelOwner` construction.
- A rejected event is absent from the sink, has an observable policy-drop
  outcome/counter, and cannot bypass policy via `#[instrument]`; an admitted
  transformed event proves redaction occurs before the sink.
- `LogAttachment` exposes no level mutation or shutdown authority. A public
  fixture explicitly detaches, logs directly through the host logger, proves
  `Arc::try_unwrap` succeeds, and calls the real consuming
  `Logger::shutdown`.
- Concurrent detach rejects new bridge admission, drains already-entered calls
  within the bound, and returns a typed timeout rather than leaking ownership.
- Existing owned `LogGuard`, foreign global logger, and tracing coexistence
  fixtures retain their documented behavior.

## Required validation

- Focused `cargo test -p sc-observability-log` macro, policy, detach,
  ownership-recovery, and foreign-facade fixtures.
- An unchanged-old-struct-literal compile fixture plus public attachment
  consumer fixture in the 1.x semver/API gate.
- `cargo test --workspace --locked` and
  `cargo clippy --workspace --all-targets -- -D warnings`.

## Non-closure

No tracing redesign, global facade replacement, owner-capability duplication,
OTLP export, or #88 work.
