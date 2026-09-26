# d-18: Integration and public API

Generated projection of `obs-d-18`; the bead is authoritative.

## Plan metadata

- Wave: 3
- Layer: 19
- Assignee / model: cobs / terra
- Relation: `must_follow`
- Closure: `integration`
- Target boundary: phase integration
- Branch: `sprint/d-18-integration-and-public-api`
- Worktree: `/Users/randlee/github/sc-observability-worktrees/sprint/d-18-integration-and-public-api`
- PR target (merge order only): `sprint/d-20-language-binding-migration`
- Blocked by: `obs-d-2-sanity`, `obs-d-3-sanity`, `obs-d-21-sanity`, `obs-d-20-sanity`, `obs-d-16-sanity`, `obs-d-1-sanity`, `obs-d-10-sanity`, `obs-d-17-sanity`, `obs-d-14-sanity`, `obs-d-6-sanity`, `obs-d-4-sanity`, `obs-d-19-sanity`, `obs-d-5-sanity`, `obs-d-15-sanity`, `obs-d-8-sanity`, `obs-d-7-sanity`
- Requirements: LAY-003, LAY-004, LAY-005, LOG-004, LOG-007, LOG-009, LOG-010, LOG-014, LOG-015, LOG-023, LOG-037, LOG-042, LOG-046, LOG-047, LOG-048, NFR-003, NFR-007, NFR-008, NFR-010, NFR-011, NFR-012, OBS-004, OBS-007, OBS-009, OBS-010, OBS-011, OBS-012, OBS-013, OBS-014, OBS-015, OBS-016, OBS-017, OBS-018, OBS-019, OBS-020, OBS-024, OTLP-001, OTLP-002, OTLP-005, OTLP-006, OTLP-007, OTLP-011, OTLP-012, OTLP-015, OTLP-017, OTLP-019, OTLP-021, OTLP-023, PHB-006, PHB-010, PHB-011, PHB-012, PHB-013, PHB-014, PHC-002, PHC-004, PHD-001, PHD-002, PHD-003, TYP-001, TYP-003, TYP-004, TYP-005, TYP-006, TYP-007, TYP-020, TYP-021, TYP-023, TYP-024, TYP-030
- ADRs: ADR-001, ADR-002, ADR-003, ADR-004, ADR-006, ADR-010, ADR-011, ADR-013, ADR-014, ADR-015, ADR-016, ADR-017, ADR-018
- Owned paths (metadata projection):
  - `CHANGELOG.md`
  - `RELEASE-NOTES*.md`
  - `crates/sc-observability-binding-runtime/src/conversion.rs`
  - `crates/sc-observability-binding-runtime/src/lib.rs`
  - `crates/sc-observability-binding-runtime/src/tests.rs`
  - `crates/sc-observability-log-consumer-check/src/lib.rs`
  - `crates/sc-observability-log-consumer-check/tests/control_consumer.rs`
  - `crates/sc-observability-log/src/control.rs`
  - `crates/sc-observability-log/src/error.rs`
  - `crates/sc-observability-log/src/handle.rs`
  - `crates/sc-observability-log/src/lib.rs`
  - `crates/sc-observability-log/src/mapping.rs`
  - `crates/sc-observability-log/tests/api_freeze.rs`
  - `crates/sc-observability-log/tests/bridge_jsonl.rs`
  - `crates/sc-observability-log/tests/flush_single_flight.rs`
  - `crates/sc-observability-log/tests/init_runtime_start.rs`
  - `crates/sc-observability-log/tests/shutdown_timeout.rs`
  - `crates/sc-observability-log/tests/static_level_cap.rs`
  - `crates/sc-observability-otlp/src/assembly.rs`
  - `crates/sc-observability-otlp/src/config.rs`
  - `crates/sc-observability-otlp/src/contracts.rs`
  - `crates/sc-observability-otlp/src/lib.rs`
  - `crates/sc-observability-otlp/src/projectors.rs`
  - `crates/sc-observability-otlp/tests/composition.rs`
  - `crates/sc-observability-otlp/tests/full_stack_integration.rs`
  - `crates/sc-observability-types/src/diagnostic.rs`
  - `crates/sc-observability-types/src/errors.rs`
  - `crates/sc-observability-types/src/errors_v2.rs`
  - `crates/sc-observability-types/src/events.rs`
  - `crates/sc-observability-types/src/lib.rs`
  - `crates/sc-observability-types/src/metric.rs`
  - `crates/sc-observability-types/src/process.rs`
  - `crates/sc-observability-types/src/projection.rs`
  - `crates/sc-observability-types/src/signals_v2.rs`
  - `crates/sc-observability-types/src/span.rs`
  - `crates/sc-observability-types/src/tracing.rs`
  - `crates/sc-observability-types/src/typed.rs`
  - `crates/sc-observability-types/src/validation.rs`
  - `crates/sc-observability-types/tests/neutral_contracts.rs`
  - `crates/sc-observability/src/builder.rs`
  - `crates/sc-observability/src/health.rs`
  - `crates/sc-observability/src/lib.rs`
  - `crates/sc-observability/src/runtime.rs`
  - `crates/sc-observability/src/settings.rs`
  - `crates/sc-observability/src/sinks.rs`
  - `crates/sc-observability/src/typed.rs`
  - `crates/sc-observe/src/lib.rs`
  - `crates/sc-observe/tests/routing_integration.rs`
  - `crates/sc-observe/tests/typed_observation.rs`
  - `docs/api-approvals/**`
  - `docs/migrate-error-api.md`
  - `docs/migration*.md`
  - `docs/plans/phase-d/sprint-d-18-integration-and-public-api.md`
  - `docs/project-plan.md`
  - `examples/custom-sink-example/src/main.rs`
  - `examples/tauri-logging/src-tauri/src/main.rs`
  - `release/**`
  - `scripts/ci/fixtures/error-migration/*/src/**`
  - `scripts/ci/fixtures/error-migration/legacy/src/main.rs`
  - `scripts/ci/fixtures/error-migration/migrated/src/compatibility_matrix.rs`
  - `scripts/ci/tests/test_error_migration*.py`
  - `scripts/ci/tests/test_public_api*.py`
  - `scripts/ci/validate_error_migration.py`
  - `scripts/ci/validate_public_api.py`
  - `scripts/ci/validate_public_api_semver.py`

## Goal

Activate the composed 2.0 library surface after the implementation sanity gates; leave collector/Python artifact qualification to their named following tracks.

## Deliverables

1. Activate the canonical 2.0 re-exports and remove transitional 1.x wrappers/classification/adapters using the recorded contract/implementation-to-integration handoffs. Compile all crate and binding consumers; preserve typed source/remediation and neutral model fields.

2. Compose the completed lifecycle, projectors and SDK/legacy implementations through D.21's private ExporterSet factory. Keep the exporter set/traits private; public Telemetry construction must fail for unsupported enabled transports and must never choose the disabled no-op path.

3. Align release/**, release notes, CHANGELOG.md, approval JSON and inventories with D.21's 2.0 Cargo versions. Activate obs-d-10's independently qualified six-platform Python policy row and artifact inventory; update docs/project-plan.md from the obs-d-10/12 handoff specifications. No Cargo or normative-doc ownership is moved here.

4. Run the existing public API/semver mechanism against frozen 1.4.1 with the consumed major-break manifest: require pass for exactly listed breaks and failure when a real listed break is temporarily omitted. Update validate_error_migration.py and its existing fixtures for canonical 2.0; publish complete migration-guide.md/migration.md/migrate-error-api.md.

5. Consume completed obs-d-19 DTO/schema/generated models and obs-d-20 language adapters without reimplementing either layer; qualify logging settings/attachment/sinks together and enabled transport construction with default and all-features workspace builds/tests.

## This Sprint Does Not Close

D.9 owns hermetic dual-collector equivalence and operational recipes; obs-d-10 owns the folded Python distribution/metadata qualification track. No publication/tagging is authorized.

## Design

## Composition and retirement

Consume the recorded handoffs from D.12/D.13/D.21 and the implementation beads. Following the root sequencing invariant, this sprint switches public re-exports to the new canonical definitions and deletes the compatibility modules/conversions. Do not add public Exporter/Signal/ExporterSet. The factory constructs a private ExporterSet and exposes only the approved Telemetry facade. All enabled backend/feature/protocol combinations either construct their real implementation or return an explicit typed initialization failure. Disabled mode is the only no-network implementation.

## Existing gate consumers

release/public-api-major-breaks.toml is consumed by the existing validate_public_api.py semver mode through validate_public_api_semver.py. It gates the observed intentional wrapper/signature/serde break set against frozen 1.4.1 and rejects unlisted breaks before writing a reviewed 2.0 baseline; retain this manifest as release evidence. No separate breaks ledger is created. Update the existing mechanism where needed; it has no --baseline/--fixture CLI flags today, so do not document invented ones. In a temporary checkout/manifest copy, omit one real observed break and require the same validator to exit nonzero, then restore the manifest and require success. No planted unlisted-break fixture is committed.

validate_error_migration.py already consumes the error-migration fixtures; adapt that existing consumer from warning-only 1.x migration to the ADR-017 2.0 contract. It prevents the observed obsolete wrappers/diagnostic loss from surviving integration. Retain the canonical migrated fixture and explicit intentional-old-source rejection case as long as the migration guide is supported, rather than creating an unconsumed ledger. The Python policy consumer remains the existing B.4a distribution validator; obs-d-10 closes its final runtime artifact proof.

## Feature closure

This bead owns logging settings + host attachment + typed registration end-to-end and public 2.0 error/model integration. OTLP composition and enabled-no-fallback construction close here; collector semantics close only in D.9. Python release policy activation closes here; obs-d-10 independently closes its native artifact proof before this sprint consumes it. These named feature criteria are not duplicated as boundary-sprint release gates. Normative documentation is D.12's contract; release/migration/API approval documents here must match it.

The only file fence is metadata.owned_paths; paths mentioned as dependencies are read-only unless that metadata grants ownership.

## Handoff from obs-d-15 (wave 2)

Created by obs-d-15, owned here from wave 3. Consume its completed sanity-gated artifact; preserve the contract while implementing or retiring staged compatibility. This serial handoff is why relation is must_follow; no same-wave sibling shares these paths.

- `crates/sc-observability-binding-runtime/src/conversion.rs`
- `crates/sc-observability-binding-runtime/src/lib.rs`
- `crates/sc-observability-binding-runtime/src/tests.rs`

## Handoff from obs-d-12 (wave 1)

Created by obs-d-12, owned here from wave 3. Consume its completed sanity-gated artifact; preserve the contract while implementing or retiring staged compatibility. This serial handoff is why relation is must_follow; no same-wave sibling shares these paths.

- `crates/sc-observability-types/src/diagnostic.rs`
- `crates/sc-observability-types/src/errors.rs`
- `crates/sc-observability-types/src/errors_v2.rs`
- `crates/sc-observability-types/src/events.rs`
- `crates/sc-observability-types/src/lib.rs`
- `crates/sc-observability-types/src/metric.rs`
- `crates/sc-observability-types/src/process.rs`
- `crates/sc-observability-types/src/projection.rs`
- `crates/sc-observability-types/src/signals_v2.rs`
- `crates/sc-observability-types/src/span.rs`
- `crates/sc-observability-types/src/tracing.rs`
- `crates/sc-observability-types/src/validation.rs`
- `crates/sc-observability-types/tests/neutral_contracts.rs`

## Handoff from obs-d-17 (wave 2)

Created by obs-d-17, owned here from wave 3. Consume its completed sanity-gated artifact; preserve the contract while implementing or retiring staged compatibility. This serial handoff is why relation is must_follow; no same-wave sibling shares these paths.

- `crates/sc-observability-log-consumer-check/src/lib.rs`
- `crates/sc-observability-log-consumer-check/tests/control_consumer.rs`
- `examples/custom-sink-example/src/main.rs`
- `examples/tauri-logging/src-tauri/src/main.rs`

## Handoff from obs-d-16 (wave 2)

Created by obs-d-16, owned here from wave 3. Consume its completed sanity-gated artifact; preserve the contract while implementing or retiring staged compatibility. This serial handoff is why relation is must_follow; no same-wave sibling shares these paths.

- `crates/sc-observability-log/src/control.rs`
- `crates/sc-observability-log/src/error.rs`
- `crates/sc-observability-log/src/handle.rs`
- `crates/sc-observability-log/src/mapping.rs`
- `crates/sc-observability-log/tests/api_freeze.rs`
- `crates/sc-observability-log/tests/flush_single_flight.rs`
- `crates/sc-observability-log/tests/init_runtime_start.rs`
- `crates/sc-observability-log/tests/shutdown_timeout.rs`
- `crates/sc-observability-log/tests/static_level_cap.rs`

## Handoff from obs-d-13 (wave 1)

Created by obs-d-13, owned here from wave 3. Consume its completed sanity-gated artifact; preserve the contract while implementing or retiring staged compatibility. This serial handoff is why relation is must_follow; no same-wave sibling shares these paths.

- `crates/sc-observability-log/src/lib.rs`
- `crates/sc-observability-types/src/typed.rs`
- `crates/sc-observability/src/settings.rs`
- `crates/sc-observability/src/typed.rs`

## Handoff from obs-d-2 (wave 2)

Created by obs-d-2, owned here from wave 3. Consume its completed sanity-gated artifact; preserve the contract while implementing or retiring staged compatibility. This serial handoff is why relation is must_follow; no same-wave sibling shares these paths.

- `crates/sc-observability-log/tests/bridge_jsonl.rs`

## Handoff from obs-d-5 (wave 2)

Created by obs-d-5, owned here from wave 3. Consume its completed sanity-gated artifact; preserve the contract while implementing or retiring staged compatibility. This serial handoff is why relation is must_follow; no same-wave sibling shares these paths.

- `crates/sc-observability-otlp/src/assembly.rs`
- `crates/sc-observability-otlp/src/projectors.rs`

## Handoff to obs-d-9 (wave 4)

Created/staged by obs-d-18, owned by obs-d-9 from wave 4; after this bead closes it makes no further edits. The receiver consumes the staged contract/implementation and owns production completion or final compatibility retirement.

- `crates/sc-observability-otlp/tests/full_stack_integration.rs`

## Handoff from obs-d-3 (wave 2)

Created by obs-d-3, owned here from wave 3. Consume its completed sanity-gated artifact; preserve the contract while implementing or retiring staged compatibility. This serial handoff is why relation is must_follow; no same-wave sibling shares these paths.

- `crates/sc-observability/src/sinks.rs`
- `crates/sc-observability/src/builder.rs`

## Handoff from obs-d-4 (wave 2)

Created by obs-d-4, owned here from wave 3. Consume its completed sanity-gated artifact; preserve the contract while implementing or retiring staged compatibility. This serial handoff is why relation is must_follow; no same-wave sibling shares these paths.

- `crates/sc-observability/src/health.rs`
- `crates/sc-observability/src/lib.rs`

## Handoff from obs-d-1 (wave 2)

Created by obs-d-1, owned here from wave 3. Consume its completed sanity-gated artifact; preserve the contract while implementing or retiring staged compatibility. This serial handoff is why relation is must_follow; no same-wave sibling shares these paths.

- `crates/sc-observability/src/runtime.rs`

## Handoff from obs-d-14 (wave 2)

Created by obs-d-14, owned here from wave 3. Consume its completed sanity-gated artifact; preserve the contract while implementing or retiring staged compatibility. This serial handoff is why relation is must_follow; no same-wave sibling shares these paths.

- `crates/sc-observe/src/lib.rs`
- `crates/sc-observe/tests/routing_integration.rs`
- `crates/sc-observe/tests/typed_observation.rs`


## Handoff from obs-d-19

obs-d-19 produces its completed wave-2 boundary artifact; obs-d-18 waits on obs-d-19-sanity, composes it unchanged and runs real combined DTO/schema/language binding tests. Layer implementation and generated output stay with obs-d-19; obs-d-18 owns activation/removal in its remaining core/types/facade handoff files only.


## Handoff from obs-d-20

obs-d-20 produces its completed wave-2 boundary artifact; obs-d-18 waits on obs-d-20-sanity, composes it unchanged and runs real combined DTO/schema/language binding tests. Layer implementation and generated output stay with obs-d-20; obs-d-18 owns activation/removal in its remaining core/types/facade handoff files only.


## Handoff from obs-d-6

obs-d-6 produces lifecycle.rs barrier, shutdown ordering and admission control. obs-d-18 consumes it after obs-d-6-sanity and wires public Telemetry facade behavior in lib.rs: emit after shutdown returns TelemetryError::Shutdown; enabled transport construction selects the real backend or a typed failure. obs-d-6 does not edit lib.rs.


## Integration scope

Final wrapper/classifier/adapter deletion is owned here only, following PHD-002. Verify no routing implementation is moved out of sc-observe (LAY-003/NFR-003), no ATM adapter behavior enters shared crates (ADR-006), and layer docs remain self-contained (NFR-008). Validate the imported provenance set under OTLP-023 during composition; no new provenance ledger is created. Release inventory includes npm under the existing shared publishing channel (PHC-002), never a repository-local substitute.


## Handoff from obs-d-12 — project-plan row

Consume obs-d-12 contract specification and write this exact row in owned docs/project-plan.md: "obs-d-12 owns canonical errors, neutral signals and wire projection; obs-d-21 owns OTLP config/default/validation contracts and atomic Cargo 2.0 version activation; obs-d-18 owns final release baseline, approvals, migration guidance and inventory alignment."

## Handoff from obs-d-10

Consume obs-d-10's independent six-wheel/30-native-cell proof and policy specification after obs-d-10-sanity. Activate release/python-platform-policy.json and inventory, then write in owned docs/project-plan.md: "Python uses abi3-py310, cp310-abi3 wheels and requires-python >=3.10 without an upper/exclusion cap; six platforms and 30 native installed-suite cells are proved from one immutable source. Raising the floor or adding a cap requires a separately approved compatibility decision, supported-interpreter matrix and guard expected-value update." obs-d-10 does not wait on this activation and does not edit this document.


## Handoff from obs-d-21 (wave 1)

Consume obs-d-21-sanity for OTLP contracts, module registration and atomic workspace-version activation. Created by obs-d-21, owned here from wave 3 for facade composition/retirement:

- `crates/sc-observability-otlp/src/config.rs`
- `crates/sc-observability-otlp/src/contracts.rs`
- `crates/sc-observability-otlp/src/lib.rs`

## Acceptance criteria

- [ ] `cargo check --workspace --locked`, `cargo check --workspace --all-features --locked`, `cargo test --workspace --locked` and `cargo test --workspace --all-features --locked` pass after canonical activation/removal; workspace clippy/rustdoc and existing boundary gates pass (#1/#5).
- [ ] `cargo test -p sc-observability-otlp --test composition --features otlp-sdk,legacy-http-json --locked` runs enabled_transport_never_noop, disabled_no_network, unsupported_selection_is_error and private_exporter_composition, asserting actual constructor selection rather than only successful compilation (#2).
- [ ] Release inventory enumerates every publishable crate/binding and six Python platforms with no omissions/duplicates; version literals match Cargo's 2.0 value. Approval JSON names each public break and its contract/ADR; changelog/release notes agree. Existing version/docs/release inventory validators pass, not merely file-existence checks (#3).
- [ ] `python3 scripts/ci/validate_public_api_semver.py` passes the complete consumed break manifest and fails with one observed break omitted in scratch; only then generate/review the 2.0 baseline. `python3 scripts/ci/validate_error_migration.py` passes migrated sources and asserts the intended old-source failure (#4).
- [ ] Migration guides enumerate all nine wrapper-to-enum mappings, cause variants, custom sink migration, config/default changes and signal serde changes; the existing migrated consumer fixture executes those recipes without deprecated/compatibility imports (#4).
- [ ] Consume completed obs-d-19/20 artifacts: bash scripts/ci/validate_python_bindings.sh, bash scripts/ci/validate_binding_schema.sh and the existing TypeScript/Tauri qualification gate pass against actual native composition. Canonical DTO/schema/binding fixtures preserve code/remediation/source projection, lossless histograms and typed operational outcomes. Combined logging integration proves precedence, one host writer, policy/redaction, typed sink failure and detach ownership. No final semver/API approval is claimed by boundary sprints (#5).

- [ ] #2/#5: combined observability/OTLP composition tests preserve construction-only routing registration, Send+Sync/open projector contracts, fail-open sibling delivery, typed Shutdown/QueueFull/RoutingFailure guards, and routing health/drop accounting. OTLP config stays independent and sc-observe has no runtime OTLP dependency.
