---
id: C.1
status: proposed
branch: fix/phase-c-1-shared-pipeline-migration
base: develop
---

# C.1 — Shared publishing pipeline migration

## Goal and dependencies

Replace this repository's bespoke, repository-specific publishing
implementation with the pinned `../sc-publish` shared package, installed only
through its caller-owned `install.py` JSON contract. `parallel_safe` with
respect to any Phase B sprint (Phase B has already merged into `develop`
before this sprint starts, per the phase sequence in `plan-phase-c.md`).
`must_follow`: none inside Phase C; C.2 `must_follow`s this sprint. No
publication, tagging, or BTIT integration test runs in this sprint.

## Deliverables (authoritative)

Every listed deliverable must land production-ready for this sprint's stated
scope. Completion requires evidence for every numbered item; partial
completion leaves the sprint open.

1. **Pin the shared-package revision.** Record the exact reviewed commit of
   `../sc-publish` used for installation (starting point:
   `3a57926b3e939835644aad944a614cbb48e4d5fc`, confirmed current at plan time)
   in this doc and in `docs/publishing.md`. Re-verify the pin is still the
   intended revision immediately before running the installer, since
   `../sc-publish` is independently developed.
2. **Author a reviewed `install.json`.** Run
   `plugins/sc-publish/install.py --example-json` against this repository to
   get a source-discovered starting point, then hand-review and correct every
   field before use — the example generator infers `publish`/`required`
   flags and channel enablement heuristically and must not be installed
   unmodified. In particular:
   - Explicitly decide and record `publish`/`required`/`preflight_check`/
     `verify_install`/`wait_after_publish_seconds` for every one of the nine
     workspace-member Rust crates (`sc-observability-types`,
     `sc-observability`, `sc-observe`, `sc-observability-otlp`,
     `sc-observability-log`, `sc-observability-log-macros`,
     `sc-observability-dto`, `sc-observability-binding-runtime`,
     `sc-observability-tauri`), plus `bindings/python/sc-observability-py`.
     `sc-observability-log-consumer-check` has `publish = false` in its own
     `Cargo.toml` and is excluded. `sc-observability-dto`,
     `sc-observability-binding-runtime`, `sc-observability-tauri`, and the
     Python extension crate currently carry no explicit `publish` field in
     their `Cargo.toml` (Cargo therefore defaults them to publishable); this
     sprint must record an explicit crates.io publish decision for each
     rather than carrying the Cargo default forward silently. The Python
     extension crate (`cdylib`) is built into a wheel via maturin and is not
     a crates.io publish target regardless of its `Cargo.toml` default;
     record `publish = false` for it in the manifest and, if the default
     should also change in `Cargo.toml`, file that as a follow-up (source
     changes are out of scope for this planning branch).
     `preflight_check` must be `"full"` for crates with no workspace path
     dependency and `"locked"` for chained crates that depend on other
     workspace members, matching the shared installer's own manifest-schema
     rule.
   - `artifacts.wheels`: `sc-observability-py` (`python_package`
     `sc_observability`).
   - `artifacts.binaries`: this repo currently ships no standalone release
     binary target; leave empty unless a Phase B deliverable adds one.
   - `channels`: enable `crates_io` (crates present) and `pypi` (wheel
     present); `github_release` only if a GitHub Release is actually wanted
     as an artifact host (record the decision either way, do not leave it at
     the heuristic default without review); `homebrew`/`scoop`/`winget`
     disabled unless a real distributable binary exists to justify them.
3. **Resolve the npm channel gap (PHC-002).** `install.py`'s `CHANNEL_NAMES`
   is exactly `github_release`, `crates_io`, `pypi`, `homebrew`, `scoop`,
   `winget` — confirmed by reading `install.py` at the pinned revision; there
   is no npm channel. The TypeScript client (`@sc-observability/client`,
   `bindings/typescript/package.json`) still needs a publish path. Adopt one
   explicit resolution and document it here and in `docs/publishing.md`:
   - **Chosen approach:** add a narrow, repository-owned
     `.github/workflows/npm-publish.yml` (manual `workflow_dispatch`, mirroring
     the shared `pypi-publish.yml`'s "publish already-built artifacts from an
     immutable GitHub Release, not a fresh build" pattern) that publishes the
     npm package using an `NPM_TOKEN` secret. This file lives outside the
     installer's managed paths (it does not collide with any file
     `install.py`'s `package_files()` would copy) so re-running the installer
     never overwrites or deletes it. Record the npm package name/version
     source and tag-prefix convention explicitly so it matches
     `release.version_source`/`tag_prefix` in the rendered
     `publish-artifacts.toml`.
   - This is a repository-local addition, not a change to `../sc-publish`;
     upstreaming an npm channel there is out of scope for this sprint (see
     `plan-phase-c.md` non-closure section).
4. **Exact legacy-to-shared asset inventory.** Delete, after the shared
   package's replacement files are installed and diffed clean:
   - `.claude/agents/publisher.md` (replaced by
     `plugins/sc-publish/.claude/agents/publisher.md` plus its five
     channel-worker agents, installed under the same `.claude/agents/` path).
   - `.github/workflows/release.yml`, `.github/workflows/release-preflight.yml`
     (replaced by the shared package's same-named workflows).
   - `scripts/release_artifacts.py` (replaced by the shared package's
     `.github/scripts/release_artifacts.py` — note the path changes from
     `scripts/` to `.github/scripts/`; every reference to the old path in
     retained docs/scripts must be updated, not left dangling).
   - `scripts/ci/validate_publish_order.sh` (its single check — publish-order
     uniqueness/ordering in `release/publish-artifacts.toml` — is superseded
     by the shared package's `validate-manifest`/`validate-preflight-checks`
     gates in `.github/scripts/release_artifacts.py`; confirm the shared
     gates actually cover ordering before deleting, and keep this script if
     they do not).
   - The dangling reference to a nonexistent `scripts/release_gate.sh` in the
     current `.claude/agents/publisher.md` (confirmed absent from the repo
     and uncalled by any workflow) is resolved by deleting that whole file,
     not by creating the missing script.
   Retain (repository-owned, not managed by the installer, no path
   collision):
   - `release/RELEASE-NOTES-TEMPLATE.md`, `release/release-inventory.json`
     and their waiver-record process — the shared package has no equivalent
     concept. Reconcile `release/release-inventory.json`'s artifact list
     against the newly rendered `release/publish-artifacts.toml` before this
     sprint closes.
   - `docs/release-readiness-checklist.md` and other independent
     build/qualification checks not specific to the publish mechanism itself.
5. **Install and diff-verify.** Run
   `install.py --dry-run --input install.json .` first and review every
   printed diff; then run the real install; then re-run `--dry-run` and
   confirm it reports "Publish-kit assets are in sync." with exit 0.
6. **Reconcile CI action-version currency (PHC-006).** The shared package's
   workflows pin `actions/checkout@v4` and, where used,
   `actions/setup-python@v5` (confirmed by grep across every workflow file in
   `plugins/sc-publish/.github/workflows/`). This repository has already
   adopted `actions/checkout@v5`/`actions/setup-python@v6` in
   `.github/workflows/windows-identity-preflight.yml`. Record an explicit
   decision: accept the shared package's pinned versions for the installed
   release/publish workflows as a tracked, time-boxed gap (these workflows
   are manual `workflow_dispatch` only, not scheduled or PR-triggered, so
   there is no CI-blocking exposure), and open a tracked follow-up against
   `../sc-publish` to bump its pinned action versions. Do not hand-patch the
   installed workflow files to change action versions — that diverges from
   the shared-source-of-truth model and would be silently reverted by the
   next install/diff cycle.
7. **Credential/auth documentation (PHC-005).** Document in
   `docs/publishing.md`, without exposing values, the exact secrets each
   enabled channel requires: `crates_io` uses the existing crates.io publish
   token convention already in place for this repo; `pypi` requires
   `PYPI_API_TOKEN` and `TEST_PYPI_API_TOKEN` repository secrets (confirmed
   from `pypi-publish.yml`'s `maturin upload --repository ... ` steps — no
   trusted-publishing/OIDC flow exists in the shared package); the added npm
   workflow requires `NPM_TOKEN`. State plainly that this sprint does not
   provision, rotate, or verify the presence of any secret value — only
   documents which secret names each active channel expects.

## Acceptance criteria (authoritative)

- `install.py --dry-run --input install.json .` reports no drift after
  installation.
- `python3 .github/scripts/release_artifacts.py validate-manifest --manifest release/publish-artifacts.toml --workspace-toml Cargo.toml`
  passes (every publishable workspace crate present).
- `python3 .github/scripts/release_artifacts.py validate-preflight-checks --manifest release/publish-artifacts.toml --workspace-toml Cargo.toml`
  passes (chained crates use `"locked"`).
- `release/publish-artifacts.toml` and `release/publish-channel-contracts.toml`
  are valid TOML and contain every crate/wheel/channel decision from
  deliverable 2, with none silently defaulted from the discovery heuristic.
- Every path listed for deletion in deliverable 4 is gone from the tree; every
  path listed for retention still exists and is reconciled.
- The npm publish resolution from deliverable 3 exists, is documented, and
  does not collide with any installer-managed path (re-verified by the
  dry-run in deliverable 5 showing it untouched).
- `docs/publishing.md` reflects the shared-package sourcing, the pinned
  revision, the npm resolution, the action-version reconciliation decision,
  and the per-channel secret-name documentation.
- No crates.io, PyPI, npm, GitHub Release, Homebrew, Scoop, or Winget
  publish command is executed; no tag is created or pushed.

## Required validation

- `bash scripts/ci/validate_docs_consistency.sh` (or its equivalent at
  execution time) passes.
- The two manifest-validation commands under acceptance criteria above pass.
- `install.py --dry-run` passes clean as stated above.
- `cargo metadata --no-deps --format-version 1` still resolves cleanly after
  any workspace-member changes implied by this sprint (there should be none;
  this sprint only touches release/CI/agent assets).

## Paths to delete

- `.claude/agents/publisher.md`
- `.github/workflows/release.yml`
- `.github/workflows/release-preflight.yml`
- `scripts/release_artifacts.py`
- `scripts/ci/validate_publish_order.sh` (conditional — see deliverable 4)

## Non-closure

- Does not publish anything or create a release tag.
- Does not run BTIT repository integration tests.
- Does not upstream an npm channel into `../sc-publish`; the npm resolution
  here is a repository-local workaround pending that upstream work.
- Does not change the pinned action versions inside the shared package's
  installed workflow files; that is tracked as a follow-up, not fixed here.
- Does not change any crate's `Cargo.toml` `publish` field; manifest
  `publish`/`required` decisions for this sprint live only in
  `release/publish-artifacts.toml`.
