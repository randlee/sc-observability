# d-2: Host-owned logger bridge and event policy (#204)

## Plan metadata

- Wave: 5
- Branch: `sprint/d-2-host-logger-bridge`
- PR target: `sprint/d-1-log-settings`
- Blocked by: `obs-d-13-sanity`
- Owned paths:
  - `crates/sc-observability-log/src/bridge.rs`
  - `crates/sc-observability-log/tests/bridge_*.rs`
  - `docs/logging/d-2-host-logger-bridge.md`

## Goal and dependency

Let `sc-observability-log` route `log` macros and
`#[instrument]` records to an existing `Arc<sc_observability::Logger>` without
creating another writer, file sink, shutdown owner, or level owner. This is
additive 1.x API work; it does not change direct logger admission semantics or
the existing public `BridgeOptions` shape.


## Deliverables

1. Implement `attach_logger` without constructing a logger or acquiring,
   cloning, or synthesizing `LevelOwner`. Coordinate slot closure and in-flight
   bridge calls so successful explicit detach releases every attachment-owned
   `Arc<Logger>` reference.

2. Apply policy on the reused `CoreLoggerBackend`/`bridge_backend` path before
   every `try_log`. Rejection records the existing `DropCause::InvalidEvent`
   accounting bucket and never calls the sink; this preserves the exhaustive
   1.x `DropCause` ABI. Do not add a second counter or redaction system.
   Policy panics are contained at the boundary.

3. Preserve current owned-init and `ForeignLoggerInstalled` behavior. Document
   facade ownership with a tracing bridge and distinguish the owned `LogGuard`
   lifecycle from the non-owning attachment lifecycle.

4. Add public integration fixtures for direct plus macro logging through one
   recording sink; allowlist/redaction, bounded-payload, rejection, panic,
   foreign-facade, concurrent detach, and ownership recovery cases.

5. Define the `#[non_exhaustive]`
   `DetachError::{Timeout, NotInstalled, ForeignLoggerInstalled}` with stable
   diagnostics; test every slot transition, reattachment, stale control,
   foreign ownership, and the one shared drain. This is the explicit TYP-030
   forward-compatibility exception for this public error.


## Non-closure

No tracing redesign, global facade replacement, owner-capability duplication,
OTLP export, or #88 work.


## Design



## Implementation targets

 implement or update the named contract consumer and its focused test for the corresponding numbered deliverable.\n

## Acceptance criteria

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


