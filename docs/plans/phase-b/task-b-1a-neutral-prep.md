---
id: B.1a-neutral-prep
status: complete
branch: feature/phase-b-1a-neutral-prep
worktree: /Users/randlee/github/sc-observability-worktrees/feature/phase-b-1a-neutral-prep
parent: fix/phase-b-policy-sync
authoritative-sprint-doc: sprint-b-1a-error-api.md
contract: error-api-contract.md
---

# B.1a neutral preparation — typed failures and adapters

This scoped plan was reconstructed during execution because the assigned task
plan was absent from the synchronized parent. The sprint document and
`error-api-contract.md` remain the authorities for the API and acceptance
requirements.

## Scope

Implement only the neutral `sc-observability-types` layer:

1. Add `typed::ClassifiedError`, all nine opaque `*Failure` values and their
   non-exhaustive `*FailureKind` enums. Every family gets the exact named
   constructors, `from_context`, `into_context`, four consuming diagnostic
   builders, `DiagnosticInfo`, `std::error::Error`, and bidirectional
   conversion to its retained legacy wrapper.
2. Store the named constructor's discriminant independently of its diagnostic
   code. Classify compatibility contexts by the exact stable code strings;
   unknown, custom, and cross-family codes become `Unclassified` while keeping
   the original context, source, timestamp, and backtrace.
3. Add the five typed resolver/subscriber/projector traits and explicit
   object-safe adapters in `sc_observability_types::typed`. Adapters invoke
   their wrapped operation exactly once and only convert the returned error.
4. Add table-driven classification, constructor, builder, pointer-identity,
   legacy-serialization, and unchanged/typed trait adapter fixtures. Keep all
   typed failures non-Serde and preserve every existing root export, legacy
   wrapper representation, registration, trait, and serialized shape.
5. Record the checked wrapper production-use inventory in
   `error-api-inventory.md`, and record validation plus the deferred copied
   bridge/source integration dependency in `handoff-b-1a.md`.

## Explicit boundaries

No runtime call-site migration, logger sink trait, warning/deprecation,
publication, binding schema, legacy removal, root re-export, root Cargo
metadata change, or bridge signature change belongs to this layer. The copied
bridge reconciliation and full B.1a acceptance remain dependent on B.1 source
integration and the accepted copied bridge. Later findings must be fixed on a
new layer cut from this branch after closeout.

## Validation checklist

- `cargo fmt --all -- --check`
- `cargo test --locked -p sc-observability-types`
- workspace doctests
- `cargo clippy --locked --workspace --all-targets -- -D warnings`
- public API diff/semver/docs and dependency-ban checks
- two-pass worktree-local checklist: API/contract and Rust best practices

Evidence and exact command results are recorded in `handoff-b-1a.md`.
