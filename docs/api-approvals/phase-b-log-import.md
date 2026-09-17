# Phase B B.1 log-bridge import

Date: 2026-09-17

## Scope

The mechanical B.1 copy of `crates/sc-observability-log`,
`crates/sc-observability-log-macros`, and CI-only
`crates/sc-observability-log-consumer-check` from the accepted BTIT source
`396a9d9f77ca1950eeb92d4f88c0eecadb5ef00b` (recorded in
`docs/plans/phase-b/import-provenance.json` and cross-checked against
`docs/plans/phase-b/handoff-b-p3.md`), against the locked
[target bridge API](../plans/phase-b/target-bridge-api.md) at
`84b32e9d6718418371ffd25a3de52346278725ca`. All three packages retain
`publish = false` (`sc-observability-log-consumer-check` permanently); none is
part of the workspace's published public API surface, so this is a source-only
governance approval, not a public API approval for these three crates
specifically: `scripts/ci/public_api_common.sh`'s `workspace_public_crates()`
excludes any `publish = false` package, so the three copied crates contribute
zero lines to `validate_public_api_diff.sh`/`validate_public_api_docs.sh`.
That said, running the whole command is expected to exit 1 with a nonempty,
purely additive diff against the four already-published core crates
(`sc-observability-types`, `sc-observability`, `sc-observe`,
`sc-observability-otlp`) -- this diff is inherited from the already-merged
B.1a/B.1b/B.1c typed-API preparation layers underneath this branch, not
introduced by this copy, and it is not a "no diff"/PASS state. Confirmed by a
fresh run at `c0a4cddaede02a7e3234bc48c421cd56be01660b`:
`validate_public_api_diff.sh` reports "public API diff report generated
(diffs detected)" (exit 1) with only additive entries under
`sc_observability_types::typed`/`error_codes` and the corresponding
`sc-observe` typed methods; `validate_public_api_docs.sh` separately passes
(exit 0, rustdoc coverage only, unaffected by the diff).

## Approval

Two distinct approvals are in play; they must not be conflated:

- **BTIT source acceptance (already granted, upstream).** Phase lead aobs's
  `docs/plans/phase-b/handoff-b-p3.md` records acceptance of BTIT's
  implementation/critical-review of the target bridge API at source commit
  `396a9d9f77ca1950eeb92d4f88c0eecadb5ef00b`. This is settled and is the basis
  for this copy's entry gate; it is not re-litigated here.
- **Destination-copy review (recorded below).** Team-lead executed
  the mechanical copy under that upstream acceptance: all 85 source files
  verified byte-identical to the accepted commit by Git blob ID (zero
  mismatches), with only the mechanical adaptations declared in
  `import-provenance.json` (workspace-inherited `[package]` metadata, and
  toolchain-drift `trybuild` `.stderr` rewording since this workspace pins
  Rust 1.94.1 while BTIT pins 1.98.1) applied on top. This document recorded
  that execution; the destination-copy reviewer sign-off itself is the
  verbatim record below.

### API-approval reviewer sign-off

Recorded verbatim from
`/Users/randlee/.config/atm/share/sc-obs/b1copy-evidence/015a8886/lead-copy-approval.json`:

- **Reviewer:** aobs (appointed Phase B lead)
- **Reviewed at:** 2026-09-17T08:49:45.948232+00:00
- **Destination SHA:** `015a88867ee79396a6cd406cca449913878c515f`
- **Accepted source SHA:** `396a9d9f77ca1950eeb92d4f88c0eecadb5ef00b`
- **Target contract SHA:** `84b32e9d6718418371ffd25a3de52346278725ca`
- **Statement:** "I approve the B.1 destination mechanical copy and its scoped
  companion-crate boundary additions at the reviewed destination SHA, against
  the accepted source and target contract. The source inventory and permitted
  adaptations are coherent; the copied public contract is retained. This is
  destination-copy source/API governance approval. It does not approve
  publication or close the separately owner-deferred runtime-level contract,
  nor resolve the remaining QA findings."
- **Evidence:**
  - `/Users/randlee/.config/atm/share/sc-obs/b1copy-evidence/015a8886/lead-import-validation.log`
  - `CI35198253852`
  - `PackageCI35198253863`
  - `docs/plans/phase-b/log-import-export-report.md` at `015a88867ee79396a6cd406cca449913878c515f`
  - `docs/plans/phase-b/import-provenance.json` at `015a88867ee79396a6cd406cca449913878c515f`

This is destination-copy source/API governance approval; it does not approve
publication, does not close the separately owner-deferred runtime-level
contract, and does not itself resolve or substitute for independent QA
findings raised against this branch.

Neither approval grants publication, a BTIT dependency switch, or
runtime-contract closure (Phase B's runtime-level contract remains
owner-deferred per `docs/api-approvals/phase-b-runtime-level.md`).

## Affected Artifacts

- `crates/sc-observability-log/`, `crates/sc-observability-log-macros/`,
  `crates/sc-observability-log-consumer-check/` (new workspace members).
- Root `Cargo.toml` (`[workspace.members]` and `[workspace.dependencies]`
  wiring for the three new crates and their third-party dependencies).
- `docs/plans/phase-b/import-provenance.json` and
  `scripts/ci/validate_log_import.py` (extended with the `.workspace = true`
  `package_metadata` line shape and the new `trybuild_diagnostic_text`
  adaptation kind; both extensions are structurally verified, not merely
  pattern-matched, per the validator's existing design).
- `scripts/ci/validate_docs_consistency.sh` (rustdoc missing-docs coverage
  extended to the two new library crates) and
  `scripts/ci/validate_repo_boundaries.sh` (ATM-coupling/home-dir-discovery
  scan and the macros-must-not-depend-on-bridge / core-must-not-depend-on-copy
  invariants extended to the three new crates).
- `docs/architecture.md` Crate Boundary Table (new rows for the three copied
  crates).
- `docs/plans/phase-b/handoff-b-1.md` (execution evidence).
- `docs/plans/phase-b/log-import-export-report.md` and
  `docs/plans/phase-b/evidence/b1-log-export.txt` /
  `b1-log-macros-export.txt` (generated exported API/impl inventory,
  reconciled against `target-bridge-api.md`'s disposition tables; added in
  response to completeness finding B1-C02).
- `scripts/ci/_runtime_level_common.py` and
  `scripts/ci/validate_runtime_level_qualification_metadata.py` (workspace
  member roster check adapted to tolerate the three unpublished companion
  crates while preserving the staged four-package publish order and the
  private consumer-check crate's permanent exclusion from that roster;
  `scripts/ci/tests/test_validate_runtime_level_qualification_metadata.py`
  gained focused positive/negative coverage; added in response to
  completeness finding B1-C03).
- `scripts/ci/prepare_runtime_level_staged_packages.py` (advance the
  bridge-to-macros exact `=V` version pin to the candidate version when
  staging B.P2 packages, alongside the plain-pin dependencies already
  advanced; the staged four-package roster/order and
  `release/publish-artifacts.toml` are unchanged).
- `.github/workflows/ci.yml` (the three-platform `test` job now also runs
  `sc-observability-log`'s `test_hooks` and release-mode
  `static_level_cap_test` opt-in features).
