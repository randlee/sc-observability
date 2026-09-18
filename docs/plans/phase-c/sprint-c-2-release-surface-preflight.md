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
     members** (`preflight_check = "locked"`): staged local resolution is
     **mandatory** for every such crate, not an alternative reached only
     where it happens to apply more precisely. `cargo package -p` cannot
     resolve these against the public registry, and a per-crate
     `cargo publish --dry-run --locked --no-verify -p <crate>` run one crate
     at a time hits the same unresolved-path-dependency failure for any
     crate whose dependency has not itself been published — it is not a
     complete pre-publication path on its own. Instead, generalize
     `scripts/ci/prepare_log_staged_packages.py`'s actual mechanism (a
     single `cargo package --locked --target-dir <build-dir> -p <crate-1> -p <crate-2> ... -p <crate-N>`
     invocation naming every not-yet-published crate together, which lets
     Cargo resolve inter-crate path dependencies from the local workspace
     instead of the public registry, followed by `verify_stage()`-style
     archive inspection from `scripts/ci/_log_staging.py`) into a new
     `scripts/ci/prepare_release_staged_packages.py` that reads its crate
     list and order from `release/publish-artifacts.toml` (every
     `"locked"`-preflight crate, in `validate-publish-order`-confirmed
     order) instead of the hardcoded six-package tuple the B.2-era script
     uses. Run this staged multi-package `cargo package` command for the
     complete `"locked"` set before any per-crate dry-run step; any crate
     that only has `"full"`-preflight dependents may still use the simpler
     per-crate `cargo package`/`cargo publish --dry-run` commands above.
   - **Python wheel/sdist**: reuse Phase B's real, existing three-subcommand
     CLI (`python3 scripts/ci/validate_python_distribution.py {build,cell,aggregate}`,
     verified directly via `--help` against each subcommand — there is no
     bare `--wheel`/`--sdist`/`--policy`/`--evidence` flag set on the
     top-level command). Prefer reusing Phase B's retained evidence over
     rebuilding:
     - First, locate the `b4a-production-inventory` GitHub Actions artifact
       (job `aggregate` in `.github/workflows/b4a-python-distributions.yml`,
       containing `production-artifacts.json`) from the CI run that
       qualified the actual post-merge `develop` commit (or a commit
       provably identical in every qualified path, per deliverable 4).
       Download it — `gh run download <run-id> --name b4a-production-inventory --dir <dir>`
       — and verify `sha256sum <dir>/production-artifacts.json` matches
       that run's recorded `inventory_sha256` output. That file **is** this
       item's completeness PASS evidence; it is the real output of the
       `aggregate` subcommand, not a placeholder.
     - If no such run exists yet for the post-merge commit, produce only
       the missing evidence: `python3 scripts/ci/validate_python_distribution.py build --sdist <sdist> --output <dir>/wheel --checkout <checkout> --platform <platform>`
       per missing platform, then
       `python3 scripts/ci/validate_python_distribution.py cell --sdist <sdist> --wheel <dir>/wheel/*.whl --checkout <checkout> --output <dir>/cell`
       per missing Python version (add `--allow-incomplete-runtime` only for
       a non-production/provisional cell), then combine retained and
       newly-produced evidence directories under one path and run
       `python3 scripts/ci/validate_python_distribution.py aggregate --policy release/python-platform-policy.json --sdist <sdist> --evidence <combined-dir> --source-commit <post-merge-sha> --output <dir>/production-artifacts.json`
       — `aggregate --evidence` recursively globs for `build-result.json`/
       `cell-result.json` under that path, so retained per-cell artifacts
       from the GitHub Actions run can sit alongside freshly produced ones
       in the same directory tree.
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
5. **Recovery/idempotency verification — named mocked harness.** Neither
   this repository nor the pinned `../sc-publish` revision has an existing
   mocked publish-retry test to reuse (confirmed: no `idempoten`/`retry`/
   `skip-existing` hit in either repo's test suites other than unrelated
   doc-claim checks). Add
   `scripts/ci/tests/test_publish_retry_idempotency.py`, a pytest harness
   that monkeypatches `subprocess.run`/`subprocess.Popen` for every
   channel's publish invocation (`cargo publish`, `maturin upload`,
   `npm publish`, `gh release create`, and the npm channel's own publish
   step once adopted) to record the exact command line and return a canned
   result instead of executing it, and additionally monkeypatches
   `socket.socket` to raise if any code path under test tries to open a
   real network connection — guaranteeing zero network writes regardless of
   what the mocked commands would have done for real. Exercise three cases
   per channel against the actual installed workflow/CLI invocation (not a
   reimplementation of its logic):
   - **missing version**: the channel's existence-check step (e.g.
     `maturin upload`'s registry probe, `cargo publish`'s own 409 handling,
     an `npm view <pkg>@<version>` probe) reports the version absent → the
     publish step must be invoked and the harness asserts success.
   - **already-present version**: the existence check reports the version
     present → the publish step must be **skipped** (idempotent no-op) and
     the harness asserts the run still reports overall success, matching
     the shared PyPI workflow's `--skip-existing` behavior and the required
     equivalent for the npm channel once adopted (a 409/"already published"
     response treated as success, not failure).
   - **registry error**: the existence check or publish step returns a
     non-success, non-409 error (e.g. HTTP 500) → the harness asserts the
     run reports failure and does not silently swallow the error as
     success.
   Run this harness in this sprint's own validation; it is new tooling this
   sprint adds, since none exists upstream or in this repo to reuse as-is.
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
- `scripts/ci/prepare_release_staged_packages.py` (new, generalized from
  `scripts/ci/prepare_log_staged_packages.py`) runs the single
  multi-package `cargo package --locked --target-dir <dir> -p ... -p ...`
  invocation across every `"locked"`-preflight crate from
  `release/publish-artifacts.toml`, in `validate-publish-order`-confirmed
  order; `cargo package --locked --allow-dirty -p <crate>` (or
  `--manifest-path bindings/tauri/Cargo.toml`) runs per-crate for every
  `"full"`-preflight crate.
- `python3 scripts/ci/validate_python_distribution.py build|cell|aggregate`
  (the real three-subcommand CLI, per its `--help` output) either verifies
  the retained `b4a-production-inventory` artifact's `production-artifacts.json`
  against its recorded `inventory_sha256`, or runs the missing `build`/
  `cell`/`aggregate` steps to produce it, per deliverable 2.
- `npm pack --dry-run` in `bindings/typescript/`, output diffed against
  `package.json`'s declared `files`/`exports`/`types`.
- `python3 -m pytest scripts/ci/tests/test_publish_retry_idempotency.py`
  passes all three cases (missing version, already-present version,
  registry error) per channel with zero real network connections opened.
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
