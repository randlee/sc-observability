---
id: D.4
status: proposed
branch: feature/phase-d-4-error-enums-2-0
base: develop
---

# D.4 — 2.0 discriminated error enum migration (#92)

## Goal and dependency

Following D.3, execute the explicitly breaking 2.0 migration from the eight
opaque `error_wrapper!` types to same-name discriminated enums. This sprint
accepts ADR-011 and ADR-012 (or records precisely scoped superseding ADR text)
before public code is changed; no acceptance may be inferred from Phase B's
previously proposed ADR status.

## Deliverables

1. Inventory every emitted `ErrorCode` and constructor/call site for
   `InitError`, `EventError`, `FlushError`, `ShutdownError`, `ProjectionError`,
   `SubscriberError`, `LogSinkError`, and `ExportError`; map each to a named
   enum variant carrying `Box<ErrorContext>`. Record every disposition—no
   generic catch-all that hides a known current code.
2. Replace all eight wrappers with `#[non_exhaustive]` same-name public enums;
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
5. Update ADR-011 boundary acceptance and ADR-012 error-migration acceptance,
   requirements/architecture/API design/docs, and public API inventories so
   their status and wording agree with the shipped 2.0 contract.

## Acceptance criteria

- `rg 'error_wrapper!' crates` returns no production macro definition or use;
  all eight former wrappers are public discriminated enums and each known
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
- `cargo test --workspace --locked`, `cargo clippy --workspace --all-targets -- -D warnings`, rustdoc, and public API/semver checks against the declared 1.x baseline.
- Documentation consistency, release-manifest validation, and a clean
  `rg 'error_wrapper!' crates docs` disposition scan.

## Non-closure

No 1.x compatibility promise, registry publication, or unrelated error-model
redesign. #88 remains excluded.
