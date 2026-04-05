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
   - Sprint 0 truth reset and design freeze
     Exit criteria: [`pre-publish-recovery-plan.md`](./pre-publish-recovery-plan.md) §5.4
   - Sprint 1 shared contract hardening
     Exit criteria: [`pre-publish-recovery-plan.md`](./pre-publish-recovery-plan.md) §6.5
   - Sprint 2 logging query/follow runtime
     Exit criteria: [`pre-publish-recovery-plan.md`](./pre-publish-recovery-plan.md) §7.6
   - Sprint 3 routing and OTLP attachment closure
     Exit criteria: [`pre-publish-recovery-plan.md`](./pre-publish-recovery-plan.md) §8.6
   - Sprint 4 hardening and final publish gate
     Exit criteria: [`pre-publish-recovery-plan.md`](./pre-publish-recovery-plan.md) §9.4
4. Resume release-readiness work only after the final recovery review reports
   zero blocking findings.
5. Execute the post-Phase-2 pre-publish usability sprint before cutting the
   1.0 release candidate so the shipped public logging surface is usable and
   documented for downstream adopters.
6. Maintain extraction inventory and boundary ADRs as modules move from ATM to
   the standalone repo.
7. Keep ATM-specific adapter work outside the shared crates.

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

## Post-Recovery Pre-Publish Usability Sprint

This follow-up sprint is plan-approved work that executes after the current
pre-publish recovery program closes and before the 1.0 release candidate is
cut. It exists to address consumer-facing usability and validation gaps that
are release-blocking but intentionally separate from the earlier runtime
recovery sprints.

1. Consumer onboarding sprint (`#20`)
   Exit criteria:
   - `README.md` is a real consumer entrypoint with crate-selection guidance
     and a minimal logging-only snippet
   - root `CONSUMING.md` documents logging-only setup, default paths,
     `SC_LOG_ROOT`, sink toggles, custom sink registration, and `Logger::health()`
   - `examples/custom-sink-example/` exists and compiles against the public API only
   - consumer-facing default sink/path/environment behavior is documented
2. Default file sink path cleanup (`#21`)
   Exit criteria:
   - the default file sink layout is simplified to
     `<log_root>/logs/<service>.log.jsonl`
   - all user-facing docs, examples, and tests reflect the new layout
   - any migration note for the old nested path is documented before release
3. Console sink writer parity (`#55`)
   Exit criteria:
   - `ConsoleSink::stderr()` is added as a public companion to `stdout()`
   - the public writer-selection surface is explicitly limited to stdout/stderr
   - consumer docs include the stdout/stderr selection guidance
4. Retained-sink fault-injection sprint (`#57`)
   Exit criteria:
   - a public retained-sink fault-injection surface exists for live validation
     of `degraded` and `unavailable` sink health states
   - the hook is intentionally gated for validation use and lives in the
     retained-sink layer, not the query/follow layer
   - docs explain how downstream consumers exercise the same failure paths they
     rely on in production health checks
