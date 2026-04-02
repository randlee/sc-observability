# SC-Observability Project Plan

## Status

This repo remains in extraction and pre-publish recovery mode.

The current controlling phase is the pre-publish recovery program in
[`pre-publish-recovery-plan.md`](./pre-publish-recovery-plan.md). No publish
work should proceed from this repo until that plan is complete.

The immediate goal is no longer "ship quickly". The immediate goal is:

- restore truth between docs, code, and release gates
- close the missing query/follow API surface
- ship a real OTLP attachment surface
- remove remaining public design ambiguity before publish

## Near-Term Work

1. Keep repo workflow and review discipline aligned with ATM.
2. Preserve the standalone crate boundaries defined in:
   - [`requirements.md`](./requirements.md)
   - [`architecture.md`](./architecture.md)
   - [`git-workflows.md`](./git-workflows.md)
   - [`publishing.md`](./publishing.md)
3. Complete the pre-publish recovery sprints in strict order:
   - truth reset and design freeze
   - shared contract hardening
   - logging query/follow runtime
   - routing and OTLP attachment closure
   - hardening and final publish gate
4. Resume release-readiness work only after the final recovery review reports
   zero blocking findings.
5. Maintain extraction inventory and boundary ADRs as modules move from ATM to
   the standalone repo.
6. Keep ATM-specific adapter work outside the shared crates.

## Rule

Any sprint plan added here must preserve the standalone boundary defined by:

- `docs/requirements.md`
- `docs/architecture.md`
- `docs/git-workflows.md`
- `docs/publishing.md`

## Implementation Planning Set

The current implementation phase should use these planning documents together:

- `docs/pre-publish-recovery-plan.md`
- `docs/implementation-plan.md`
- `docs/public-api-checklist.md`
- `docs/test-strategy.md`
- `docs/sprint-plan.md`
- `docs/release-readiness-checklist.md`

## Pre-Publish Recovery Phase

The current phase is not a release cut. It is a recovery program that:

1. fixes correctness and best-practice gaps first
2. ships the missing approved API surface
3. closes incomplete design elements in dedicated sprints
4. repeats the design-closure loop until no unresolved issue remains

The detailed sprint-by-sprint execution order is defined in
[`pre-publish-recovery-plan.md`](./pre-publish-recovery-plan.md).
