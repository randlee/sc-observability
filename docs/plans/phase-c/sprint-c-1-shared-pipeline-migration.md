---
id: C.1
status: complete
branch: fix/phase-c-1-shared-pipeline-migration
worktree: /Users/randlee/github/sc-observability-worktrees/fix/phase-c-1-shared-pipeline-migration
base: develop
---

# C.1 — Shared publishing pipeline migration

## Completion evidence

The superseded C.1 qualification kit was `006092a305bb03bd483d79dd6b5d51bda1e545e4`;
a later regeneration validated `7b899fea2325b6bda55a5d061f2c507366246974`
(`168 passed, 11 skipped`; see
`docs/plans/phase-c/evidence/c2-sc-publish-7b899fea.txt`), and the current
validated immutable kit pinned in `release/sc-publish-pin.toml` is
`22137c2da13bf4638b4267b69c6c2f021617da73`. Its installer reports
`Publish-kit assets are in sync.` on the repeat dry-run. The historical C.1
suite passed `157 passed, 11 skipped, 0 failed`; that retained evidence and
exact command are in `docs/plans/phase-c/evidence/c1-installed-suite-006092a.txt`.
Manifest, dependency-order, action-floor, docs, rustdoc, semantic-contract,
and locked workspace-test gates pass. No publication, tag, or release dispatch
was performed.

## Goal and dependencies

Replace this repository's bespoke, repository-specific publishing
implementation with the pinned `../sc-publish` shared package, installed only
through its caller-owned `install.py` JSON contract.

Phase B merged into `develop` is a hard execution prerequisite for this
sprint, not a `parallel_safe` relation: this sprint's branch bases on
post-Phase-B `develop`, and its manifest rewrite covers Phase B's crates, so
it structurally cannot start before that merge lands. `must_follow`: none
other inside Phase C; C.2 `must_follow`s this sprint (see C.2's own
dependency section for the full merge-forward/PR-completion contract). No
publication, tagging, or BTIT integration test runs in this sprint.

**Upstream prerequisites, owned by `../sc-publish`, not this repository:**
this sprint additionally requires that the `../sc-publish` revision selected
for install already provides (a) an npm publish channel and (b) current,
non-deprecated pinned CI action versions in its shared workflows — see
deliverables 3 and 6. Neither is satisfied by a repository-local workaround.
If upstream work cannot land before Phase C needs to execute, this sprint
stops on the technical compatibility gate and reports the gap; it does not
substitute a local workaround or treat an unvalidated revision as equivalent
adoption. Phase C migration itself is already authorized; the lead selects the
reviewed/validated upstream pin after the compatibility evidence is complete.

## Deliverables (authoritative)

Every listed deliverable must land production-ready for this sprint's stated
scope. Completion requires evidence for every numbered item; partial
completion leaves the sprint open.

1. **Pin the shared-package revision.** Record the exact reviewed commit of
   `../sc-publish` used for installation in this doc and in
   `docs/publishing.md`. The revision inspected while writing this plan,
   `3a57926b3e939835644aad944a614cbb48e4d5fc`, does **not** satisfy the
   upstream prerequisites in deliverables 3 and 6 (confirmed: no npm channel
   in `install.py`'s `CHANNEL_NAMES`; every shared workflow pins
   `actions/checkout@v4`). Selecting a pin for actual execution requires a
   later `../sc-publish` revision that adds both, or the owner escalation
   path above. Re-verify the pin is still the intended revision immediately
   before running the installer.
2. **Author a reviewed `install.json`.** Run
   Create a complete caller-owned `install.json` and hand-review every field
   before use. The older pinned installer offered `--example-json` discovery,
   but the current candidate contract no longer does; no source-discovery
   output is an acceptable installation input. In particular:
   - **Preserve the full ten-crate Phase B publish inventory** unless the
     owner explicitly approves an exclusion — `sc-observability-types`,
     `sc-observability`, `sc-observe`, `sc-observability-otlp`,
     `sc-observability-log`, `sc-observability-log-macros`,
     `sc-observability-dto`, `sc-observability-binding-runtime`,
     `sc-observability-tauri`, and the `sc-observability-py` PyO3 Rust crate
     (crates.io remains its intended publish target as a Rust source crate;
     this is a **separate** `[[crates]]` manifest entry from its wheel/sdist
     `[[artifacts.wheels]]` entry — the two are different channels for the
     same crate, and this sprint must declare both, not one in place of the
     other). `sc-observability-log-consumer-check` has `publish = false` in
     its own `Cargo.toml` and stays excluded.
   - For each of the ten crates, explicitly decide and record `publish`/
     `required`/`preflight_check`/`verify_install`/
     `wait_after_publish_seconds`. `sc-observability-dto`,
     `sc-observability-binding-runtime`, `sc-observability-tauri`, and
     `sc-observability-py` currently carry no explicit `publish` field in
     their own `Cargo.toml` (Cargo therefore defaults them to publishable);
     record an explicit crates.io publish decision for each in the manifest
     rather than silently carrying the Cargo default forward. `preflight_check`
     is `"full"` for crates with no workspace path dependency and `"locked"`
     for chained crates that depend on other workspace members.
   - **`bindings/tauri` is a standalone Cargo workspace, not a member of the
     root workspace** (`bindings/tauri/Cargo.toml` declares its own empty
     `[workspace]` table, confirmed by reading it directly). Root
     `cargo metadata`/`cargo package -p sc-observability-tauri` will not
     enumerate or select it. Record its manifest entry's `cargo_toml` as the
     explicit path `bindings/tauri/Cargo.toml`, and use
     `--manifest-path bindings/tauri/Cargo.toml` (never root `-p`) in every
     packaging/validation command that targets it, in this sprint and C.2.
   - `artifacts.wheels`: `sc-observability-py` (`python_package`
     `sc_observability`) — the wheel/sdist distribution entry, distinct from
     its crates.io source-crate entry above.
   - `artifacts.binaries`: this repo currently ships no standalone release
     binary target; leave empty unless a Phase B deliverable adds one.
   - `channels`: root crates.io/GitHub Release channels are implicit in the
     current candidate installer; declare opt-in post-release `pypi` and npm
     channels only when their upstream contracts are present. Record every
     decision explicitly; do not rely on the older heuristic defaults.
3. **npm publish capability — upstream prerequisite (PHC-002).**
   The stale pinned installer lacked npm; the current upstream candidate adds
   npm as an opt-in post-release channel. The TypeScript client
   (`@sc-observability/client`,
   `bindings/typescript/package.json`) needs a publish path, but that
   capability belongs in `../sc-publish` itself, consumed here at a reviewed
   pin, matching how every other channel is adopted:
   - Request (tracked outside this repo, referenced here by an explicit
     description since this sprint cannot edit `../sc-publish`) an npm
     channel in the shared `publish-channel-contracts.toml.j2`, modeled on
     the existing `pypi` channel's shape: `stage = "post_release"`, a
     dedicated `npm-publisher` agent, and `environment_secrets = [{ environment =
     "npm", name = "NPM_TOKEN" }]` (the GitHub `npm` environment must hold
     that secret), `public_registry_checks = true`
     against the npm registry API, and a `.github/workflows/npm-publish.yml`
     that publishes already-built artifacts from an immutable GitHub
     Release, matching `pypi-publish.yml`'s pattern.
   - Once that lands and is pinned, enable `channels.npm` in this
     repository's `install.json` exactly like any other shared channel — no
     bespoke repository-local publish workflow, agent, or secret convention.
   - If this upstream work cannot land before Phase C needs to execute,
     stop and escalate to the owner per the prerequisite note above; do not
     substitute a local fork as if it satisfied shared-package adoption.
4. **Exact legacy asset replacement/removal/retention table.** One
   authoritative disposition per path — no path appears in more than one
   category:

   | Path | Disposition |
   | --- | --- |
   | `.claude/agents/publisher.md` | Overwritten in place by the installer's copy of `plugins/sc-publish/.claude/agents/publisher.md` (same path); no manual delete step. |
   | `.claude/agents/{crates-io,github-release,homebrew,pypi,scoop,winget}-publisher.md`, `.claude/agents/publisher-channel-protocol.md`, `.claude/skills/publishing/**` | New paths added by the installer; nothing to remove first. |
   | `.github/workflows/release.yml`, `.github/workflows/release-preflight.yml` | Overwritten in place by the installer's same-named shared workflows; no manual delete step. |
   | `.github/workflows/{homebrew,pypi,scoop,winget}-publish.yml`, `.github/actions/**` | New paths added by the installer. |
   | `scripts/release_artifacts.py` | **Delete.** No same-path replacement exists — the shared package installs its equivalent at the different path `.github/scripts/release_artifacts.py`. Every reference to the old path in retained docs/scripts must be updated, not left dangling. |
   | `scripts/ci/validate_publish_order.sh` | **Delete, unconditionally.** Confirmed by reading `.github/scripts/release_manifest.py`'s `validate_publish_order()`: the shared `validate-publish-order` subcommand checks actual workspace dependency-graph ordering (every crate's `publish_order` exceeds every crate it depends on), which is strictly stronger than this script's own uniqueness/sortedness-only check. This is not a conditional decision. |
   | `.claude/agents/publisher.md`'s dangling reference to a nonexistent `scripts/release_gate.sh` | Resolved by the same-path overwrite in the row above: the installed content is the shared package's `publisher.md`, which contains no `scripts/release_gate.sh` reference. `publisher.md` itself is never deleted — only its content changes. The **authoritative** byte-parity gate is `install.py`'s own `--dry-run` (deliverable 5: it reports "Publish-kit assets are in sync." only when every installer-managed path, `publisher.md` included, already matches the pinned revision's copy — this is the installer's own diff logic, not a re-derived one). An independent manual spot-check, if needed, must use an explicit absolute path to the reviewed sibling checkout — never a bare `../sc-publish`-style relative path or a `plugins/sc-publish/...` path assumed nested inside this consumer repo, since neither resolves correctly from an arbitrary nested worktree: `diff .claude/agents/publisher.md "$SC_PUBLISH_CHECKOUT/plugins/sc-publish/.claude/agents/publisher.md"`, where `$SC_PUBLISH_CHECKOUT` is set to the absolute path of the sibling checkout pinned at the exact reviewed revision (deliverable 1) before running the command. This is kept distinct from the **absence check** for the two genuinely-deleted paths above (`test ! -e scripts/release_artifacts.py`, `test ! -e scripts/ci/validate_publish_order.sh`). |
   | `release/RELEASE-NOTES-TEMPLATE.md`, `release/release-inventory.json` | **Retain**, repository-owned — the shared package has no equivalent concept and does not touch these paths. Reconcile `release-inventory.json`'s artifact list against the newly rendered `release/publish-artifacts.toml` before this sprint closes. |
   | `docs/release-readiness-checklist.md` and other independent build/qualification checks not specific to the publish mechanism | **Retain.** |
   | `release/bindings-artifacts.toml`, `release/bp2-publish-artifacts.toml`, `release/public-api-policy.json`, `release/python-platform-policy.json`, `release/runtime-level-qualification.toml` | **Retain**, Phase B companion qualification/policy manifests; reconcile their current status fields independently and do not merge them into the shared publish manifest. |
   | `scripts/release_bindings_artifacts.py`, `scripts/ci/tests/test_release_bindings_artifacts.py`, `scripts/ci/tests/test_bindings_evidence.py`, `scripts/ci/_log_release_adaptations.py` | **Retain**, Phase B binding/evidence tooling outside shared publish-channel plumbing; keep its tests and references intact. |
5. **Install and diff-verify.** Run
   `install.py --dry-run --input install.json .` first and review every
   printed diff; then run the real install; then re-run `--dry-run` and
   confirm it reports "Publish-kit assets are in sync." with exit 0.
6. **CI action-runtime currency — upstream prerequisite, with a validation
   gate (PHC-006).** The shared package's workflows pin
   `actions/checkout@v4` and, where used, `actions/setup-python@v5`
   (confirmed by grep across every workflow file in
   `plugins/sc-publish/.github/workflows/`), which this repository has
   already moved past elsewhere (`actions/checkout@v5`/
   `actions/setup-python@v6` in `.github/workflows/windows-identity-preflight.yml`).
   This is not accepted as a tracked-but-shipped gap: the pin selected in
   deliverable 1 must come from a `../sc-publish` revision whose installed
   workflows meet this repository's current action-currency floor
   (`actions/checkout>=v5`, `actions/setup-python>=v6`, and no other action
   below the newest major version already adopted anywhere in this repo's
   own `.github/workflows/`). Add
   `scripts/ci/validate_publish_workflow_action_versions.sh`, run in this
   sprint's own validation, that greps every installed
   `.github/workflows/release*.yml` and `.github/workflows/*-publish.yml`
   for `uses: actions/<name>@v<N>` and fails if any pinned major version is
   below this repository's current floor for that action. If no such
   `../sc-publish` revision exists yet, escalate to the owner per the
   prerequisite note above instead of installing the stale pin and shipping
   a follow-up ticket in its place.
7. **Credential/auth documentation (PHC-005), environment-scoped.**
   Document in `docs/publishing.md`, without exposing values, the exact
   secret scope each enabled channel requires, confirmed by reading
   `plugins/sc-publish/release/publish-channel-contracts.toml.j2` directly
   rather than assumed:
   - `crates_io`: `repository_secrets = ["CARGO_REGISTRY_TOKEN"]` —
     repository-scoped, matching this repo's existing crates.io publish
     token convention.
   - `pypi`: `environment_secrets` — `PYPI_API_TOKEN` in the `pypi`
     environment and `TEST_PYPI_API_TOKEN` in the `testpypi` environment
     (not repository secrets; `pypi-publish.yml`'s jobs select
     `environment: pypi` or `environment: testpypi`). No trusted-publishing/
     OIDC flow exists in the shared package.
   - `npm`: secret name(s) and scope are defined by the upstream channel
     contract once deliverable 3 lands; this sprint does not invent them.
   This sprint does not provision, rotate, or verify the presence of any
   secret value — only documents which secret names, and at which scope,
   each active channel expects.

## Acceptance criteria (authoritative)

- `install.py --dry-run --input install.json .` reports no drift after
  installation.
- `python3 .github/scripts/release_artifacts.py validate-manifest --manifest release/publish-artifacts.toml --workspace-toml Cargo.toml`
  passes for all ten crates (every publishable workspace crate present,
  including `sc-observability-tauri` via its explicit `bindings/tauri/Cargo.toml`
  path and `sc-observability-py`'s crates.io entry).
- The installed CLI has no `validate-preflight-checks` subcommand; the
  equivalent manifest, publish-order, version-lockstep, and rendered workflow
  checks pass as recorded by C.2 evidence.
- `python3 .github/scripts/release_artifacts.py validate-publish-order --manifest release/publish-artifacts.toml --workspace-toml Cargo.toml`
  passes (dependency-graph-correct publish order for all ten crates).
- `bash scripts/ci/validate_publish_workflow_action_versions.sh` passes
  (deliverable 6).
- `release/publish-artifacts.toml` and `release/publish-channel-contracts.toml`
  are valid TOML and contain every crate/wheel/channel decision from
  deliverable 2, with none silently defaulted from an obsolete discovery heuristic.
- Every path in deliverable 4's table has the disposition the table states:
  same-path overwrites pass their byte-parity diff check, and the two
  genuinely-deleted paths pass their absence check. No path is both
  installed/overwritten and separately listed for deletion.
- The npm channel is either fully adopted from a landed upstream revision
  (enabled in `install.json` like any other channel, secrets documented per
  its own contract) or this sprint is explicitly held open pending the
  owner escalation from the prerequisite note — never a local workaround
  presented as equivalent.
- `docs/publishing.md` reflects the shared-package sourcing, the pinned
  revision, the npm channel's actual adopted contract, the confirmed
  action-currency floor, and the per-channel, per-scope secret-name
  documentation.
- No crates.io, PyPI, npm, GitHub Release, Homebrew, Scoop, or Winget
  publish command is executed; no tag is created or pushed.

## Required validation

- `bash scripts/ci/validate_docs_consistency.sh` (or its equivalent at
  execution time) passes.
- The three manifest-validation commands and the action-version validation
  command under acceptance criteria above pass.
- `install.py --dry-run` passes clean as stated above.
- `cargo metadata --no-deps --format-version 1` still resolves cleanly for
  the root workspace; `cargo metadata --no-deps --format-version 1 --manifest-path bindings/tauri/Cargo.toml`
  resolves cleanly for the standalone Tauri workspace.

## Paths to delete

- `scripts/release_artifacts.py`
- `scripts/ci/validate_publish_order.sh`
- `.claude/agents/publisher.md` is not separately deleted — it is
  overwritten in place by the installer (see deliverable 4's table); listing
  it here would contradict that disposition.

## Non-closure

- Does not publish anything or create a release tag.
- Does not run BTIT repository integration tests.
- Does not edit `../sc-publish` directly from this repository's worktree;
  the npm channel and action-currency prerequisites are upstream work this
  sprint requests and consumes at a pin, not implements itself.
- Does not change any crate's `Cargo.toml` `publish` field; manifest
  `publish`/`required` decisions for this sprint live only in
  `release/publish-artifacts.toml`.
