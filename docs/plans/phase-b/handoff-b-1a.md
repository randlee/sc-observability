# B.1a neutral preparation handoff

## Implementation

- Added `sc_observability_types::typed` with `ClassifiedError`, nine opaque
  failure families, stored family kinds, exact canonical code mappings, named
  constructors, in-place builders, and `from_context`/`into_context` APIs.
- Added bidirectional conversions that move the original legacy context box,
  preserving its source chain, timestamp, and captured backtrace.
- Added typed process identity, subscriber, log projector, span projector, and
  metric projector traits plus explicit adapters in both directions.
- Retained all root exports, legacy wrappers, open legacy traits,
  registrations, and legacy Serde behavior. No runtime crate dependency was
  introduced.
- Added focused fixtures in `src/typed.rs` covering constructors, unknown and
  wrong-family codes, pointer identity, builders, legacy serialization, and
  all adapter directions.

## Validation evidence

Initial focused validation passed:

```text
cargo test --locked -p sc-observability-types: 44 passed; 0 failed
workspace doctests in that command: 2 passed; 0 failed
```

The final handoff will append the exact output for formatting, workspace tests,
clippy, public API/docs checks, dependency bans, and the two-pass checklist
after the branch is synchronized with its parent immediately before push.

## Pending dependency and scope honesty

The assigned task plan was missing from the synchronized parent and was
reconstructed from `sprint-b-1a-error-api.md` and `error-api-contract.md`.
`error-api-inventory.md` records all existing nine-wrapper production,
feature-gated, public/custom extension, and internal exporter uses. Runtime
adoption, copied-bridge reconciliation, workspace-level registry parity, and
full B.1a closure remain pending in B.1 integration layers; they are not
claimed as completed evidence by this neutral preparation handoff.
