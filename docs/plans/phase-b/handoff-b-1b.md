# B.1b typed logger preparation handoff

## Implementation

The logger preparation layer adds neutral `LogFailure` and `TryLogFailure`,
typed startup, admission, and flush methods, plus `typed::TypedLogSink` and
explicit legacy/typed adapters. JSONL and console sinks run their typed write
implementation once; their retained `LogSink` implementations only convert the
returned error. Validation, unavailable-level admission, and writer flush now
produce neutral failures at their production sites, while legacy facade methods
perform compatibility conversion.

The retained `emit` path, owner constructors, query/follow/health/shutdown
surfaces, admission/filtering semantics, and bridge API are unchanged.

## Parent and revisions

- Required provenance parent merged: `8aeabb584b3b2d004606591ff80741867ebb9ca7`
- Merge-forward revision: `44b3f3c5d06e3ab4195984f8141777be5726ae6d`
- Typed production-site correction: `2d1f207c24268d618f1fda135484cde88f079706`

## Validation

- PASS: `cargo test --locked -p sc-observability --all-targets`
- PASS: `cargo test --locked -p sc-observability --all-targets --all-features`
  (75 unit tests and the logging-only consumer)
- PASS: `cargo fmt --all -- --check`
- PASS: `cargo clippy --locked -p sc-observability --all-targets --all-features -- -D warnings`
- PASS: `cargo test --locked -p sc-observability --doc`
- PASS: `cargo test --locked --workspace --doc`
- PASS: `cargo clippy --locked --workspace --all-targets --all-features -- -D warnings`

The task-plan fixture matrix records the paired legacy/typed and retained
behavior coverage. The copied bridge regression is pending B.1 source
integration and is intentionally not represented as a completed local run.

## Remaining integration boundary

This is scoped B.1b preparation only. B.1 accepted bridge integration,
observation/telemetry adoption, warning activation, publication, and phase
closure remain owned by their respective later layers.
