# d-4: 2.0 discriminated error enum migration (#92)

## Plan metadata

- Wave: 7
- Branch: `sprint/d-4-error-enums-2-0`
- PR target: `sprint/d-3-typed-sink-registration`
- Blocked by: `obs-d-12-sanity`
- Owned paths:
  - `crates/sc-observability/src/error_codes.rs`
  - `crates/sc-observability/src/health.rs`
  - `crates/sc-observability/tests/logging_only.rs`
  - `crates/sc-observability/src/lib.rs`
  - `crates/sc-observability/src/sinks.rs`

## Goal and dependency

Migrate only the sc-observability crate’s error surface to the D12 nine-enum contract.

## Deliverables

1. Replace obsolete wrapper construction and tuple-field access in `src/lib.rs`, `src/sinks.rs`, `src/error_codes.rs`, and `src/health.rs` with D12 named variants and boxed `ErrorContext`.
2. Retype `tests/logging_only.rs` for named variants, stable codes, and source identity.

## Non-closure

Examples, bindings, fixtures, workspace release/API evidence, and compatibility removal outside this crate are D18.

## Design

Contract: obs-d-12 design, section "D.4 canonical error-enum inventory".

## Implementation targets

- `crates/sc-observability/src/lib.rs`: replace core `LogError`/`TryLogError` compatibility construction with D12 named variants (deliverable 1).
- `crates/sc-observability/src/sinks.rs`: construct `LogSinkError::{Write, Flush}` with boxed `ErrorContext` (deliverable 1).
- `crates/sc-observability/src/error_codes.rs`: preserve stable core error-code mapping (deliverable 1).
- `crates/sc-observability/src/health.rs`: render typed diagnostic context without tuple-wrapper access (deliverable 1).
- `crates/sc-observability/tests/logging_only.rs`: assert variant, code, and source identity (deliverable 1).

## Acceptance criteria

## Acceptance criteria

- All nine former wrappers are public discriminated enums.
- Every retained diagnostic property is tested per enum variant, including
  source chain and serde shape where public serialization is promised.
- A 1.x consumer fixture fails only at intentional wrapper-construction or
  exhaustive-match boundaries; its paired 2.0 fixture compiles without
  deprecated/error-wrapper dependencies.
- Version, lock/manifests, changelog, migration guide, API approval, and ADR
  acceptance all name 2.0 and agree on the breaking scope.


## Required validation

- Focused type/error-code matrix tests and cross-crate consumer fixtures.
- `cargo test --workspace --locked`, `cargo clippy --workspace --all-targets -- -D warnings`, rustdoc, and the explicit major-release API comparison against published 1.4.1 followed by reviewed 2.0 rebaseline.
- Documentation consistency and release-manifest validation.


