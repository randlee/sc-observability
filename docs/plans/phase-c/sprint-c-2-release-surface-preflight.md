---
id: C.2
status: proposed
branch: fix/phase-c-2-release-surface-preflight
base: fix/phase-c-1-shared-pipeline-migration
---

# C.2 — Phase B release-surface preflight

## Goal and dependencies

Preflight the complete, post-merge Phase B release surface through the
pipeline migrated in C.1. `must_follow` C.1, with the full dependency
contract stated explicitly:

- Merge C.1's pushed development forward into this sprint's branch before
  every round (trigger: C.1's development push, not its QA outcome — this
  sprint may develop against C.1's in-progress branch).
- **C.1's own PR must merge into `develop` before this sprint's PR is
  considered complete/mergeable** (PR-completion trigger: parent PR merges
  first). This does not add a QA-wait requirement on top of the merge — C.2
  may finish its own review and validation work in parallel with C.1's PR
  review, but C.2's PR cannot itself be merged until C.1's PR has merged.

This sprint requires the installed shared manifest/channel contract, the
upstream npm channel adopted in C.1 (or the owner's explicit escalation
decision if that prerequisite could not be met), and the reconciled
`release/publish-artifacts.toml` from C.1. This sprint runs preflight only;
it never publishes and never runs BTIT repository integration tests.

## Deliverables (authoritative)

Every listed deliverable must land production-ready for this sprint's stated
scope. Completion requires evidence for every numbered item; partial
completion leaves the sprint open. **None of deliverables 2–5 may close on
an owner-visible open-gap note in place of PASS evidence** — a genuine
external blocker (for example, a Phase B qualification run that has not yet
been re-run against the actual merge commit) leaves the specific deliverable
open pending that blocker's resolution, or pending an explicit owner
decision to revise this sprint's scope; it is never recorded as an accepted
gap that lets the sprint close anyway.

1. **Deterministic release-surface inventory (PHC-004).** Re-derive the full
   publishable surface from the actual post-Phase-B-merge `develop` tree —
   do not reuse a pre-merge snapshot. This is the same ten-crate inventory
   fixed in C.1 deliverable 2 (`sc-observability-types`, `sc-observability`,
   `sc-observe`, `sc-observability-otlp`, `sc-observability-log`,
   `sc-observability-log-macros`, `sc-observability-dto`,
   `sc-observability-binding-runtime`, `sc-observability-tauri`, and the
   `sc-observability-py` Rust crate), plus:
   - The `sc-observability-py` wheel/sdist artifact set across its full
     packaging matrix (the Python distribution/platform qualification work
     landed in Phase B's B.4a/B.5/B.6 sprints) — a **separate** inventory row
     from the `sc-observability-py` crates.io row above, never merged into
     one line.
   - The `@sc-observability/client` npm package
     (`bindings/typescript/package.json`, currently version `1.4.0`).
   - `sc-observability-tauri`'s standalone-workspace crate (queried via
     `--manifest-path bindings/tauri/Cargo.toml`, never root `-p`, per C.1
     deliverable 2) and any Tauri plugin distribution asset Phase B's B.3a
     sprint produced.
   Cross-check this inventory against `cargo metadata --no-deps --format-version 1`
   (root workspace), `cargo metadata --no-deps --format-version 1 --manifest-path bindings/tauri/Cargo.toml`
   (Tauri), the wheel build matrix's declared targets, and
   `bindings/typescript/package.json`. A surface item present in the tree but
   missing from the inventory is a defect that blocks this sprint's closure.
2. **Package completeness preflight — concrete, executable commands
   (PHC-006 validation-quality fix).** For each inventory item, run and
   record PASS/FAIL with actual command output:
   - **Crates with no unpublished workspace path dependency** (`preflight_check = "full"`
     in C.1's manifest): `cargo package --locked --allow-dirty -p <crate>`
     from the root workspace, or
     `cargo package --locked --allow-dirty --manifest-path bindings/tauri/Cargo.toml`
     for the standalone Tauri crate.
   - **Chained crates that depend on other not-yet-published workspace
     members** (`preflight_check = "locked"`): `cargo package -p` cannot
     resolve these against the public registry. Use
     `cargo publish --dry-run --locked --no-verify -p <crate>` instead,
     exactly as `docs/publishing.md`'s existing documented dual-mode
     preflight pattern already specifies for the pre-first-publish case; run
     each crate strictly in the `validate-publish-order`-confirmed publish
     order so every earlier dependency in the chain has already been
     dry-run-validated before its dependents are attempted. Where Phase B's
     own staged-dependency-resolution pattern applies more precisely (crates
     that were validated pre-merge via path/registry-substitute overrides),
     generalize the existing
     `scripts/ci/prepare_log_staged_packages.py` /
     `scripts/ci/prepare_runtime_level_staged_packages.py` and
     `scripts/ci/validate_log_staged_consumer.py` /
     `scripts/ci/validate_runtime_level_staged_consumer.py` /
     `scripts/ci/validate_binding_registry_consumers.sh` pattern to the full
     ten-crate chain rather than inventing a new mechanism.
   - **Python wheel/sdist**: `python3 scripts/ci/validate_python_distribution.py --wheel <path> --sdist <path> --policy <policy-file> --evidence <out.json>`
     for every matrix cell, reusing `_python_distribution.py`'s
     `inspect_wheel`/`verify_native_architecture`/`verify_source` checks
     already built for this in Phase B — this is real, existing tooling, not
     a new script.
   - **npm**: `npm pack --dry-run` in `bindings/typescript/` (confirmed
     non-mutating for npm 7+: it lists the exact tarball contents without
     writing a tarball or contacting the registry) and inspect the printed
     file list against `package.json`'s declared `files`/`exports`/`types`
     fields for completeness.
3. **Authentication setup verification without exposing tokens, at correct
   scope (PHC-005 fix).** Confirm every secret C.1 deliverable 7 documents is
   configured at the scope its channel contract actually requires — a
   repository-scope-only check does not validate an environment-scoped
   secret:
   - `gh secret list` (repository scope) for `CARGO_REGISTRY_TOKEN`
     (crates.io — genuinely repository-scoped).
   - `gh api repos/{owner}/{repo}/environments --jq '.environments[].name'`
     to confirm the `pypi` and `testpypi` GitHub Environments exist, then
     `gh secret list --env pypi` and `gh secret list --env testpypi` to
     confirm `PYPI_API_TOKEN` and `TEST_PYPI_API_TOKEN` are each present in
     their own environment (names only, never values).
   - The npm channel's own environment/secret name, once adopted from
     upstream in C.1, checked the same way (`gh secret list --env <name>`
     against whatever environment its shared channel contract specifies) —
     this sprint does not assume `NPM_TOKEN` or repository scope for it in
     advance of C.1's actual adopted contract.
   - Never read, print, or otherwise exercise a secret's value.
4. **Platform qualification cross-check — no completeness escape hatch
   (PHC-005 fix).** For each wheel-matrix cell and the Tauri artifact set,
   name the Phase B CI run that last qualified it (run ID, source SHA,
   conclusion), reusing Phase B's retained evidence rather than re-running
   the full matrix, *only when that evidence's source SHA is the actual
   post-merge `develop` commit or a commit provably identical in the
   qualified paths* (record the equivalence check, e.g. a diff against the
   qualified SHA touching none of the qualified paths). Where no such
   evidence exists for an item — for example the merge commit itself has
   never been through the full matrix — this sprint does **not** record an
   open gap and close anyway: it runs the missing non-publishing
   qualification check itself (build + `validate_python_distribution.py` /
   equivalent architecture/content verification, without publishing) until
   every item has PASS evidence, or it stays open pending an explicit owner
   decision to revise this sprint's scope.
5. **Recovery/idempotency verification.** Confirm the shared package's
   documented retry semantics — per-crate idempotent skip-if-already-published
   behavior, immutable-GitHub-Release-then-separate-channel-upload structure
   for PyPI — extend correctly to every inventory item added in this sprint,
   including the npm channel adopted in C.1 (idempotent re-run against an
   already-published version must not fail the whole run, matching the
   shared PyPI workflow's skip-existing behavior). Verify this with a mocked
   dry-run (for example, invoking the relevant workflow's publish step
   against a deliberately already-published test version in a scratch/dry
   context) rather than asserting it from the workflow's YAML alone.
6. **Post-publish verification design.** Because this sprint does not
   publish, specify the exact read-only commands a later, separately
   authorized publish step will run to confirm each channel actually
   received the expected artifact: `npm view @sc-observability/client versions`,
   `cargo search <crate>` or the crates.io API per crate,
   `pip index versions sc-observability` (or the actual PyPI project name)
   or equivalent, and the npm channel's own verification command once
   adopted. Write these as ready-to-run acceptance commands.

## Acceptance criteria (authoritative)

- The inventory from deliverable 1 is committed to this doc (or a generated
  artifact this doc links to) and matches `cargo metadata` (root and Tauri),
  `bindings/typescript/package.json`, and the Python packaging matrix's
  declared targets with zero unexplained discrepancies.
- **Deliverables 2–5 each have recorded PASS evidence (command + actual
  output) for every inventory item.** No deliverable closes with an
  open-gap note as a substitute for that evidence; a genuine external
  blocker keeps the affected deliverable, and therefore the sprint, open
  until resolved or until the owner explicitly revises this sprint's scope.
- Deliverable 6's commands are present, syntactically valid, and confirmed
  to run without write/publish credentials (all listed commands are
  read-only by nature).
- No channel receives a publish, upload, or tag-creation call anywhere in
  this sprint's validation.
- Every secret check in deliverable 3 targets the scope (`repository` vs.
  named `environment`) its own channel contract actually specifies.

## Required validation

- `gh secret list` (repository scope, `CARGO_REGISTRY_TOKEN` only) and
  `gh secret list --env pypi` / `gh secret list --env testpypi` (names
  only), cross-checked against the documented secret-name/scope list.
- Per-crate packaging dry runs in publish order, using
  `cargo package --locked --allow-dirty -p <crate>` for `"full"`-preflight
  crates and `cargo publish --dry-run --locked --no-verify -p <crate>` for
  `"locked"`-preflight chained crates (root workspace and, for
  `sc-observability-tauri`, `--manifest-path bindings/tauri/Cargo.toml`).
- `python3 scripts/ci/validate_python_distribution.py` runs for every wheel/
  sdist matrix cell, using the same policy file Phase B's B.4a sprint
  established.
- `npm pack --dry-run` in `bindings/typescript/`, output diffed against
  `package.json`'s declared `files`/`exports`/`types`.
- The shared package's three manifest-validation commands (same as C.1) and
  `bash scripts/ci/validate_publish_workflow_action_versions.sh`, re-run
  against the post-merge `develop` state to confirm no drift was introduced
  by the merge.
- The mocked idempotency/retry dry run from deliverable 5.
- Read-only registry-check commands from deliverable 6, executed against
  already-published versions only (never the unreleased Phase B version) to
  confirm they run without error.

## Paths to delete

None. This sprint is inventory/verification only; it does not remove files.

## Non-closure

- Does not publish anything to any channel.
- Does not create or push a release tag.
- Does not run BTIT repository integration tests — those remain gated on
  authorized publication, per `plan-phase-c.md`'s sequence.
- Does not resolve an open platform-qualification gap found in deliverable 4
  by re-running CI itself when that gap is a genuine external blocker (for
  example, a Phase B CI system unavailable to this sprint); it instead stays
  open pending that blocker's resolution or an explicit owner scope
  revision — it does not record the gap and close anyway.
