---
id: B.1a
status: complete
branch: feature/phase-b-1a-neutral-prep
base: fix/phase-b-policy-sync
worktree: /Users/randlee/github/sc-observability-worktrees/feature/phase-b-1a-neutral-prep
---

# B.1a — Typed failure values and neutral extension adapters

## Goal and dependencies

Implement the neutral portion of #92 with stored typed classification and
lossless legacy conversion. `must_follow` B.1 so inventory includes the accepted
bridge and B.P2-qualified staged runtime-level prerequisite; runtime-contract
acceptance remains owner-deferred to Phase B completion. B.1b `must_follow` this
sprint.
The normative signatures and code mapping are in [the contract](error-api-contract.md),
which is part of this sprint's QA scope, not a future design deliverable.

For every `must_follow`, merge pushed parent development into the child before
every development/fix round; the parent PR merges before child completion.
No listed related sprint is `parallel_safe`: shared neutral contracts, runtime
call sites or release artifacts intersect.

## Deliverables (authoritative)

Every listed deliverable must land production-ready for this sprint's stated
scope. Completion requires evidence for every numbered item, including its code,
documentation and validation artifacts; partial completion leaves the sprint open.

1. Implement `typed::ClassifiedError`, the nine distinct `*Failure` values and their
   `*FailureKind` discriminated enums in `sc-observability-types`, with named
   constructors, diagnostic/context access and bidirectional legacy conversion
   exactly as specified in the linked contract. First-party new constructors
   select kinds at the failure site; context classification is a compatibility path.
2. Add the five typed resolver/subscriber/projector traits and explicit adapters
   in the contract under `sc_observability_types::typed`, without root
   re-exports. They operate only on neutral types and existing registration
   interfaces; do not depend on runtime crates or modify existing traits.
3. Add the conversion/trait compile fixtures and a checked production-constructor
   inventory at `docs/plans/phase-b/error-api-inventory.md` during execution.
   Inventory every use of the nine wrappers, including public signatures,
   feature-gated code, custom extension points and internal exporters against
   the source ownership table in the contract. This is execution evidence for
   that fixed scope, not deferred API design or discretionary boundary selection.

## Contract

```rust
// In sc_observability_types::typed, never root-re-exported.
pub trait ClassifiedError: DiagnosticInfo {
    type Kind: Copy + Eq;
    fn kind(&self) -> Self::Kind;
    fn context(&self) -> &ErrorContext;
}
// Representative; exact families and constructors are enumerated in the contract.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdentityFailureKind { ResolutionFailed, Unclassified }
pub struct IdentityFailure { /* private kind and original Box<ErrorContext> */ }
impl IdentityFailure {
    pub fn resolution_failed(message: impl Into<String>, remediation: Remediation) -> Self;
    pub fn from_context(context: Box<ErrorContext>) -> Self;
    pub fn into_context(self) -> Box<ErrorContext>;
}
impl From<IdentityError> for IdentityFailure { /* move original context */ }
impl From<IdentityFailure> for IdentityError { /* move original context */ }
```

The failure is a new opaque value with an immutable, discriminated kind, rather
than an in-place change to the legacy tuple struct. This prevents callers from
pairing a named variant with the wrong code. No fields are added to published
structs, and no variants or attributes are added to published enums. Existing
`DiagnosticInfo` sealing remains unchanged. Native failure types have no Serde
implementation in this phase; legacy serialization and versioned binding DTOs
remain separate contracts.

## Acceptance criteria (authoritative)

- AC1: All nine families expose total typed classification and named creation;
  missing/custom/cross-family codes yield `Unclassified` without data loss or panic.
- AC2: Conversions preserve the same boxed context, source chain and backtrace;
  every existing legacy serialized fixture and custom-trait compile fixture passes.
- AC3: Both adapter directions retain object safety, Send/Sync bounds, success
  values and original failure metadata; adapters never recurse or retry operations.
- AC4: Published API review is additive; no deprecations are enabled before the
  recommended runtime paths land in B.1b–B.1d.

## Required validation (authoritative)

Run `cargo fmt --all -- --check`, `cargo test --locked -p sc-observability-types`,
workspace doctests and `cargo clippy --locked --workspace --all-targets -- -D warnings`.
Run public API diff/semver/docs and dependency-ban checks. Add table-driven
fixtures for every contract mapping, every named constructor, unknown codes and
wrong-family codes; compare context pointer identity across round trips and
legacy Serde bytes. Compile/run unchanged and typed implementations of all five
open extension traits through their explicit adapters, and compile unchanged root-glob consumers
without adding qualifications or changing imports. Record evidence in
`docs/plans/phase-b/handoff-b-1a.md`.

## Paths to delete

None. Existing APIs, representations, registrations and compatibility paths remain.

## Non-closure

No runtime replacement, warning activation, publication, binding schema change,
legacy removal or changed bridge signature. B.1e owns deprecation and adoption.
This sprint closes neutral types/adapters, not all of #92.
