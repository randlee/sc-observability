# d-9: Cross-path qualification and observability docs

Generated projection of `obs-d-9`; the bead is authoritative.

## Plan metadata

- Wave: 4
- Layer: 17
- Assignee / model: cobs / terra
- Relation: `must_follow`
- Closure: `integration`
- Target boundary: OTLP dual-path qualification
- Branch: `sprint/d-9-otlp-conformance`
- Worktree: `/Users/randlee/github/sc-observability-worktrees/sprint/d-9-otlp-conformance`
- PR target (merge order only): `sprint/d-18-integration-and-public-api`
- Blocked by: `obs-d-18-sanity`
- Requirements: DOC-001, DOC-004, LAY-001, LAY-004, LAY-005, LAY-006, NFR-001, NFR-004, NFR-005, NFR-006, NFR-007, NFR-009, OTLP-001, OTLP-002, OTLP-003, OTLP-004, OTLP-005, OTLP-006, OTLP-007, OTLP-008, OTLP-009, OTLP-010, OTLP-011, OTLP-012, OTLP-013, OTLP-014, OTLP-015, OTLP-016, OTLP-017, OTLP-018, OTLP-019, OTLP-020, OTLP-021, OTLP-022, OTLP-023, OTLP-024, PHB-003, PHB-004, PHB-005, PHB-006, PHB-010, PHB-011, SRC-001, SRC-002, SRC-003, SRC-004, SRC-005, SRC-006, TYP-001, TYP-002, TYP-003, TYP-004, TYP-005, TYP-007, TYP-008, TYP-009, TYP-010, TYP-011, TYP-012, TYP-013, TYP-014, TYP-015, TYP-016, TYP-017, TYP-018, TYP-019, TYP-021, TYP-023, TYP-024, TYP-030, TYP-031
- ADRs: ADR-001, ADR-002, ADR-004, ADR-005, ADR-006, ADR-008, ADR-009, ADR-017, ADR-018
- Owned paths (metadata projection):
  - `.github/workflows/otlp-conformance.yml`
  - `crates/sc-observability-otlp/tests/full_stack_integration.rs`
  - `docs/observability/otlp/**`
  - `docs/plans/phase-d/sprint-d-9-otlp-conformance.md`
  - `scripts/ci/fixtures/otlp/**`
  - `scripts/ci/otlp_dev_install_smoke.py`
  - `scripts/ci/verify_otlp_grafana_smoke.py`

## Goal and dependency

Qualify the complete restored OTLP surface and close the regression with
collector evidence and current-schema documentation. D.9 follows obs-d-18-sanity for its composed production exporter factory; it adds no third transport.


## Deliverables

1. Add a shared conformance corpus covering resources/scope, log severity/body/
   attributes, trace parent/kind/flags/links/events/status/timing, and counter,
   gauge, and zero/one/many-bucket histograms. The harness must also prove that
   both modes exercise identical crate-private exporter-trait call sites and
   differ only in construction/injection.
2. Run both backends against hermetic collectors and compare decoded semantic
   output. Add negative cases for disabled no-network, unsupported selections,
   invalid models, auth redaction, timeout, retry exhaustion, partial signal
   failure, recovery, flush, and idempotent shutdown. SDK cases must use and
   await D.6's async lifecycle; legacy cases must exercise both its async
   worker-barrier completion and synchronous compatibility lifecycle.
3. Add CI jobs/features for both backends with hermetic collectors and no
   hidden external service requirement.
4. Restore Grafana dashboard and LogQL/trace/metric recipes from legacy phases
   AV–AY only after translating them to current neutral resource/attribute
   schema. No stale ATM-only label is presented as a generic contract. Use the exact blobs and
   destinations in `legacy-otlp-provenance.json`, including
   `docs/observability/otlp/` and `scripts/ci/`; validate the import manifest's
   pinned-source and allowed-delta checks.


## This Sprint Does Not Close

No `atm-core` implementation, Python OTEL binding (#88), registry publication,
or new transport beyond the two qualified paths.

## Design

## Qualification contract

Consume D.18's production factory and D.12's authoritative model, lifecycle and failure contracts. The same logical logs/spans/metrics corpus runs through both adapters; compare decoded semantic content, allowing only protocol differences. full_stack_integration.rs, the dedicated otlp-conformance.yml workflow and scripts/ci/fixtures/otlp contain the hermetic collector proof. The two smoke scripts and docs/observability/otlp restore current-schema recipes from the existing immutable source manifest; do not create a second manifest or modify unrelated CI. This wave follows D.18 for the actual composed exporter artifact, not because of a shared file. Wave 4 remains pending the user ruling recorded in the root.

The only file fence is metadata.owned_paths; paths mentioned as dependencies are read-only unless that metadata grants ownership.

## Handoff from obs-d-18 (wave 3)

Created by obs-d-18, owned here from wave 4. Consume its completed sanity-gated artifact; preserve the contract while implementing or retiring staged compatibility. This serial handoff is why relation is must_follow; no same-wave sibling shares these paths.

- `crates/sc-observability-otlp/tests/full_stack_integration.rs`

## Acceptance criteria

## Acceptance criteria

- Both backends export all three signal families from the shared corpus and
  decoded collector output is semantically equivalent for every required field.
- Every lifecycle/failure negative case asserts the exact D.12 health and
  accounting contract for both backends, with no credential leakage; D.9
  neither extends nor restates that model.
- Awaited SDK shutdown surfaces the actual final-export failure on both
  current-thread and multi-thread runtimes; after successful await the host can
  tear its runtime down immediately without losing an admitted export.
- Runtime teardown before async completion asserts the corresponding D.12
  failure-table outcome and accounting; concurrent emit/flush/shutdown follows
  D.12's sequence/barrier contract.
- Feature isolation proves the synchronous path needs no caller-owned Tokio runtime and
  no official OTel SDK/tonic dependency while explicitly recording reqwest's
  internal transitive Tokio graph.
- Restored dashboards/queries work against the current collector fixture.
- Import provenance validates pinned source commit/blob ids and the named
  allowed deltas for transplanted destinations.


## Required validation

- Shared dual-backend conformance suite and negative matrix.
- Feature-isolated tests for `otlp-sdk` and `legacy-http-json`, plus combined
  feature tests for both graphs.
- `cargo test --workspace --locked`, clippy with warnings denied, rustdoc,
  public API/semver, dependency/license, and CI workflow validation.
- Local collector smoke commands for both backends.


- `cargo test -p sc-observability-otlp --test full_stack_integration --features otlp-sdk,legacy-http-json --locked` runs a nonzero hermetic dual-backend corpus. This is the sole collector-equivalence feature gate.

- [ ] At this bead's close, `cargo check --workspace --all-features --locked` and `cargo test --workspace --locked` pass. This is the lead's intermediate-workspace invariant; D.18 additionally runs all-features release tests and semver/removal gates.
