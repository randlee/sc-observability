# d-13: Logging contract

## Plan metadata

- Wave: 2
- Branch: `sprint/d-13-c-log`
- PR target: `sprint/d-12-c-types`
- Blocked by: `obs-phase-d-plan-qa`
- Owned paths:
  - `crates/sc-observability/src/settings.rs`
  - `crates/sc-observability/src/typed.rs`
  - `crates/sc-observability-log/src/lib.rs`

## Deliverables

1. Add the public `LogSettings`, `ResolvedLogSettings`, `EnvSnapshot`, and `LogSettingsInputs` source/resolved configuration contract, with serde behavior and stable `LogSettingsError` codes from D.1.
2. Add `AttachmentOptions`, the open `BridgeEventPolicy` trait, `BridgeEventDecision`, `PolicyRejection`, and non-owning `LogAttachment` contract from D.2.
3. Add `SinkRegistration::typed(Arc<dyn TypedLogSink>) -> Self` and `LoggerBuilder::register_typed_sink(...)` public signatures from D.3.

```rust
pub trait BridgeEventPolicy: Send + Sync { fn decide(&self, event: &LogEvent) -> BridgeEventDecision; }
pub fn attach_logger(logger: Arc<Logger>, options: AttachmentOptions) -> Result<LogAttachment, InitError>;
pub fn register_typed_sink(&mut self, sink: Arc<dyn TypedLogSink>) -> Result<&mut Self, SinkRegistrationError>;
```

## This Sprint Does Not Close

Environment resolution, bridge lifecycle, and typed-sink implementation are closed by D.1, D.2, and D.3 respectively.


## Design

Contract owner for shared logging public signatures.

## Acceptance criteria

boundary:logging: public signatures compile; run target crate tests and boundary validation.
