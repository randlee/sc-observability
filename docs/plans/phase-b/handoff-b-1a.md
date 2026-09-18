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
public diff. The API docs validator only checks that approval files exist; its
PASS is not approval of this new API. Independent review acceptance for the
typed API remains pending. No removals or changes were reported for the scoped
legacy APIs.

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

## Fixture correction handoff

The follow-up fixture review reproduced and fixed all four assigned findings:

- N01: `src/typed.rs` now exercises all nine families through all four
  consuming builders, preserving kind, context-box address, backtrace address,
  timestamp, and source chain; all nine legacy round trips and family-local
  custom/cross-family `Unclassified` cases are checked.
- N02: `tests/neutral_contracts.rs` exercises all ten adapter directions with
  actual errors, context/source address checks, exactly-one invocation counters,
  and concrete success-value equality checks.
- N03: the external test is a root-glob unchanged-consumer proof; the typed
  module contains separate `compile_fail` doctests proving failures do not
  implement `Clone` or Serde.
- N04: the API-doc PASS claim was corrected above. Existing approval-file
  presence is not treated as approval; independent API acceptance remains
  pending review.

Second-pass exact output:

```text
cargo fmt --all -- --check: passed
cargo test --locked --workspace: 158 unit/integration tests passed; 0 failed; 6 normal doctests passed; 2 compile-fail doctests passed
cargo clippy --locked --workspace --all-targets -- -D warnings: passed
python3 -m unittest discover -s scripts/ci/tests -p 'test_validate_log_import.py': 24 tests, OK
```

The final parent merge-forward was `b69b8c5d2574e21f94c686d135686d7b42ac6509`
from `feature/phase-b-1-provenance-prep`; no provenance tooling was edited.

## Integration-layer status addendum (feature/phase-b-1-integration)

Implementation-complete, confirmed against merged source: all nine
`typed::*Failure` families, `ClassifiedError`, and the adapter traits listed
above are present and unchanged from this handoff's description. Registry
parity across all nine families is now proven by an executable test
(`crates/sc-observability-otlp/tests/error_registry_parity.rs`, 9 tests, all
passing), superseding the "workspace-level registry parity... remain pending"
line above. The checked `error-api-inventory.md` (also owned by the
integration layer) records every production, feature-gated, and copied-bridge
use of `IdentityError`, the only B.1a-owned family the frozen bridge import
touches.

Still not established by this addendum: independent QA/coordinator
completeness PASS for either this preparation layer or the integration layer
itself, and the copied-bridge `IdentityError` call sites' narrow deprecated-use
allowance is coordinated with lobs and not yet fully landed (provenance
manifest side pending as of this writing). This addendum reports evidence
gathered by the integration layer; it does not itself constitute the
coordinator's completeness review.
