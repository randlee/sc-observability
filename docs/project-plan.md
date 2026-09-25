# SC-Observability Project Plan

## Status

This repo is in post-1.0 maintenance and downstream-integration mode.

The workspace crates are already published. The current planning focus is:

- keep code, docs, and release procedures aligned with the shipped surface
- preserve the standalone crate boundaries defined by the normative docs
- maintain consumer-facing usability and downstream integration contracts
- stage new cross-repo work behind explicit review and QA

Historical recovery and pre-publish planning documents remain valuable
reference material, but they are no longer the controlling phase for current
work.

## npm publishing follow-up

Active: [npm scope and shared publishing adoption](plans/npm-publish-adoption/sprint.md). Lead: aobs; developer: cobs. Correct the active package identity and adopt reviewed shared publishing updates on a branch from develop, without another publication.

## Near-Term Work

1. Keep repo workflow and review discipline aligned with ATM.
2. Preserve the standalone crate boundaries defined in:
   - [`requirements.md`](./requirements.md)
   - [`architecture.md`](./architecture.md)
   - [`git-workflows.md`](./git-workflows.md)
   - [`publishing.md`](./publishing.md)
3. Maintain the consumer-facing docs and examples that prove the shipped public
   API remains usable for downstream adopters.
4. Keep release and publishing docs aligned with the fact that the workspace
   crates are already published and semver-governed.
5. Maintain extraction inventory and boundary ADRs as modules move from ATM to
   the standalone repo.
6. Keep ATM-specific adapter work outside the shared crates.
7. Maintain explicit downstream integration contracts for shipped consumers so
   cross-repo reviews do not rely on inferred layering or stale assumptions.
8. Keep completed sprint records archived without leaving root `docs/`
   cluttered.
9. Preserve the completed `v1.2.0` Phase A record and route new migration,
   additive API and binding work through the proposed Phase B plan.

## Issue #70 / v1.1.0

Retained-log rotation, pruning, and maintenance was the additive `v1.1.0`
logging-layer work item for `sc-observability`.

Historical sprint plan:

- [`archive/sprints/sprint-plan-retained-log-maintenance.md`](./archive/sprints/sprint-plan-retained-log-maintenance.md)

Historical sequence:

1. S1 docs sprint:
   - update `requirements.md`, `architecture.md`, and `project-plan.md`
   - define the retained-log policy surface, maintenance ownership, health
     reporting, and definitive shutdown contract
   - complete quality review before any code lands
   Exit criteria:
   - field names and defaults match
     `archive/sprints/sprint-plan-retained-log-maintenance.md`
   - normative docs lock `RetainedLogPolicy` as a struct nested in
     `LoggerConfig`
   - `LoggingHealthReport` and retained-log worker-state ownership are defined
     consistently across docs
   - no implementation code lands before docs review passes
2. S2 implementation sprint:
   - add the retained-log policy/config surface to `sc-observability`
   - move generic rotation/pruning maintenance into the logging layer
   - add health, shutdown, and integration tests
3. S3 consumer-doc sprint:
   - update `README.md` and/or `CONSUMING.md` with a retained-log policy
     configuration example once the implementation ships

## Thread Optimization / v1.2.0 Phase A

The completed logging-runtime workstream after retained-log maintenance was
the Phase A thread optimization effort, released in `v1.2.0`.

Controlling phase folder:

- [`plans/phase-a/readiness.md`](./plans/phase-a/readiness.md)
- [`plans/phase-a/sprint-A1.md`](./plans/phase-a/sprint-A1.md)
- [`plans/phase-a/sprint-A2.md`](./plans/phase-a/sprint-A2.md)
- [`plans/phase-a/sprint-A3.md`](./plans/phase-a/sprint-A3.md)
- [`plans/phase-a/sprint-A4.md`](./plans/phase-a/sprint-A4.md)

Planned sequence:

1. `A.1` architecture-and-API lock sprint:
   - lock the writer-thread architecture, queue semantics, health surface,
     `log()` / `try_log()` / deprecated `emit()` contract, and public-API
     governance requirements in the normative docs
2. `A.2` public-API governance sprint:
   - add CI-visible semver/public-API gates so future API changes cannot land
     silently
3. `A.3` runtime-and-API implementation sprint:
   - replace the dedicated maintenance-only worker model with a queue-backed
     writer runtime and implement the approved public logging API additions
4. `A.4` consumer rollout sprint:
   - update consumer docs and examples so new adopters use `log()` /
     `try_log()` and can surface queue/writer degradation in app health or
     `doctor` output

Phase A exit criteria:

- `A.1` through `A.4` each record an accepted commit and verdict in
  `docs/plans/phase-a/readiness.md`
- the final accepted runtime uses one writer-owned queue-backed logging model
  rather than a maintenance-only background worker
- public API changes are documented and CI-visible
- consumer docs teach `log()` / `try_log()` as the preferred logging APIs

## Phase B — Log bridge publication and language bindings

The proposed next lettered phase is tracked in
[`plans/phase-b/plan-phase-b.md`](./plans/phase-b/plan-phase-b.md).
Its first migration sprint copies the review-corrected generic BTIT crates.
Explicit prerequisite sprints implement/qualify additive core runtime elevation
and obtain accepted BTIT integration before that copy. Bounded core error
migration sprints add improved methods and warning-only legacy adapters before
Rust publication, followed by separately gated shared schema (B.3),
shared native runtime (B.3b), TypeScript/Tauri (B.3a), Python runtime (B.4) and Python distribution/platform
qualification (B.4a), then Python integration/async support and binding release.
Go remains future scope. The proposal does not reopen the accepted
Phase A closure or claim that BTIT's currently open review findings are resolved.

### B.3a — TypeScript client and Tauri host adapter

[B.3a](plans/phase-b/sprint-b-3a-typescript.md) implementation now includes the
generated TypeScript package, isolated Tauri adapter, IPC consumer example, and
binding validation handoff (tracking evidence in
[`handoff-b-3a.md`](plans/phase-b/handoff-b-3a.md)); its sprint record remains
the source of platform qualification and merge evidence. Real artifact/IPC
qualification (installed npm/Rust artifacts, cross-platform CI, and the broad
C05 consumer matrix) is recorded as passed in
[`handoff-b-3a-qualification.md`](plans/phase-b/handoff-b-3a-qualification.md).
Independent phase-end QA and formal API/ADR approval remain separate gates; this
qualification is not the B.7 registry-publication tail and is not waived by
[B.7's phase-end gate amendment](plans/phase-b/plan-phase-b.md#phase-end-completeness-gate-amendment-atm-qa-103).

### B.3b — Shared native binding runtime

Implementation and developer qualification are complete on
`feature/phase-b-3b-native-runtime` (worktree
`/Users/randlee/github/sc-observability-worktrees/feature/phase-b-3b-native-runtime`).
The [handoff](plans/phase-b/handoff-b-3b.md) records both native backends,
bounded helpers/operations/observers, all 26 contract cases in debug/release on
Linux/macOS/Windows, and the isolated packaged consumer. Public API scope is
independently reviewed; consolidated QA, owner-deferred contract acceptance and
later publication remains separate gates.

### B.P1 — Per-logger runtime level core

B.P1 implements the additive neutral runtime-level state, weak ownership, and
admission-outcome core described by
[`plans/phase-b/sprint-b-p1-runtime-core.md`](./plans/phase-b/sprint-b-p1-runtime-core.md).
It follows the execution-authorized runtime-level contract; B.P2 develops
from B.P1's pushed, independently verified implementation, and B.P1's PR
merges before B.P2's PR, to qualify staged artifacts. QA-4 independently verified 17/18 tracked B.P1 findings at
`a8951321e6b1df6044c2ee1f6f41c07a9dad7d99`; the remaining manual governance
hold was withdrawn by coordinating lead aobs following the owner's direction
to complete Phase B with publication delayed until the end. Public-API review
status remains proposed for public API review and is owner-deferred to Phase B
completion; historical QA verdicts remain
unchanged (QA-5 scoped checks satisfied; overall historical verdict FAIL), and
B.P1 remains unmerged and not live-published. Sprint `status: complete` denotes
implementation completion, with QA and merge state tracked separately.

### B.P2 — Staged runtime-level package qualification

B.P2 selects the `1.3.0` four-crate candidate, retains deterministic `.crate`
archive checksums and normalized package inventories, and validates separate
published-baseline and extracted-candidate consumers. Retained macOS/Linux/Windows
qualification artifacts passed for the recorded staged candidate; independent QA,
merge, live publication, and B.7 registry-only re-proof remain pending. B.P2
does not publish to crates.io. B.7 owns the phase-end live release and
registry-only consumer proof.

### B.P3 — Accepted BTIT runtime-level bridge integration

[B.P3](plans/phase-b/sprint-b-p3-runtime-btit.md) owns BTIT's integration and
source acceptance of B.P2's staged runtime-level capability before B.1's
mechanical copy. [`handoff-b-p3.md`](plans/phase-b/handoff-b-p3.md) records
the accepted verdict: BTIT source `396a9d9f77ca1950eeb92d4f88c0eecadb5ef00b`,
independent QA2 reporting 16/16 deliverables complete with zero remaining
findings, and lead verification against retained CI run `35190374497`
(13/13 jobs). Source acceptance does not grant merge or publication; runtime
public-API acceptance remains owner-deferred to Phase B completion, and B.1
independently re-verifies every copied file blob against this handoff's
immutable commit. `sprint-b-p3-runtime-btit.md`'s own frontmatter `status`
still reads `proposed`, tracking the BTIT-repository review process rather
than this accepted-source outcome; it is not reconciled by this entry.

### B.1a — Neutral typed failure preparation

The scoped neutral preparation layer is recorded in
[`plans/phase-b/task-b-1a-neutral-prep.md`](./plans/phase-b/task-b-1a-neutral-prep.md).
It adds opt-in typed failure values and explicit resolver/subscriber/projector
adapters inside `sc-observability-types`, while retaining the published root
APIs and legacy serialization. Runtime adoption, copied-bridge reconciliation,
and warning policy remain owned by the subsequent B.1 layers.

### B.1b — Typed logger preparation

The scoped logger preparation layer is tracked in
[`plans/phase-b/task-b-1b-logger-prep.md`](./plans/phase-b/task-b-1b-logger-prep.md).
It adds opt-in typed logger construction, admission and sink interoperability
while retaining every existing logger entry point and its bridge behavior.
Warning rollout, copied-bridge integration, observation/telemetry adoption,
publication, and B.1 closure remain separately gated.

### B.1c — Typed observation preparation

The scoped observation preparation layer is tracked in
[`plans/phase-b/task-b-1c-observation-prep.md`](./plans/phase-b/task-b-1c-observation-prep.md).
It adds typed configuration, construction, observation routing, flush, and
shutdown entry points over the existing `sc-observe` runtime while exercising
the neutral subscriber/projector adapters through unchanged registration
boundaries. Full B.1c integration, copied-bridge acceptance, and independent
QA remain pending.

### B.1d — Typed telemetry preparation

The scoped telemetry preparation layer is tracked in
[`plans/phase-b/task-b-1d-telemetry-prep.md`](./plans/phase-b/task-b-1d-telemetry-prep.md).
It adds opt-in typed OTLP configuration, assembly, construction, flush, and
shutdown operations while retaining existing exporters, telemetry lifecycle,
projector registration, serialization, and public error surfaces. Copied-bridge
integration, warning rollout, publication, and independent QA remain separate.

### B.1a/B.1b — QA1 reconciled corrections

The six scoped QA1 corrections are recorded in
[`plans/phase-b/task-b-1ab-qa1-fixes.md`](./plans/phase-b/task-b-1ab-qa1-fixes.md).
They harden test-only concurrency controls, preserve standalone sink diagnostic
remediation/source semantics, and use the canonical typed identity code without
changing the retained logger configuration, shutdown behavior, or root exports.
Coordinator completeness passed at
`57176ba4c263a6cf603ef4d78a7e3be94b5d1568`; independent QA remains separate.

### B.1 — Copy the corrected generic BTIT crates

The mechanical copy sprint is tracked in
[`plans/phase-b/sprint-b-1-copy.md`](./plans/phase-b/sprint-b-1-copy.md). It
copies `crates/sc-observability-log`, `crates/sc-observability-log-macros`,
and CI-only `crates/sc-observability-log-consumer-check` from the accepted
BTIT source `396a9d9f77ca1950eeb92d4f88c0eecadb5ef00b` (`docs/plans/phase-b/handoff-b-p3.md`),
wires them into the workspace with `publish = false`, and records exact
per-file provenance and permitted mechanical adaptations in
`docs/plans/phase-b/import-provenance.json`, verified by
`scripts/ci/validate_log_import.py`. All 85 copied source files are
byte-identical to the accepted source by Git blob ID; the only content
adaptations are workspace-inherited `[package]` metadata and four
toolchain-drift `trybuild` `.stderr` fixtures (this workspace pins Rust
`1.94.1`, BTIT pins `1.98.1`), recorded as a disposition in the sprint doc.
Independent QA and API approval remain pending; publication remains deferred
to B.7.

### B.1e — Typed error migration and warning rollout

The completed implementation and validation layer is tracked in
[`plans/phase-b/task-b-1e-migration-prep.md`](./plans/phase-b/task-b-1e-migration-prep.md)
and is based on the authoritative
[`sprint-b-1e-error-adoption.md`](./plans/phase-b/sprint-b-1e-error-adoption.md)
and [error contract](./plans/phase-b/error-api-contract.md). It records the
exact legacy-wrapper and method replacements, supported owner-constructor
exemptions, typed matching/source-retention guidance and narrow warning policy
against the merged B.1d API. The implementation activates the nine wrapper
and 20 method warnings at `1.4.0`, migrates ordinary routing call sites,
preserves narrow compatibility boundaries, and validates legacy/migrated/
partial external Cargo consumers with JSON diagnostics and a Serde golden.
B.2 qualification and later publication remain separately gated; no removal
schedule is introduced. Publication follows Phase-C `sc-publish` migration and
separate authorization.

### B.1 integration — Combined B.1a-B.1d reconciliation and registry parity

The integration layer closing the preparation sprints is tracked in
[`plans/phase-b/task-b-1-integration.md`](./plans/phase-b/task-b-1-integration.md),
built on the merged B.1a-B.1e preparation layers and cobs's pushed
`fix/phase-b-1c-qa1` observation fixes. It replaces the preliminary
`error-api-inventory.md` with a checked nine-family inventory (every
production constructor, feature-gated path, open trait, private exporter, and
copied-bridge use, each with an explicit typed-production or named-
compatibility disposition) backed by an executable workspace parity test
(`crates/sc-observability-otlp/tests/error_registry_parity.rs`) that asserts
every typed constructor's diagnostic code against its owning crate's
`error_codes` registry constant. It also addresses a genuine gap the B.1e
warning activation exposed: the frozen B.1 BTIT bridge import legitimately
still uses the newly-deprecated legacy wrapper types (`IdentityError` in
`mapping.rs`; `InitError`/`FlushError` via `Logger::new`/`Logger::flush` in
`handle.rs`; `EventError` via `Logger::try_log_with_outcome`'s `TryLogError`
compatibility path in `control.rs`/`handle.rs`) and cannot be edited without
violating `import-provenance.json`'s
pinned source bytes. A workspace- or bridge-wide clippy `-A deprecated`
suppression was rejected as too broad; the landed replacement is narrow,
per-call-site `#[allow(deprecated, reason = ...)]` annotations recorded as
post-import adaptations in a separate `docs/plans/phase-b/
post-import-adaptations.json` manifest (owned by lobs), validated via
`validate_log_import.py --post-import-adaptations`, without altering
`import-provenance.json`'s original pinned import manifest.
`.github/workflows/ci.yml`'s clippy job remains a single `-D warnings` step.
Broader AC-by-AC reconciliation and the four handoffs' final integration-status
update are complete; independent QA/coordinator completeness review remains
pending. This entry does not itself claim that review.

### B.1 provenance-prep — Import/acceptance validator built ahead of B.1

[`plans/phase-b/task-b-1-provenance-prep.md`](./plans/phase-b/task-b-1-provenance-prep.md)
built and proved `scripts/ci/validate_log_import.py` (B.1 deliverable 3) in
parallel with B.P3's then-active source corrections, ahead of B.1 having an
accepted source to copy. At the time it was preparation tooling only: no
BTIT source was copied, no source approval was granted, and no real
`import-provenance.json` existed. B.P3's source handoff has since been
accepted, and the B.1 copy section above records that this validator's real
`import-provenance.json` now exists and B.1 has copied the accepted source.

### B.7 — Binding publication readiness machinery (publication deferred beyond Phase B)

The phase-end publication sprint is tracked in
[`plans/phase-b/sprint-b-7-publish-bindings.md`](./plans/phase-b/sprint-b-7-publish-bindings.md),
built from `feature/phase-b-6-python-async`. **Owner sequencing correction:
no mid-phase publication.** B.P2/B.2 are reviewed immutable release
candidates, B.3-B.6 and these bindings consume prepublication bundles, and
B.7 readiness packet covers all of Phase B (core, bridge/macros, and bindings)
-- but Phase B records readiness only. Until the
phase merges, no publication workflow is dispatched and no registry
credentials are sought. Phase C migrates and preflights `sc-publish`; separate
owner authorization follows, then BTIT adopts the published artifacts.

This entry records the review-readiness machinery built instead:
`release/bindings-artifacts.toml` (manifest for the 4 binding crates.io
crates plus the PyPI and npm packages, parallel to the existing
`release/publish-artifacts.toml`), `scripts/release_bindings_artifacts.py`
(`validate-manifest`/`verify-versions`/`list-publish-plan`, plus
`build-evidence`/`verify-evidence` for real rebuildable candidate artifact
hashes), `scripts/ci/validate_binding_registry_consumers.sh` (real isolated
Rust/Python/TypeScript consumer-matrix checks against those built artifacts,
never a live registry), a pytest/unittest negative-path harness (29 cases,
`scripts/ci/tests/test_release_bindings_artifacts.py` and
`test_bindings_evidence.py`), and
[`plans/phase-b/handoff-b-7.md`](./plans/phase-b/handoff-b-7.md), which is
the review packet. `.github/workflows/release.yml` installs none of this:
it keeps only the 6-core-crate `gate-and-tag`/`publish`/`release` jobs that
predate B.7.

`sc-observability-tauri` (qualification passed; independent phase-end QA/API
approval remains pending) and the npm client (awaiting owner-deferred
sc-publish credential provisioning and owner publication approval) are correctly
and honestly reported as `pending` with named reasons, not treated as failures or
given fabricated placeholder files. Live npm/PyPI credential state is not
asserted by this review packet, and
the crates.io/PyPI/npm name-preflight only proved the 6 target names are
currently unclaimed, not that namespace control is secured. This sprint is
not marked complete; producing this review packet is not sprint closure. It
prepares B.7 for its actual phase-end publication pass, deferred (not
cancelled) until owner review authorizes it.

### B.5 — Python logging and mixed-language context

B.5 implements the explicit standard-library logging handler, Result-returning
request scopes, typed examples, and shared Rust/Python request correlation in
[`plans/phase-b/sprint-b-5-python-integration.md`](./plans/phase-b/sprint-b-5-python-integration.md).
Implementation and installed distribution qualification are active on
`feature/phase-b-5-python-integration`; package publication remains B.7.
### B.2 — Six-package Rust qualification

[B.2 implementation](plans/phase-b/sprint-b-2-publish-rust.md) stages the six
public Rust packages at `1.4.0`, preserving historical B.P2 artifacts and B.1
source provenance. The [handoff](plans/phase-b/handoff-b-2.md) pins the final
candidate source, archives, normalized manifests, isolated platform consumers
and scoped API approval. The [checklist](plans/phase-b/checklist-b-2-qualification.md)
separates implementation/verification from lead completeness and independent
QA. Publication and the later registry-only consumer proof remain B.7 gates.

### B.3 — Shared neutral DTO/schema

B.3 implements `sc-observability-dto`, checked core conversions, the canonical
input/output schema, schema-only TypeScript/Python projections, frozen conformance
fixtures and the real isolated source-bundle helper. The implementation handoff
is [`plans/phase-b/handoff-b-3.md`](./plans/phase-b/handoff-b-3.md), including exact
qualified B.2 archive provenance and the lead's crate-specific wire API approval.
Implementation validation passed; consolidated Phase B QA/ordered merges and
B.7 registry-only publication proof remain separate.

### B.6 — Python immediate receipts and optional asyncio observation

[B.6](plans/phase-b/sprint-b-6-python-async.md) adds synchronous context-aware
`submit`, caller-owned resolved receipts and bounded loop-local `flush_async`
observation for owned and attached backends. The
[handoff](plans/phase-b/handoff-b-6.md) and
[checklist](plans/phase-b/checklist-b-6.md) retain implementation, exact-source
local checks and remaining gates. The final B.4/B.5/B.6 combined candidate must
pass one immutable five-wheel/25-cell matrix, including actual embedded hosts
per interpreter. Lead completeness, consolidated QA, ordered parent merges and
later publication remains separate; no intermediate local result closes the matrix.

### B.4 — Owned and host-attached Python runtime

[B.4 Python runtime](plans/phase-b/sprint-b-4-python.md) provides the locked
`abi3-py310` PyO3 owned and Rust-host-attached logging surface, typed `Ok`/`Err`
facade, and the Rust `rlib` embedding surface. Its
[handoff](plans/phase-b/handoff-b-4.md) records the tested implementation,
source-wheel runtime/conformance gates, private companion-only fault proofs and
the public API digest. B.4a separately owns wheel/sdist and platform
qualification; B.7 retains publication.

### B.4a — Python distributions and platform qualification

[B.4a](plans/phase-b/sprint-b-4a-python-packaging.md) builds a self-contained
sdist through B.3's source-bundle helper, freezes outer extension/Rust-host
locks against that layout, and qualifies the built wheels across the platform
matrix. Its [handoff](plans/phase-b/handoff-b-4a.md) records the packaging
approach; no package is published from this sprint. Sprint `status:
in_progress` is accurate qualification pending, not a publication gap:
PHB-CI-002 (sdist manifest fix, landed on PR#144) and the still-open PHB-CI-003
(packaged wheel test-hook symbol failure, assigned to
`fix/phase-b-wheel-test-hooks`) are both non-publication CI findings that must
resolve, independent of and before B.7's separately owner-deferred
registry-publication tail.

## Phase C — Shared publishing migration

Status: in progress (owner-authorized execution). Lead: `aobs`. Integration:
`integrate/phase-c`; C.1: `fix/phase-c-1-shared-pipeline-migration`; C.2:
`fix/phase-c-2-release-surface-preflight` (depends on C.1). Publication remains
separately authorized; upstream prerequisites still gate installation.

The proposed next lettered phase is tracked in
[`plans/phase-c/plan-phase-c.md`](./plans/phase-c/plan-phase-c.md). It replaces
this repo's bespoke publishing implementation with the shared `../sc-publish`
package and preflights the complete Phase B release surface (Rust crates,
Python wheels/sdist, the npm client, and applicable native/Tauri artifacts)
through that shared pipeline. It is planning-and-preflight scope only: it does
not authorize publication, tag creation, or BTIT repository integration tests,
and its planning branch does not execute until Phase B merges into `develop`.
C.2 implementation and nonpublishing qualification are complete, with lead
completeness PASS recorded on 2026-09-19 in
[`final-c2-audit.md`](plans/phase-c/evidence/final-c2-audit.md).
Current Python qualification passed all 33 jobs; native qualification, package
completeness, required environment scopes, and actual release preflight passed.
The final documentation correction awaits independent QA registry closure.
The shared installer and immutable sc-lint source pins are adopted; npm scope
is configured. The owner separately authorized only the annotated candidate tag
recorded in the audit. Phase C integration/ordered merges and publication remain
separate from completed sprint implementation; nothing here authorizes release
publication or BTIT post-publication integration tests. See
`docs/requirements.md` §11 and
[ADR-016](architecture.md#adr-016-shared-publishing-pipeline-adoption).

## Rule

Any sprint plan added here must preserve the standalone boundary defined by:

- `docs/requirements.md`
- `docs/architecture.md`
- `docs/git-workflows.md`
- `docs/publishing.md`

## Implementation Planning Set

Current maintenance and integration work should use these planning documents
together:

- `docs/pre-publish-recovery-plan.md`
- `docs/implementation-plan.md`
- `docs/public-api-checklist.md`
- `docs/test-strategy.md`
- `docs/archive/sprints/sprint-plan.md`
- `docs/release-readiness-checklist.md`

## Historical Recovery Baseline

The earlier recovery program remains the historical baseline for these planning
principles:

1. fix correctness and best-practice gaps first
2. ship the missing approved API surface before higher-layer expansion
3. close incomplete design elements in dedicated sprints
4. repeat the design-closure loop until no unresolved issue remains

The detailed sprint-by-sprint execution record remains in
[`pre-publish-recovery-plan.md`](./pre-publish-recovery-plan.md) for reference.

## Phase D — Host logging, OTLP restoration, and distribution completion

The finalized Phase D plan is
[`docs/plans/phase-d/plan-phase-d.md`](plans/phase-d/plan-phase-d.md). Its
streams are:

1. Parallel-safe [D.1 startup settings](plans/phase-d/sprint-d-1-log-settings.md), [D.2 host logger bridge](plans/phase-d/sprint-d-2-host-logger-bridge.md), and [D.3 1.x typed-sink bridge](plans/phase-d/sprint-d-3-typed-sink-registration.md)
2. [D.4 2.0 error enums and release baseline](plans/phase-d/sprint-d-4-error-enums-2-0.md) → [D.5 OTLP signal model](plans/phase-d/sprint-d-5-otlp-signal-model.md) → [D.6 lifecycle core](plans/phase-d/sprint-d-6-otlp-lifecycle-core.md) → parallel [D.7 SDK adapter](plans/phase-d/sprint-d-7-otlp-sdk-tokio.md) and [D.8 legacy transplant](plans/phase-d/sprint-d-8-otlp-http-json-transplant.md) → [D.9 conformance](plans/phase-d/sprint-d-9-otlp-conformance.md)
3. [D.10 Windows ARM64 wheel](plans/phase-d/sprint-d-10-windows-arm64-wheel.md) → [D.11 open-ended Python metadata guard](plans/phase-d/sprint-d-11-python-open-ended-guard.md)

The detailed plan names branches, worktrees, agents, and the final integration
step. The logging trio is serial only because cobs owns it; its artifacts are
otherwise parallel-safe. D.4 precedes all 2.0 OTLP work.

Only concrete code-consumption edges are `must_follow`; D.4 alone owns release
baseline alignment before its children begin, while final integration is
merge-only. D.1–D.3 remain additive against the 1.4.1
baseline. Planning
or implementation does not authorize a release, tag, registry publication,
or downstream `atm-core` change. Issue #88 (Python OTEL/structured logging)
is expressly excluded.

## Consumer Usability Baseline

This follow-up work defines the minimum consumer-facing usability baseline for
the shipped public API. It remains relevant after the initial release because
downstream adopters still depend on these entrypoints and examples staying
accurate.

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

## Downstream Integration Documentation

The repo also needs stable downstream integration guidance for adjacent repos
that integrate against the shipped public API.

1. `sc-compose` logging-only integration contract
   Exit criteria:
   - `requirements.md` and `architecture.md` explicitly state the exact split
     between `sc-observability-types` and `sc-observability`
   - the docs explicitly scope this work to simple logging-only integration and
     explicitly defer OTel expansion
   - the docs state that `sc-composer` keeps a local observer layer and does
     not depend on `sc-observability-types`
   - the docs define the minimum local observer interface shape, event source,
     and `dyn`-compatible injection model required for the downstream adapter
   - the docs state that `sc-compose` constructs `Logger`, applies the
     file/console sink policy, uses `Logger::health()` for health reporting,
     and calls `Logger::shutdown()` on exit
   - the docs explicitly define the no-op fallback path when no observer or
     logger-backed adapter is installed
   - the docs define the adapter-owned mapping from `sc-compose` local observer
     events to `LogEvent` fields, including command lifecycle events and
     `message` guidance
   - the docs identify the planned downstream `sc-compose observability-health`
     CLI surface precisely enough for implementation and review
   - `qm-comp` cross-document consistency review passes; all three docs are
     confirmed mutually consistent before merge

Follow-up: [complete immutable installer adoption](plans/npm-publish-adoption/followup.md), lead aobs, developer cobs; child of PR199.
