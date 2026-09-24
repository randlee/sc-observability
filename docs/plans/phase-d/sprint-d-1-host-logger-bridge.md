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
creating another writer, file sink, or shutdown owner. This is additive 1.x
API work; it does not change direct logger admission semantics.

## Deliverables

1. Add an additive host attachment entry point such as
   `init_with_logger(Arc<Logger>, BridgeOptions) -> Result<LogGuard, InitError>`.
   The returned guard owns only the facade/bridge attachment; dropping or
   shutting it down must never call `Logger::shutdown` on the supplied logger.
2. Add an explicit, `Send + Sync` event-policy contract to `BridgeOptions`,
   invoked after macro/tracing event assembly and before `try_log`. Its decision
   is `Admit` or a reasoned rejection; rejection increments a dedicated,
   inspectable bridge drop counter and never calls the sink.
3. Preserve current owned-init behavior and `ForeignLoggerInstalled` behavior.
   Document exactly which facade owner is permitted when a tracing bridge is
   also present, and what lifecycle owner shuts down the underlying logger.
4. Add integration fixtures for host direct logging plus macro logging through
   one recording sink; allowlist/redaction, bounded-payload, and rejection
   policy cases; foreign-facade rejection; and guard drop without host shutdown.

## Acceptance criteria

- A host-created logger receives direct and macro-originated records through
  one sink/writer, with no second logger construction.
- A rejected event is absent from the sink, has an observable policy-drop
  outcome/counter, and cannot bypass the policy via `#[instrument]`.
- Host logger health remains available after `LogGuard` drop; explicit host
  shutdown remains solely host-controlled.
- Existing `init(config, options)` tests and a foreign global logger fixture
  retain their documented behavior.
- Rustdoc and consumer example compile without installing a global logger at
  import/construction time.

## Required validation

- Focused `cargo test -p sc-observability-log` macro, policy, lifecycle, and
  foreign-facade fixtures.
- `cargo test --workspace --locked` and `cargo clippy --workspace --all-targets -- -D warnings`.
- A compile-time consumer fixture uses only public APIs.

## Non-closure

No tracing redesign, global facade replacement, OTLP export, or #88 work.
