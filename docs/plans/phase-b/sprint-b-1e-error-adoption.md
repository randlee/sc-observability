---
id: B.1e
status: complete
branch: feature/phase-b-1e-migration-validation
base: fix/phase-b-1ab-qa1
worktree: /Users/randlee/github/sc-observability-worktrees/feature/phase-b-1e-migration-validation
---

# B.1e — Warn-only deprecations and downstream upgrade guidance

## Goal and dependencies

`must_follow` B.1d and transitively B.1a–B.1c: every replacement must work before
warnings recommend it. B.2 `must_follow` this sprint for publication; B.3 consumes
the resulting public classification contract. This sprint owns rollout/documentation,
not another runtime implementation or removal milestone.

For every `must_follow`, merge pushed parent development into the child before
every development/fix round; the parent PR merges before child completion.
No listed related sprint is `parallel_safe`: shared neutral contracts, runtime
call sites or release artifacts intersect.

## Deliverables (authoritative)

Every listed deliverable must land production-ready for this sprint's stated
scope. Completion requires evidence for every numbered item, including its code,
documentation and validation artifacts; partial completion leaves the sprint open.

1. Apply the contract's exact deprecation policy to nine wrapper types and the
   mapped old methods in B.1b–B.1d except the explicit supported-method
   exemptions below. Each `since` is the selected next minor release
   after B.P2's staged candidate version; each `note` names its method/type
   replacement and the migration guide.
   Existing `Logger::emit` keeps its existing deprecation version and documents
   typed blocking versus nonblocking alternatives without changing behavior.
2. Migrate first-party ordinary production use, examples and docs to recommended
   entry points and typed kind matching. Retain narrow compatibility fixtures and
   adapter/trait-impl modules with explicit `allow(deprecated)` reasons. The copied
   bridge may retain narrowly scoped uses necessary for its accepted public API;
   record internal-only edits separately from B.1 historical provenance. Do not
   rename its public errors or alter its signature/variant baseline.
3. Extend `.claude/skills/sc-observability-adopting/SKILL.md` with routing to
   `references/migrate-error-api.md`. Provide old→new symbol table, version
   prerequisite, typed matching with fallback, preservation of diagnostic/source
   data, custom-trait adapters, incremental rollback and narrow warning handling.
   Update requirements/API-design migration status, migration guide, changelog and
   release documentation consistently, distinguishing implemented from proposed.
4. Add `scripts/ci/validate_error_migration.py` and three downstream Cargo fixtures:
   unchanged legacy (default lints), migrated (deny deprecated), and partial
   migration (local allow on explicit legacy boundary). Validate all four crates,
   trait implementations and old serialized values from outside their workspace.

## Contract

The old→new method mapping is exact: every `foo_typed` declaration in B.1b–B.1d
recommends the same owner's `foo` operation, retaining arguments, success values
and consuming/borrowing semantics. Exceptions to method warning activation are
legacy infallible LoggerBuilder::build and the B.P1 owner constructors. Keep
legacy build unchanged and supported; its new
build_typed counterpart adds a fallible startup boundary rather than changing
the old signature. Logger::new_with_level_owner and
LoggerBuilder::build_with_level_owner retain their B.P1 Result signatures and
remain supported without method-level deprecation; avoid qualifying/staging
them in B.P2 only to deprecate the methods in B.2. Their `_typed` counterparts are
additive recommended alternatives, not mandatory replacements. Nine `XError`
wrappers still recommend `XFailure`, including InitError returned by those
supported owner constructors. Explicitly naming/constructing InitError can warn;
that is a wrapper warning, not a deprecated-method warning. Keep narrowly
justified allowances on retained signatures/compatibility implementations where
required, and explain that strict-lint consumers can use the typed counterparts
or allow the specific wrapper use. Do not promise warning-free use of explicitly
named legacy error types.
Do not mark unrelated methods/types deprecated. The existing LogError/TryLogError,
ObservationError, TelemetryError and QueryError remain supported; new logger
methods use improved nested EventFailure without changing those old enums.

A legacy caller with default lints must still build and run. Caller-selected
`-D warnings`/`-D deprecated` can fail because warnings are the explicit migration
mechanism. Explain that policy; never promise warning-free legacy compilation.
Each allow must name a compatibility symbol/module and justification. No crate-
root or workspace-wide deprecation suppression, hidden unwrap/panic fallback, or
automatic rewrite of downstream repos is authorized by this sprint.

The skill teaches explicit Result matching and `ClassifiedError::kind`, retaining
a fallback for new non_exhaustive kinds and all custom Unclassified diagnostics.
The migration fixture follows the guide verbatim and handles both success and
failure; import-only examples do not establish an upgrade path.

## Acceptance criteria (authoritative)

- AC1: Every deprecated item has a working, documented replacement; no warning
  encourages a planned-only API. New consumer fixtures deny deprecated usage.
  Both B.P1 owner constructors remain callable with no method-level deprecation
  attribute; their InitError wrapper policy is documented separately.
- AC2: Legacy/default-lint and partially migrated fixtures execute successfully
  with expected deprecation diagnostic codes, exact replacement notes and
  unchanged Serde fixtures; custom traits compile without required-method changes.
- AC3: A clean downstream fixture follows the adoption skill successfully across
  logger, observation and telemetry paths. Narrow internal bridge allowances
  preserve its exact accepted public contract and regression behavior.
- AC4: Public API/semver review finds only additive changes and authorized warning
  attributes. All documents state no removal schedule and no planned major release.

## Required validation (authoritative)

Run `python3 scripts/ci/validate_error_migration.py` (implemented here), workspace
formatting/tests/doctests/clippy with warnings denied and only named compatibility
allows, public API diff/semver/docs, docs-consistency and copied bridge regressions.
The new validator parses Cargo JSON warning diagnostics, asserts each deprecated
item's code and replacement note, rejects unexpected warnings and broad allows,
runs all three fixtures and legacy JSON golden values, and fails if a fixture or
code mapping is skipped. Add focused fixtures calling both B.P1 owner
constructors without explicitly naming InitError, explicitly naming InitError,
and using both `_typed` counterparts under deny(deprecated). Assert no method
is marked deprecated, isolate any wrapper-use diagnostic by its span/item, and
verify the typed counterparts need no deprecation allowance. A blanket allow
that hides unexpected method warnings fails validation. Test migration of tuple construction/access, match-based
kind handling, custom codes, source chains, each open trait adapter and mixed
old/new registrations, and unchanged root-glob imports. New typed traits and
adapters are imported from explicit typed modules; no new root re-exports
introduce same-named trait methods into legacy glob consumers. Record exact versions and results in `handoff-b-1e.md`.

## Completion evidence

B.1e implementation is complete on the branch recorded in the frontmatter.
The nine legacy wrapper families and all 20 mapped methods carry actionable
`since = "1.4.0"` warnings with exact typed replacements. The supported
infallible `LoggerBuilder::build`, both owner-returning constructors, and the
existing `Logger::emit` `since = "1.2.0"` policy remain unchanged. Ordinary
routing production paths use typed logger admission and flush APIs; retained
public trait and adapter boundaries use named, reason-bearing local allowances.

The standalone validator and external fixtures are present at
`scripts/ci/validate_error_migration.py` and
`scripts/ci/fixtures/error-migration/`. The validator passed source-contract,
Cargo JSON diagnostic, legacy Serde golden, migrated `deny(deprecated)`, and
partial local-allow checks, with all three fixtures compiling and running.
The implementation also merged the active QA1 parent before final validation.

Qualification remains B.2 work and publication remains B.7 work; this sprint
does not remove legacy APIs, set a removal schedule, or claim a major release.

## Paths to delete

None. Existing APIs, representations, registrations and compatibility paths remain.

## Non-closure

No publication (B.2), legacy removal, in-place representation replacement,
scheduled 2.0 conversion, BTIT dependency switch or bridge API redesign. #92's
additive migration closes only after these runtime and adoption gates pass.
