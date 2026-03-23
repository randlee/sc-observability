# Enforcement Strategy

This document defines how the `rust-best-practices` pattern set is enforced in this
repo across design review, implementation review, and CI.

## Purpose

Use this file when reviewing observability design and implementation work against the
tracked Rust best-practices patterns. The goal is to make enforcement predictable:

- design review catches API-shape and boundary mistakes before implementation
- code review catches implementation drift while code is still cheap to change
- CI blocks regressions that can be validated mechanically

## How To Use The Skill In This Repo

- Before implementation begins, run a design review against the approved document set
  and this enforcement strategy.
- During implementation, use the `rust-best-practices` skill in code review to check
  that crate boundaries, type safety, and error modeling match the approved design.
- In CI, enforce the subset of rules that can be checked mechanically. CI should gate
  merges on blocking findings and surface advisory findings for follow-up.

## Lifecycle Stages

### Design Review

Use design review to judge whether the API and architecture create the right extension
 points, type boundaries, and invariants before code exists.

### Code Review

Use code review to judge whether the implementation matches the approved design and
 whether the code uses the intended zero-cost and safety patterns.

### CI

Use CI to block mechanically detectable regressions such as forbidden dependencies,
 missing files, or rejected API shapes that can be tested automatically.

## Severity Meanings

- `Blocking`: must be fixed before the work can progress to the next lifecycle stage.
- `Important`: should be fixed before broad rollout; may be deferred only with explicit
  approval and documented follow-up.
- `Minor`: advisory only; track for cleanup when the touched area changes next.

## Pattern Matrix

### Newtype / Zero-Cost Types

Apply when:
- a public API uses semantic strings, IDs, prefixes, names, categories, or other
  primitive values with domain meaning
- a value has validation rules or should not be mixed with other values of the same
  primitive type

Design review:
- `Blocking`: new public APIs use bare primitives where type confusion would lock in
  avoidable API churn before first implementation release.
- `Important`: existing reviewed design still carries semantic primitives that should be
  converted before implementation begins.
- `Minor`: internal-only fields remain primitive where no boundary confusion exists.

Implementation review:
- `Blocking`: code lands with public semantic primitives that the approved design
  requires as newtypes.
- `Important`: internal conversions erase the newtype too early or bypass validation.
- `Minor`: helper code could avoid cloning/allocation but remains correct.

CI:
- `Blocking`: any mechanical check that the repo adopts for required newtypes fails.
- `Important`: audit scripts report new untyped public semantic fields.
- `Minor`: lint-like reporting for zero-cost cleanup opportunities.

Repo-specific open finding:
- `Blocking` before the first implementation sprint: define newtypes for
  `ToolName`, `EnvPrefix`, `ServiceName`, `TargetCategory`, and `ActionName`.
- `Advisory` at design stage only: the reviewed docs may temporarily name these as bare
  strings while the pre-implementation fix is being staged.

### Typestate

Apply when:
- the API models lifecycle transitions or invalid state combinations
- illegal transitions should be unrepresentable

Design review:
- `Blocking`: a stateful API is designed around runtime booleans or public mutable state
  where typestate is the intended invariant mechanism.
- `Important`: the state transitions are identified but not yet encoded cleanly.
- `Minor`: helper/accessor shape could better communicate the state model.

Implementation review:
- `Blocking`: invalid state transitions are possible in safe public code where the
  design requires typestate.
- `Important`: typestate exists but leaks escape hatches that undermine it.
- `Minor`: ergonomics around state transitions need cleanup.

CI:
- `Blocking`: compile-time tests or doctests proving state transitions fail.
- `Important`: state-machine examples drift from the approved contract.
- `Minor`: extra coverage requested around edge transitions.

### Sealed Traits

Apply when:
- a trait should remain implementation-controlled inside the crate
- downstream implementations would break invariants or future evolution

Design review:
- `Blocking`: the design leaves a trait open even though downstream implementations
  would make invariants unenforceable.
- `Important`: trait openness is ambiguous and not documented.
- `Minor`: the openness rationale exists but could be clearer.

Implementation review:
- `Blocking`: a trait that should be sealed is publicly implementable.
- `Important`: sealing exists but is incomplete or inconsistently applied.
- `Minor`: docs do not explain why the trait is sealed.

CI:
- `Blocking`: compile-time checks or review scripts detect missing seal boundaries where
  the design requires them.
- `Important`: doc checks show missing openness or sealedness statements.
- `Minor`: wording cleanup only.

### Error Context + Recovery

Apply when:
- an API returns errors across crate boundaries
- diagnostics are intended for both machine and human consumers

Design review:
- `Blocking`: the design permits bare errors without cause/remediation context where
  the repo standard requires structured error context.
- `Important`: remediation is present but not structurally enforced.
- `Minor`: display/JSON details need tightening.

Implementation review:
- `Blocking`: public errors bypass the structured error context model.
- `Important`: recovery guidance is inconsistent or hard-coded ad hoc.
- `Minor`: formatting or documentation drift only.

CI:
- `Blocking`: schema checks or audits fail for required structured error output.
- `Important`: error inventory or code registry checks fail.
- `Minor`: advisory lint output for weak recovery wording.

### Cow / Interior Mutability / Infallible

Apply when:
- APIs may benefit from borrowed-or-owned data without needless copies
- interior mutability is proposed for shared runtime state
- infallible code paths are represented as `Result` or vice versa

Design review:
- `Blocking`: the design introduces avoidable interior mutability or fallible APIs where
  invariants require stronger guarantees.
- `Important`: performance-sensitive paths are likely to incur unnecessary allocation or
  locking.
- `Minor`: potential ergonomic/performance refinement only.

Implementation review:
- `Blocking`: interior mutability or error shape choices undermine safety invariants or
  create hidden shared-state hazards.
- `Important`: code allocates or clones unnecessarily on hot paths.
- `Minor`: cleanup opportunities around `Cow`, borrowing, or `Infallible`.

CI:
- `Blocking`: targeted tests or lints for forbidden shared-state patterns fail.
- `Important`: profiling or audit scripts flag regressions on approved hot paths.
- `Minor`: optimization suggestions only.

## Stage Guidance By Pattern

### Design Review Blocking Patterns

Treat these as blocking during design review:
- missing newtypes for public semantic identifiers that would cause release-time churn
- missing typestate where invalid lifecycle transitions must be unrepresentable
- missing sealed-trait boundaries where downstream implementation would violate design
- missing structured error context/recovery requirements on public APIs

### Implementation Review Blocking Patterns

Treat these as blocking during implementation review:
- required newtypes not implemented before first implementation sprint
- typestate invariants broken in public APIs
- sealed traits accidentally left open
- structured errors bypassed in public interfaces
- unsafe interior mutability or incorrect fallibility in invariant-sensitive code

### CI Blocking Patterns

Treat these as blocking in CI when checks exist:
- forbidden dependency edges or crate-boundary violations
- missing required docs/files used by the review process
- compile-time/state-machine checks that enforce approved API invariants
- error-schema or repo-boundary validations that fail

## Repo-Specific Enforcement Notes

- The current open IMPORTANT finding on unvalidated semantic strings is advisory while
  the design is being finalized, but it becomes blocking before the first implementation
  sprint begins.
- This repo should use design review to settle crate boundaries first, then use code
  review and CI to prevent boundary drift during implementation.
- When a pattern cannot yet be enforced mechanically, document it here as design-stage
  or implementation-stage guidance so later CI work has a clear source of truth.
