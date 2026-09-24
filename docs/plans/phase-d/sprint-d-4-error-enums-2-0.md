---
id: D.4
status: complete
branch: feature/phase-d-4-error-enums-2-0
base: develop
worktree: /Users/randlee/github/sc-observability-worktrees/feature/phase-d-4-error-enums-2-0
depends_on: [D.3]
relation: must_follow
owned_docs: [docs/architecture.md, docs/requirements.md, docs/api-design.md, docs/migrate-error-api.md]
---

# D.4 — 2.0 discriminated error enum migration (#92)

## Goal and dependency

Following D.3, execute the explicitly breaking 2.0 migration from nine opaque
wrappers (the eight `error_wrapper!` types plus hand-written `IdentityError`)
to same-name discriminated enums. Before public code changes, the technical
lead must accept ADR-017, which precisely supersedes ADR-012 for this listed
2.0 migration while retaining ADR-011's diagnostic boundary. A proposed ADR,
silence, or issue label is not approval.

## Deliverables

1. Inventory every emitted `ErrorCode` and constructor/call site for
   `InitError`, `EventError`, `FlushError`, `ShutdownError`, `ProjectionError`,
   `SubscriberError`, `LogSinkError`, `ExportError`, and `IdentityError`; map each to a named
   enum variant carrying `Box<ErrorContext>`. Record every disposition—no
   generic catch-all that hides a known current code.
2. Replace all nine wrappers with `#[non_exhaustive]` same-name public enums;
   make all existing public error enums non-exhaustive where the 2.0 contract
   requires future-safe matching. Preserve `DiagnosticInfo`, stable diagnostic
   code/message/remediation/backtrace/source behavior by delegation.
3. Remove `error_wrapper!`, obsolete wrapper constructors, tuple-field
   construction, and 1.x compatibility adapters that would retain the old
   representation. Convert workspace, examples, bindings, and fixtures to
   pattern matching/named constructors as appropriate.
4. Perform the 2.0 release work: coordinated workspace/package version bump,
   dependency/version literals and release manifests, changelog/release notes,
   public API approval/semver baseline update, and a migration guide with
   before/after examples for `?` propagation, matching, custom sinks, and
   serialization expectations. State that exhaustive downstream matching is a
   breaking change and show wildcard matching.
5. Record accepted ADR-017, mark the conflicting portion of ADR-012 superseded
   (without rewriting its historical decision), and update requirements,
   architecture, API design, migration docs, and public inventories to agree.
6. Record an explicit disposition for every parallel surface. Remove
   `typed::*Failure` duplicates and `impl_legacy_classification!`; migrate typed
   methods to the canonical same-name enums. Make canonical `LogSink` return
   the discriminated `LogSinkError`, remove `TypedLogSink`, `legacy_sink`, and
   D.3's `SinkRegistration::typed`/`register_typed_sink` bridge. Inventory the
   log crate's own error enums and either migrate a distinct boundary with a
   documented mapping or retain it with proof it is not a duplicate.
7. Update every version-bearing target: workspace/package `Cargo.toml` and
   `Cargo.lock`, `crates/sc-observability-py/{Cargo.toml,pyproject.toml}`,
   JavaScript package metadata, `release/{publish-artifacts.toml,
   bindings-artifacts.toml,python-platform-policy.json}`, hard-coded wheel
   names/fixtures, changelog, and release documentation. A repository scan
   must disposition every remaining `1.4.1` literal.
8. Add the controlled major-release mechanism to
   `validate_public_api_semver.py`: compare against frozen 1.4.1, require every
   break in reviewed `release/public-api-major-breaks.toml` linked to accepted
   ADR-017, fail unlisted changes, then generate/review the 2.0 baseline.

## Acceptance criteria

- `rg 'error_wrapper!' crates` returns no production macro definition or use;
  all nine former wrappers are public discriminated enums and each known
  emitted code has an explicit variant disposition.
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
- Documentation consistency, release-manifest validation, and a clean
  `rg 'error_wrapper!' crates docs` disposition scan.

## Non-closure

No 1.x compatibility promise, registry publication, or unrelated error-model
redesign. #88 remains excluded.
