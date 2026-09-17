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
governance approval, not a public API approval
(`bash scripts/ci/validate_public_api_docs.sh` and
`validate_public_api_diff.sh` report no public API diff for the copy, since
`publish = false` crates are excluded from that scan).

## Approval

Phase lead aobs's `docs/plans/phase-b/handoff-b-p3.md` records BTIT source
acceptance for mechanical import. Team-lead executed the mechanical copy under
that acceptance: all 85 source files verified byte-identical to the accepted
commit by Git blob ID (zero mismatches), with only the mechanical adaptations
declared in `import-provenance.json` (workspace-inherited `[package]`
metadata, and toolchain-drift `trybuild` `.stderr` rewording since this
workspace pins Rust 1.94.1 while BTIT pins 1.98.1) applied on top. This
approval covers the copy's structural conformance; it grants no publication,
no BTIT dependency switch, and no runtime-contract closure (Phase B's
runtime-level contract remains owner-deferred per
`docs/api-approvals/phase-b-runtime-level.md`).

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
