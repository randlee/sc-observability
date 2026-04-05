# Phase 2 Production Readiness Review

**Status**: Review complete, remediation required
**Branch**: `phase-2-prod-readiness`
**Verdict**: `FAIL` / not production-ready yet
**Applies to**: `sc-observability-types`, `sc-observability`, `sc-observe`, `sc-observability-otlp`

## 1. Review Scope

This review covered the shipped public and externally reachable runtime surface
for all four shared crates, with focus on:

- correctness
- error handling
- public API quality
- panic behavior
- platform behavior
- dependency hygiene
- safety
- test coverage gaps
- documentation completeness

Review method:

- read the controlling contract docs in `docs/requirements.md`,
  `docs/architecture.md`, `docs/pre-publish-recovery-plan.md`, and
  `docs/cross-platform-guidelines.md`
- inspect public API definitions and runtime entry points across all four crates
- inspect known-risk call sites for panic/discard/invariant masking patterns
- run `cargo rustdoc -p <crate> -- -Dmissing-docs` for each crate to find
  documentation gaps on the shipped public surface

Areas reviewed with no new findings:

- no `unsafe` usage was found in the shared crates during this review
- `sc-observability-otlp` currently keeps `sc-observe` as a dev-only
  dependency; no runtime dependency-layering violation was found in
  `Cargo.toml`

## 2. Production Readiness Verdict

The Phase 2 codebase is close to release-candidate quality, but it is not yet
production-ready.

Release should remain blocked until:

1. Sprint 4 closure accounting is made truthful again
2. runtime correctness blockers are fixed
3. the public surface is either documented or intentionally reduced
4. platform limitations are either eliminated or explicitly downgraded from
   promised semantics before release
5. the remaining important gaps are closed or formally deferred with updated
   release criteria

## 3. Finding Inventory

### 3.1 Blocking Findings

#### PRR-B-001 / `BP-ECR-002`

- Severity: `blocking`
- Category: error handling
- Crate: `sc-observability-otlp`
- Location: `crates/sc-observability-otlp/src/lib.rs:716-723`
- Issue: `Telemetry::shutdown()` intentionally returns `Result<(), ShutdownError>`,
  but it discards the `flush()` result with `let _ = self.flush();`. Export
  failures are recorded only in health state, so shutdown callers cannot
  distinguish a clean shutdown from a failed export flush.
- Required fix:
  return a structured shutdown error when the flush path fails, or downgrade the
  public contract so the API no longer implies flush success can be observed.
- Breaking API change: `no`

#### PRR-B-002 / `REQ-QA-003`

- Severity: `blocking`
- Category: public API quality
- Crate: `sc-observability`
- Location: `crates/sc-observability/src/constants.rs:5-11`
- Issue: seven constants are exported publicly even though they are crate-local
  implementation defaults rather than supported public API:
  `DEFAULT_LOG_QUEUE_CAPACITY`, `DEFAULT_ROTATION_MAX_BYTES`,
  `DEFAULT_ROTATION_MAX_FILES`, `DEFAULT_RETENTION_MAX_AGE_DAYS`,
  `DEFAULT_ENABLE_FILE_SINK`, `DEFAULT_ENABLE_CONSOLE_SINK`, and
  `REDACTED_VALUE`.
- Required fix:
  reduce visibility to `pub(crate)` or move them behind explicit supported
  configuration accessors if they must remain public.
- Breaking API change: `yes`

#### PRR-B-003

- Severity: `blocking`
- Category: documentation
- Crate: `sc-observability-types`
- Location:
  `crates/sc-observability-types/src/lib.rs:377-378,405-427,523-556,651-689,712-734,851-881,892,900,908,916-930,939-952`;
  `crates/sc-observability-types/src/errors.rs:73-86`;
  `crates/sc-observability-types/src/health.rs:10-12,18-20,26-40,46-55,61-72,78-98,103,109-115`;
  `crates/sc-observability-types/src/query.rs:16-17,23-24,40-50,85-86,93-101`
- Issue: `cargo rustdoc -p sc-observability-types -- -Dmissing-docs` fails on
  a large portion of the exported surface, including public fields, enum
  variants, trait methods, and shared query/health contracts.
- Required fix:
  add complete rustdoc coverage for all exported items and add a CI/readiness
  gate that keeps `-Dmissing-docs` green for this crate.
- Breaking API change: `no`

#### PRR-B-004

- Severity: `blocking`
- Category: documentation
- Crate: `sc-observability`
- Location:
  `crates/sc-observability/src/lib.rs:41-42,57,77-79,115-116,135-145`;
  `crates/sc-observability/src/constants.rs:5-11`;
  `crates/sc-observability/src/error_codes.rs:5-15`
- Issue: `cargo rustdoc -p sc-observability -- -Dmissing-docs` fails on public
  policy/config fields, public constants, and error-code exports.
- Required fix:
  complete rustdoc for the public logging surface and keep it gated in CI.
- Breaking API change: `no`

#### PRR-B-005

- Severity: `blocking`
- Category: documentation
- Crate: `sc-observe`
- Location: `crates/sc-observe/src/lib.rs:34-39`
- Issue: `cargo rustdoc -p sc-observe -- -Dmissing-docs` fails because
  `ObservabilityConfig` still exposes undocumented public fields.
- Required fix:
  document every public config field and add missing-docs gating for the crate.
- Breaking API change: `no`

#### PRR-B-006

- Severity: `blocking`
- Category: documentation
- Crate: `sc-observability-otlp`
- Location:
  `crates/sc-observability-otlp/src/lib.rs:33-35,41-51,75,81,95,109-110,131-136,238-239,396,401,406`
- Issue: `cargo rustdoc -p sc-observability-otlp -- -Dmissing-docs` fails on
  public enum variants, config fields, and exporter trait methods.
- Required fix:
  complete rustdoc for the shipped OTLP surface and keep it gated in CI.
- Breaking API change: `no`

#### PRR-B-007 / `PHASE-END-B-001`

- Severity: `blocking`
- Category: platform behavior
- Crate: `sc-observability`
- Location:
  `crates/sc-observability/src/query.rs:350-364`;
  `crates/sc-observability/src/lib.rs:1546-1596`
- Issue: the non-Unix file identity fallback uses only `len` and
  `modified_nanos`. The shipped test suite explicitly ignores recreate coverage
  on Windows and accepts replay after truncate there. That does not meet the
  documented cross-platform follow semantics for rotation/truncation correctness.
- Required fix:
  either implement a stronger Windows identity strategy that satisfies the
  documented follow contract, or change the release contract and docs so Windows
  behavior is explicitly degraded and non-promissory before publish.
- Breaking API change: `no`

#### PRR-B-008

- Severity: `blocking`
- Category: documentation / release accounting
- Location: `docs/pre-publish-recovery-plan.md:345-402`
- Issue: the controlling recovery plan currently claims Sprint 4 "closed" the
  carried finding set in §9.5 even though Sprint 4 never executed. That leaves
  release accounting in an untrustworthy state for the following IDs:
  `QA-001`, `BP-ST-001`, `BP-ST-002`, `BP-TS-001`, `BP-TS-002`,
  `BP-IMC-001`, `BP-IMC-002`, `BP-NT-003`, `BP-NT-004`, `BP-NT-005`,
  `BP-ECR-001`, `BP-ECR-002`, `BP-ECR-003`, `REQ-QA-008-phase`, and
  `REQ-QA-009-phase`.
- Required fix:
  replace the optimistic closure text with a per-ID reconciliation table that
  maps each carry-over tag to one of:
  `fixed with evidence`, `still open`, or `explicitly deferred`. Per the
  addendum for this review, only `BP-TS-001` and `BP-TS-002` may remain
  deferred post-publish.
- Breaking API change: `no`

### 3.2 Important Findings

#### PRR-I-001 / `BP-NAMES-002`

- Severity: `important`
- Category: public API quality
- Crate: `sc-observability-types`
- Location: `crates/sc-observability-types/src/health.rs:101-103`
- Issue: `TelemetryHealthProvider` reads like an OTLP-owned/provider-specific
  abstraction, but the current design uses it as the generic routing-layer
  health bridge. The name is narrower than its actual architectural role.
- Required fix:
  rename to a more architecture-accurate provider name before publish, or
  explicitly document why the current name is intentionally retained.
- Breaking API change: `yes`

#### PRR-I-002 / `BP-NAMES-003`

- Severity: `important`
- Category: public API quality
- Crate: `sc-observability-types`
- Location: `crates/sc-observability-types/src/lib.rs:888-892`
- Issue: `ObservationSubscriber::handle(...)` is the odd one out beside
  projector/filter naming and does not communicate dispatch semantics as clearly
  as the rest of the surface.
- Required fix:
  rename the method to the final approved verb before publish, or explicitly
  freeze `handle` as the supported name in the API docs and checklist.
- Breaking API change: `yes`

#### PRR-I-003

- Severity: `important`
- Category: panics
- Crates: `sc-observability`, `sc-observability-otlp`, `sc-observe`
- Location:
  `crates/sc-observability/src/lib.rs:325-347`;
  `crates/sc-observability-otlp/src/lib.rs:598-607,621,653-657,663-665,724-726`;
  `crates/sc-observe/src/lib.rs:326-338,356-374`
- Issue: several public methods still panic on poisoned mutexes or internal
  type-erasure invariants. The invariant panics on registration are documented,
  but the runtime mutex-poison panics remain implicit and inconsistent across
  crates.
- Required fix:
  convert runtime poisoning cases to structured errors where practical, and add
  explicit `# Panics` docs for any public panic that remains intentional.
- Breaking API change: `no`

#### PRR-I-004

- Severity: `important`
- Category: correctness
- Crate: `sc-observability-otlp`
- Location: `crates/sc-observability-otlp/src/lib.rs:463-466`
- Issue: `SpanAssembler` uses `self.events.remove(&key).unwrap_or_default()`
  when completing a span. Missing event state is silently treated as an empty
  event list, which masks assembler invariants during complex lifecycle bugs.
- Required fix:
  make the missing-event case explicit in the implementation and in health/error
  reporting so event loss is observable during diagnosis.
- Breaking API change: `no`

#### PRR-I-005

- Severity: `important`
- Category: correctness
- Crate: `sc-observability`
- Location: `crates/sc-observability/src/query.rs:92-102`
- Issue: `tracked_offset_for(...)` silently resets to `0` when the previously
  tracked offset is larger than the newly observed file length or when no prior
  tracked file is found. That can replay data without surfacing a reason.
- Required fix:
  distinguish truncate/recreate/missing-state transitions explicitly and record
  the reason in follow tracking or health diagnostics.
- Breaking API change: `no`

#### PRR-I-006

- Severity: `important`
- Category: test coverage
- Crate: `sc-observe`
- Location: `crates/sc-observe/src/lib.rs:235-237`
- Issue: `Observability::flush()` is part of the public runtime surface, but
  this review did not find a direct behavioral test that proves the method
  forwards logger flush success and failure correctly.
- Required fix:
  add a dedicated runtime test for `Observability::flush()` rather than relying
  on broader integration coverage to imply the behavior.
- Breaking API change: `no`

#### PRR-I-007

- Severity: `important`
- Category: release planning / traceability
- Location: `docs/pre-publish-recovery-plan.md:350-364`
- Issue: the carry-over IDs in §9.5 are listed without preserving their
  underlying finding text, code references, or acceptance evidence. That means
  the plan no longer lets a reviewer distinguish "implemented but unverified"
  from "still unfixed" for `QA-001`, `BP-ST-001`, `BP-ST-002`, `BP-IMC-001`,
  `BP-IMC-002`, `BP-NT-003`, `BP-NT-004`, `BP-NT-005`, `BP-ECR-001`,
  `BP-ECR-003`, `REQ-QA-008-phase`, and `REQ-QA-009-phase`.
- Required fix:
  rebuild the Sprint 4 carry-over list as a traceable status table with links
  to the source file, code evidence, or reopened work item for each ID.
- Breaking API change: `no`

### 3.3 Minor Findings

#### PRR-M-001 / `REQ-QA-001`

- Severity: `minor`
- Category: documentation
- Location: `docs/requirements.md:224`
- Issue: OTLP-014 links `LAY-005` to `#2-layering-requirements`, but the actual
  section heading is `## 2. Layered Dependency Order`, so the anchor is broken.
- Required fix:
  update the anchor to the real section target.
- Breaking API change: `no`

## 4. Breaking API Change Set

The following findings require semver-significant public API decisions and
should not be mixed into non-breaking cleanup without explicit approval:

| Finding | Change | Why it breaks |
| --- | --- | --- |
| `PRR-B-002` | reduce visibility or redesign exported constants | callers may already reference the constants directly |
| `PRR-I-001` | rename `TelemetryHealthProvider` | public trait name changes |
| `PRR-I-002` | rename `ObservationSubscriber::handle` | public trait method changes |

No other finding identified in this review requires a mandatory public API
rename or visibility break.

## 5. Sprint 4 Carry-Over Accounting Set

The addendum to this review requires every §9.5 carry-over item to appear in
the plan, even when the current repo already contains partial code for it.

The production-readiness stance for those IDs is:

| Carry-over ID | Current readiness status in this review | Planning treatment |
| --- | --- | --- |
| `QA-001` | not release-closed because Sprint 4 accounting is untrusted | reconcile with evidence |
| `BP-ST-001` | not release-closed because Sprint 4 accounting is untrusted | reconcile with evidence |
| `BP-ST-002` | not release-closed because Sprint 4 accounting is untrusted | reconcile with evidence |
| `BP-IMC-001` | not release-closed because Sprint 4 accounting is untrusted | reconcile with evidence |
| `BP-IMC-002` | not release-closed because Sprint 4 accounting is untrusted | reconcile with evidence |
| `BP-NT-003` | not release-closed because Sprint 4 accounting is untrusted | reconcile with evidence |
| `BP-NT-004` | not release-closed because Sprint 4 accounting is untrusted | reconcile with evidence |
| `BP-NT-005` | not release-closed because Sprint 4 accounting is untrusted | reconcile with evidence |
| `BP-ECR-001` | not release-closed because Sprint 4 accounting is untrusted | reconcile with evidence |
| `BP-ECR-002` | open concrete defect in this review | handled by `PRR-B-001` and reconciliation |
| `BP-ECR-003` | not release-closed because Sprint 4 accounting is untrusted | reconcile with evidence |
| `REQ-QA-008-phase` | not release-closed because Sprint 4 accounting is untrusted | reconcile with evidence |
| `REQ-QA-009-phase` | not release-closed because Sprint 4 accounting is untrusted | reconcile with evidence |
| `BP-TS-001` | explicit post-publish deferral allowed | keep deferred |
| `BP-TS-002` | explicit post-publish deferral allowed | keep deferred |

No other §9.5 item should remain in a deferred state for release accounting.

## 6. Recommended Fix Sprints

### Sprint PR-0: Sprint 4 Closure Reconciliation

- Goal: repair the release-accounting baseline before additional fix work is
  treated as "done"
- Scope:
  - `PRR-B-008`
  - `PRR-I-007`
  - all required §9.5 carry-over IDs:
    `QA-001`, `BP-ST-001`, `BP-ST-002`, `BP-IMC-001`, `BP-IMC-002`,
    `BP-NT-003`, `BP-NT-004`, `BP-NT-005`, `BP-ECR-001`, `BP-ECR-002`,
    `BP-ECR-003`, `REQ-QA-008-phase`, `REQ-QA-009-phase`
  - explicit deferral confirmation for `BP-TS-001` and `BP-TS-002`
- Estimated scope: `medium`
- Deliverables:
  - truthful replacement for the optimistic §9.5 closure text
  - per-ID evidence mapping for every carry-over tag
  - reopened concrete findings for any carry-over ID that is not fully proven
  - explicit statement that only `BP-TS-001` and `BP-TS-002` remain deferred
- Dependencies: none

### Sprint PR-1: Runtime Correctness And Platform Parity

- Goal: remove the real runtime blockers before any docs sweep or naming churn
- Scope:
  - `PRR-B-001`
  - `PRR-B-007`
  - `PRR-I-004`
  - `PRR-I-005`
- Estimated scope: `large`
- Deliverables:
  - explicit shutdown/flush outcome handling in OTLP
  - stable follow-tracking behavior across truncate/recreate transitions
  - `PRR-B-007` resolved with accepted Windows platform limitation: Unix
    recreate coverage is deterministic, Windows remains explicitly
    best-effort-only per `docs/pre-publish-recovery-plan.md:386-391` and this
    fix-r1 change on `fix/ubuntu-test-flake`
  - invariant-masking defaults removed from span assembly and query tracking
- Dependencies:
  - start after `PR-0` so the repo is no longer claiming unverified closure

### Sprint PR-2: Public API Containment And Breaking-Surface Decisions

- Goal: shrink or rename the remaining problematic public surface while the
  release is still blocked
- Scope:
  - `PRR-B-002`
  - `PRR-I-001`
  - `PRR-I-002`
- Estimated scope: `medium`
- Deliverables:
  - final decision for internal constants visibility
  - final approved provider and subscriber naming
  - migration note updates for any public rename or removed export
- Dependencies:
  - start after `PR-1` so runtime semantics are stable before names and
    visibility are frozen

### Sprint PR-3: Rustdoc Completeness Sweep

- Goal: make the shipped public surface self-describing and keep it that way
- Scope:
  - `PRR-B-003`
  - `PRR-B-004`
  - `PRR-B-005`
  - `PRR-B-006`
  - `PRR-M-001`
- Estimated scope: `large`
- Deliverables:
  - all four crates pass `cargo rustdoc -p <crate> -- -Dmissing-docs`
  - broken requirements anchor fixed
  - CI/readiness gate updated so missing rustdoc regresses loudly
- Dependencies:
  - start after `PR-2` so rustdoc is written against the final public names and
    visibility

### Sprint PR-4: Panic Contract And Test Hardening

- Goal: finish the remaining operational-quality work before release
- Scope:
  - `PRR-I-003`
  - `PRR-I-006`
- Estimated scope: `medium`
- Deliverables:
  - explicit panic documentation or structured error conversion for all
    remaining public panic sites
  - direct `Observability::flush()` behavioral coverage
  - release checklist updated only after the new tests and docs are merged
- Dependencies:
  - start after `PR-1`
  - may run in parallel with the end of `PR-3` once public names are frozen

## 7. Sprint Dependency Order

The recommended sequence is:

1. `PR-0` Sprint 4 closure reconciliation
2. `PR-1` runtime correctness and platform parity
3. `PR-2` public API containment and breaking-surface decisions
4. `PR-3` rustdoc completeness sweep
5. `PR-4` panic contract and test hardening
6. rerun a full production-readiness review on the merged result

Reasoning:

- `PR-0` restores a truthful release baseline and prevents false closure
- `PR-1` removes the highest-risk runtime ambiguity first
- `PR-2` freezes the public shape before the docs sweep
- `PR-3` avoids writing docs against names or exports that will immediately
  change
- `PR-4` then locks down operational guarantees and release confidence

## 8. Exit Criteria For Re-Review

Do not rerun a production-readiness signoff until all of the following are
true:

- every finding in Section 3 is either fixed or explicitly accepted in a signed
  release exception
- every §9.5 carry-over ID in Section 5 is reconciled with evidence
- all breaking API changes have approved migration notes
- `cargo test --workspace` passes
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` passes
- all four crates pass `cargo rustdoc -p <crate> -- -Dmissing-docs`
- docs and release checklists are updated to the shipped truth

Until those conditions are met, the repo should be treated as
`release-candidate / remediation in progress`, not production-ready.
