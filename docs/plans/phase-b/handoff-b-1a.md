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

Focused validation passed:

```text
cargo test --locked -p sc-observability-types: 45 passed; 0 failed
workspace doctests in that command: 2 passed; 0 failed
```

Final validation passed:

```text
cargo fmt --all -- --check: passed
cargo test --locked --workspace: 151 unit/integration tests passed; 0 failed; 6 doctests passed; 0 failed
cargo clippy --locked --workspace --all-targets -- -D warnings: passed
bash scripts/ci/validate_docs_consistency.sh: docs consistency validation passed; rustdoc missing-docs validation passed
bash scripts/ci/validate_dependency_bans.sh: dependency ban validation passed
bash scripts/ci/validate_repo_boundaries.sh: repo boundary validation passed
python3 scripts/ci/validate_public_api_semver.py: public API semver validation passed (223 checks pass, 30 skip per crate)
bash scripts/ci/validate_public_api_docs.sh: public API docs validation passed
bash scripts/ci/validate_public_api_diff.sh: exit 1, public API diff report generated (diffs detected)
```

The API diff exit 1 is the repository's intentional signal for an additive
public diff; the companion docs gate passed because the existing approval
artifacts cover public API changes. No removals or changes were reported for
the scoped legacy APIs.

Two-pass worktree-local checklist:

1. Contract pass: all nine family mappings and named constructors are tested;
   custom and cross-family codes are `Unclassified`; conversions compare the
   same context pointer; legacy Serde and all ten adapter directions pass.
2. Rust best-practices pass: no runtime dependency or root re-export was added;
   typed values remain non-Serde; builders update the existing box in place;
   formatting, clippy, rustdoc missing-docs, workspace tests, boundary, docs,
   dependency, API docs, and semver gates pass.

## Pending dependency and scope honesty

The assigned task plan was missing from the synchronized parent and was
reconstructed from `sprint-b-1a-error-api.md` and `error-api-contract.md`.
`error-api-inventory.md` records all existing nine-wrapper production,
feature-gated, public/custom extension, and internal exporter uses. Runtime
adoption, copied-bridge reconciliation, workspace-level registry parity, and
full B.1a closure remain pending in B.1 integration layers; they are not
claimed as completed evidence by this neutral preparation handoff.
