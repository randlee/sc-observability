---
id: B.1a
status: proposed
branch: feature/phase-b-1a-error-api
base: develop
---

# B.1a — Typed error implementation and warning-only legacy migration

## Goal and dependencies

Implement the additive migration portion of issue #92 before first Phase B
publication. The improved implementation is the recommended API; the existing
interface stays functional with compiler-visible deprecation warnings. Removal
has no scheduled release. This is published core API evolution, not redesign of
the copied bridge contract.

`must_follow` B.1 because the copied bridge participates in compatibility checks.
B.2 `must_follow` this sprint so the improved API and upgrade guidance ship with
the Rust release. B.3 consumes this classification contract for binding mappings.
These sprints share public contracts and release artifacts and are not
`parallel_safe`. Follow the phase merge-forward and parent-merge rules.

## Deliverables (authoritative)

1. Inventory all nine opaque error families (Identity, Init, Event, Flush,
   Shutdown, Projection, Subscriber, LogSink, Export), every production
   construction site, and every public function/trait exposing them across the
   four published core crates. Record the old-to-new symbol mapping, concrete
   variant/code table, adapters, serialization policy, and scoped API approval
   in `docs/plans/phase-b/error-api-contract.md`. Include custom sink/projector/
   subscriber implementations and error paths not emitted by built-in code.
2. Implement improved enum errors in the neutral types crate, with mandatory
   ErrorContext payloads, source/backtrace preservation and shared diagnostic
   classification. Create errors as typed variants at the failure site rather
   than routing new code through legacy wrappers or parsing messages. Preserve
   stable diagnostic codes. Use distinct new names to coexist with deprecated
   types; do not replace published structs in place.
3. Add improved public entry points and extension contracts using those errors.
   Preserve existing signatures and trait implementability through adapters;
   do not add required methods to existing consumer-implemented traits. Both
   paths use one implementation and preserve lifecycle, nonblocking admission,
   and failure behavior. Use Rust `#[deprecated(since = "V", note = "...")]`
   on legacy types/entry points, with an exact replacement in each note and V
   replaced by the selected minor release. Trait-only classification or
   documentation-only deprecation does not close this deliverable.
4. Migrate first-party production callers/examples to the improved path; retain
   explicit legacy compatibility fixtures. Limit deprecation allowances to
   compatibility modules/tests, never blanket-suppress them workspace-wide.
   Keep the bridge's accepted public signature/variant contract intact; any
   foreseeable bridge-facing requirement discovered while planning must be
   resolved with BTIT before the B.1 source gate, not scheduled as a later
   bridge public API change.
5. Extend `.claude/skills/sc-observability-adopting/SKILL.md` to route existing
   consumers to `references/migrate-error-api.md`. The reference covers version
   prerequisites, symbol mappings, before/after examples, custom trait adapters,
   replacing code-string comparisons with typed matches, preserving remediation,
   incremental upgrades, and compiler/test verification. Update API design,
   migration guide, changelog and release documentation. Do not instruct repos
   to suppress all warnings or introduce unwrap/expect/throw/raise handling.

## Contract direction

The sprint contract must enumerate all concrete replacements before its
implementation is accepted. The following shared trait is additive; existing
DiagnosticInfo implementations and sealing remain unchanged:

```rust
pub trait ClassifiedError: DiagnosticInfo {
    type Kind: Copy + Eq;
    fn kind(&self) -> Self::Kind;
}
```

Improved errors use names such as `InitFailure`, retain an ErrorContext in every
failure variant, and implement ClassifiedError and std::error::Error. Legacy
wrappers implement classification as a migration convenience. Keep this trait
and kind accessors supported through any future representation change.

Classification of externally constructed legacy values is total: an explicit
Unclassified kind retains the original diagnostic through the error object.
Unknown/custom codes never panic, become success, or lose metadata. Named
constructors enforce the variant/code association; arbitrary contexts entering
through compatibility adapters are validated/classified, not mislabeled.

New kind/variant extensibility must be explicit in the reviewed contract:
`#[non_exhaustive]` requires downstream fallback handling and does not force
callers to update on each added variant. Do not add it to existing public enums
in this minor release. Preserve the old types' Serde representation; new native
error serialization and versioned binding DTOs are separate contracts. Never
serialize native source objects/backtraces into language binding error payloads.

## Acceptance criteria (authoritative)

- AC1: The inventory accounts for every affected public boundary and production
  error constructor. Improved APIs expose typed failures and usable diagnostic
  classification without requiring caller string comparisons.
- AC2: An unchanged legacy consumer, including custom trait implementations,
  compiles and runs with default lint settings and emits actionable deprecation
  warnings. Its migrated counterpart passes with deprecated usage denied.
  Projects choosing `-D warnings` may fail on warnings; this is documented, not
  disguised as a source-signature break or silently suppressed.
- AC3: Both APIs preserve behavior, diagnostic/source data and legacy serialized
  fixtures. Custom/unknown legacy codes and adapter failure paths return values
  without panics or recursive logging. The imported bridge contract is unchanged.
- AC4: A downstream upgrade fixture follows the adoption reference successfully;
  public API/semver approval establishes an additive minor release. Documentation
  states that legacy removal and a 2.0 conversion are unscheduled.

## Required validation (authoritative)

Run workspace formatting, tests, doctests and clippy with warnings denied, using
only documented narrow compatibility allowances. Run existing public API diff,
semver, public API docs and docs-consistency checks and attach crate-specific
approvals. Add downstream compile/run fixtures for old and new entry points and
custom traits; assert deprecation diagnostic codes/replacement notes from Cargo
JSON output, not just successful compilation. Exercise failure classification,
unknown codes, metadata/source preservation, serialized compatibility and
behavior parity. Run the bridge's accepted API/behavior regression checks.
Record results in `docs/plans/phase-b/handoff-b-1a.md` during execution.

## Paths to delete

None. Legacy interfaces remain available.

## Non-closure

No legacy API removal, in-place struct-to-enum replacement, scheduled 2.0 release,
BTIT dependency switch, or post-copy bridge public API redesign. Publication
belongs to B.2. This sprint closes the additive migration scope, not all of #92.
