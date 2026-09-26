# d-19: neutral DTO/schema and generated models

Generated projection of `obs-d-19`; the bead is authoritative.

## Plan metadata

- Wave: 2
- Layer: 16
- Assignee / model: cobs2 / terra
- Relation: `must_follow`
- Closure: `boundary`
- Target boundary: neutral DTO/schema and generated models
- Branch: `sprint/d-19-dto-and-schema-migration`
- Worktree: `/Users/randlee/github/sc-observability-worktrees/sprint/d-19-dto-and-schema-migration`
- PR target (merge order only): `sprint/d-17-log-consumer-error-migration`
- Blocked by: `obs-phase-d-plan-qa`, `obs-d-12-sanity`
- Requirements: LAY-001, PHB-002, PHB-010, PHB-012, PHB-013, PHD-001, TYP-002, TYP-003, TYP-004, TYP-005, TYP-007
- ADRs: ADR-002, ADR-005, ADR-011, ADR-014, ADR-017
- Owned paths (metadata projection):
  - `bindings/conformance/v1/conversion-cases.json`
  - `bindings/conformance/v1/schema-cases.json`
  - `bindings/generation-manifest.json`
  - `bindings/generation-toolchain.toml`
  - `bindings/python/sc-observability-py/python/sc_observability/generated/__init__.py`
  - `bindings/python/sc-observability-py/python/sc_observability/generated/__init__.pyi`
  - `bindings/python/sc-observability-py/python/sc_observability/generated/py.typed`
  - `bindings/schema-generator/src/main.rs`
  - `bindings/schema-generator/tests/conformance.rs`
  - `bindings/schema/errors-v1.json`
  - `bindings/schema/v1.json`
  - `bindings/typescript/src/generated/index.ts`
  - `bindings/typescript/src/generated/package.json`
  - `crates/sc-observability-dto/src/conversion.rs`
  - `crates/sc-observability-dto/src/lib.rs`
  - `crates/sc-observability-dto/src/wire.rs`
  - `crates/sc-observability-dto/src/wire/events.rs`
  - `crates/sc-observability-dto/src/wire/health.rs`
  - `crates/sc-observability-dto/src/wire/operations.rs`
  - `crates/sc-observability-dto/src/wire/primitives.rs`
  - `crates/sc-observability-dto/tests/**`
  - `docs/plans/phase-d/sprint-d-19-dto-and-schema-migration.md`

## Goal

Close the neutral DTO/schema boundary against obs-d-12's frozen 2.0 error/signal and wire-projection contract.

## Deliverables

1. Migrate DTO checked conversions and error projections to canonical cause variants while preserving stable code/remediation fields and lossless histogram/temporal values. Preserve PHB-002's neutral-only dependency boundary.

2. Update schema-generator source, canonical JSON/error schemas, schema/conversion conformance corpus, and generated TypeScript/Python model outputs to match those DTOs. Existing generators remain the only generation path; records/unknown values retain checked numeric/path/variant validation.

3. Run DTO/schema/generator conformance and generated-model typing checks, including valid/invalid histogram, typed operational error and unknown-variant fixtures.

## This Sprint Does Not Close

obs-d-20 owns language transport/extraction adapters and runtime tests; obs-d-18 integrates both completed artifacts, removes compatibility and closes semver/release approvals. No registry definition, manifest change, runtime policy or publication belongs here.

## Design

## Independent boundary closure

Consume obs-d-12's frozen neutral types and wire-projection specification after its sanity gate. DTO owns projection only, never native runtime/bridge mapping. obs-d-19 owns canonical schema and generated model output together, preventing a same-wave generation dependency on obs-d-20. Operational envelope field names and discriminants used by obs-d-20 are frozen by obs-d-12; adapters compile/test against the existing compatible envelope plus local fixtures, not unfinished new schema output. New neutral signal fields remain additive/staged until integration activation. Reject invalid histograms and temporal intervals on checked conversion; never lose count/sum/bounds or coerce an unknown error into success. Existing generator commands consume the canonical schema. Do not modify error_codes.rs: consume obs-d-12's registry read-only.

## Handoff from obs-d-12

obs-d-12 freezes canonical errors/signals and wire-projection specifications in docs/api-design.md; obs-d-19 consumes them after obs-d-12-sanity with types/registry files read-only.

## Handoff to obs-d-18

obs-d-19 produces completed DTO/schema/generated models. obs-d-18 consumes them after obs-d-19-sanity, runs combined binding/schema tests and activates canonical exports; it does not reimplement DTO or generation work. Owned files remain read-only to obs-d-18 unless a separately approved integration correction is assigned.


## Release gate and scope

This boundary releases only its named artifact to obs-d-18 after its paired sanity check; final 2.0 semver/API approval, obsolete-wrapper removal and release inventory are obs-d-18 gates. The phase-root workspace invariant applies once to every sprint.

## Acceptance criteria

- [ ] #1: cargo test --locked -p sc-observability-dto passes canonical_error_projection_preserves_context, histogram_conversion_is_lossless and invalid_histogram_rejected, added to owned DTO tests with nonzero test counts.
- [ ] #2: cargo test --locked --manifest-path bindings/schema-generator/Cargo.toml passes; cargo run --locked --manifest-path bindings/schema-generator/Cargo.toml --bin sc-observability-schema -- --output bindings/schema/v1.json --errors-output bindings/schema/errors-v1.json --check reports no drift.
- [ ] #2–3: bash scripts/ci/validate_binding_schema.sh passes its existing schema, generator and typing consumers using the pinned toolchains; conformance covers unknown variants and tagged failures without success conversion.
- [ ] #1–3: root workspace invariant passes; no runtime adapter or registry definition was introduced in the DTO boundary.
