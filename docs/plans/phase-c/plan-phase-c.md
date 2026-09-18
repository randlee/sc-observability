---
phase: C
status: draft
branch: plan/phase-c-sc-publish
worktree: /Users/randlee/github/sc-observability-worktrees/plan/phase-c-sc-publish
---
# Phase C — Shared publishing migration

Owner scope: two sprints, executed only after Phase B merges into develop.
This branch is planning only; do not install or execute publication while planning.

Sprints:

- [`sprint-c-1-shared-pipeline-migration.md`](./sprint-c-1-shared-pipeline-migration.md) —
  replace the repository-specific publishing implementation with the pinned
  `../sc-publish` shared package; inventory and remove conflicting legacy
  assets; resolve the npm channel gap.
- [`sprint-c-2-release-surface-preflight.md`](./sprint-c-2-release-surface-preflight.md) —
  preflight the complete post-merge Phase B release surface through the
  migrated pipeline. `must_follow` C.1: preflight requires the installed
  shared manifest/channel contract and the resolved npm step from C.1; merge
  C.1's pushed development forward before every C.2 round, not gated on C.1's
  QA.

Requirements/architecture deltas: `docs/requirements.md` §11 (PHC-001–006),
[ADR-016](../../architecture.md#adr-016-shared-publishing-pipeline-adoption).

1. Replace the repository-specific publishing implementation with the shared
   package from ../sc-publish. Inventory and explicitly remove conflicting legacy
   publishing workflows, agent prompts, skills, scripts and obsolete references;
   retain independent build/qualification checks. Use a caller-owned release
   contract with explicit dependency-ordered crates and deliberate channel settings.
2. Preflight the complete Phase B release surface: Rust crates, Python wheels/sdist,
   npm client and applicable native/Tauri artifacts. Verify package completeness,
   deterministic manifest inventory, authentication setup without exposing tokens,
   platform qualification, recovery/idempotency and post-publish verification.
   Preflight must not publish. Actual publication requires later authorization.

Inspect the merged Phase B integration tree as the release-surface source, because
this planning branch starts at develop before Phase B lands. At execution, update
from post-Phase-B develop and reconcile the inventory before enabling workflows.
Pin and document the reviewed shared-pipeline revision and its supported channels.
Any unsupported Phase B channel needs an explicit resolution and acceptance test;
never silently omit it or claim unsupported functionality exists.

Confirmed shared-package facts as of the pinned revision
`3a57926b3e939835644aad944a614cbb48e4d5fc` (see sprint C.1 for the full
inventory and resolution):

- `install.py`'s `CHANNEL_NAMES` is exactly `github_release`, `crates_io`,
  `pypi`, `homebrew`, `scoop`, `winget` — no npm channel exists.
- The PyPI channel workflow uses `secrets.PYPI_API_TOKEN` /
  `secrets.TEST_PYPI_API_TOKEN` via `maturin upload`; it does not assume
  trusted (OIDC) publishing.
- Every shared workflow pins `actions/checkout@v4` and, where used,
  `actions/setup-python@v5`. This repo has already adopted
  `actions/checkout@v5`/`actions/setup-python@v6` in at least one Phase B
  workflow (`windows-identity-preflight.yml`); installing the shared
  workflows verbatim would reintroduce the older, Node20-based action
  versions in the newly-installed release/publish workflows.

Sequence: Phase B review and merge → Phase C shared-pipeline migration/preflight →
authorized publication → BTIT repository integration tests after publication.
BTIT tests and actual publishing are not authorized by this planning request.

## Non-closure / explicitly out of scope

- Actual publication to any channel, tag creation, and running the release
  workflow are not authorized by either Phase C sprint.
- BTIT repository integration tests remain out of scope until after
  authorized publication.
- Upstream changes to the `../sc-publish` repository itself (for example,
  adding an npm channel or bumping its pinned action versions) are out of
  scope for this repo's sprints; where a shared-package limitation blocks a
  clean adoption, the sprint records an explicit repository-local resolution
  or an explicit, owner-accepted deferral instead of editing the shared repo.
- Go and future sc-runtime publishing surfaces remain deferred per Phase B.

Planning deliverables: complete plan and two sprint docs, requirements and
architecture deltas, exact legacy-to-shared asset replacement/deletion inventory,
channel/artifact matrix, commands and observable acceptance criteria, failure and
recovery handling, dependency gates and docs/index updates. Preserve historical
Phase B evidence and explicit owner approval deferrals.
