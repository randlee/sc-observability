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

## Goal and dependency

Plan and execute the explicitly breaking 2.0 migration from nine opaque
wrappers (the eight `error_wrapper!` types plus hand-written `IdentityError`)
to same-name discriminated enums. Before public code changes, the technical
lead must accept ADR-017, which precisely supersedes ADR-012 for this listed
2.0 migration while retaining ADR-011's diagnostic boundary. A proposed ADR,
silence, or issue label is not approval.


## Deliverables

1. Remove `error_wrapper!`, obsolete wrapper constructors, tuple-field
   construction, and 1.x compatibility adapters that would retain the old
   representation. Convert workspace, examples, bindings, and fixtures to
   pattern matching/named constructors as appropriate.

## Non-closure

No 1.x compatibility promise, registry publication, or unrelated error-model
redesign. #88 remains excluded.


## Design

Contract: obs-d-12 design, section "D.4 canonical error-enum inventory".

## Owned Paths and Exact Targets

- `Cargo.toml`
- `Cargo.lock`
- `crates/**`
- `bindings/**`
- `examples/**`
- `release/public-api-major-breaks.toml`
- `release/public-api-policy.json`
- `release/release-inventory.json`
- `release/bindings-artifacts.toml`
- `release/publish-artifacts.toml`
- `release/bp2-publish-artifacts.toml`
- `release/RELEASE-NOTES-*.md`
- `CHANGELOG.md`
- `docs/migration-guide.md`
- `docs/migration.md`
- `docs/publishing.md`
- `docs/public-api-checklist.md`
- `scripts/ci/validate_public_api_semver.py`
- `scripts/ci/validate_public_api.py`
- `scripts/ci/validate_version_literals.py`
- `scripts/ci/validate_error_migration.py`
- `scripts/ci/fixtures/**`
- `scripts/ci/validate_python_distribution.py`
- `.github/workflows/**`
- `docs/api-approvals/**`
- `docs/architecture.md`
- `docs/requirements.md`
- `docs/api-design.md`

These are edit fences for the deliverables above, including their tests and
public API approval where listed; reading dependencies does not claim ownership.
New modules stay inside the listed crate fences. No unrelated changes are authorized.

Must follow D.1, D.2, and D.3 because migration edits core src/lib.rs, types src/errors.rs and the log bridge files they own. These are code conflicts, not documentation-only edges. Link their separate additive documents from docs/api-design.md while retaining D.4 sole ownership of the 2.0 baseline.

## Implementation targets


- `crates/sc-observability/src/error_codes.rs`: map core error codes to D12 enum variants (deliverable 1).
- `crates/sc-observability/src/health.rs`: report typed error diagnostics without wrappers (deliverable 2).
- `crates/sc-observability/tests/logging_only.rs`: assert enum variants and `ErrorContext` source identity (deliverable 3).

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


