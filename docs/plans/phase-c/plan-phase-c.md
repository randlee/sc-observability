---
phase: C
status: in_progress
branch: integrate/phase-c
worktree: /Users/randlee/github/sc-observability-worktrees/integrate/phase-c
---
# Phase C — Shared publishing migration

Owner scope: two sprints, executed only after Phase B merges into develop.
Execution authorized by the owner after Phase B and the Phase C plan merged into
`develop` at `28e4ee6`. Lead: `aobs`. Publication remains unauthorized.
C.1 completed inventory, installation, and compatibility qualification against
the immutable shared-package revision recorded in its pin file. Publication
remains separately unauthorized.

Sprints:

- [`sprint-c-1-shared-pipeline-migration.md`](./sprint-c-1-shared-pipeline-migration.md) —
  replace the repository-specific publishing implementation with the pinned
  `../sc-publish` shared package; inventory and remove conflicting legacy
  assets; adopt the npm channel and current action runtimes as named
  upstream prerequisites (not a local workaround or an accepted regression).
- [`sprint-c-2-release-surface-preflight.md`](./sprint-c-2-release-surface-preflight.md) —
  preflight the complete post-merge Phase B release surface through the
  migrated pipeline. `must_follow` C.1: preflight requires the installed
  shared manifest/channel contract and the upstream-adopted npm channel from
  C.1. Merge C.1's pushed development forward before every C.2 round
  (trigger: C.1's dev push, not its QA outcome); C.1's own PR must merge
  before C.2's PR is considered complete.

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

Historical shared-package facts from the superseded revision
`3a57926b3e939835644aad944a614cbb48e4d5fc` are retained below for audit. The
current upstream candidate under review is PR95 (`a99c9a7`); its caller-owned
JSON adds npm and structured wheel entries, but it is not yet a reviewed pin.
See sprint C.1 for the full inventory and resolution:

- The superseded installer's `CHANNEL_NAMES` lacked npm. PR95 adds npm as an
  opt-in post-release channel; adopting it remains a **named upstream
  prerequisite** until that revision is reviewed and pinned, not a
  repository-local publisher.
- The PyPI channel's `environment_secrets` require `PYPI_API_TOKEN` in a
  `pypi` GitHub Environment and `TEST_PYPI_API_TOKEN` in a `testpypi`
  Environment via `maturin upload`; it does not assume trusted (OIDC)
  publishing, and these are Environment-scoped secrets, not repository
  secrets — distinct from crates.io's repository-scoped
  `CARGO_REGISTRY_TOKEN`.
- Every shared workflow pins `actions/checkout@v4` and, where used,
  `actions/setup-python@v5`. This repo has already adopted
  `actions/checkout@v5`/`actions/setup-python@v6` in at least one Phase B
  workflow (`windows-identity-preflight.yml`). C.1 treats a compatible,
  current action-runtime pin in the selected `../sc-publish` revision as a
  **named upstream prerequisite**, gated by an added validation check (see
  sprint C.1 deliverable 6), not an accepted regression shipped with a
  follow-up ticket.

Sequence: Phase B review and merge → Phase C shared-pipeline migration/preflight →
authorized publication → BTIT repository integration tests after publication.
BTIT tests and actual publishing are not authorized by this planning request.

## Non-closure / explicitly out of scope

- Actual publication to any channel, tag creation, and running the release
  workflow are not authorized by either Phase C sprint.
- BTIT repository integration tests remain out of scope until after
  authorized publication.
- This repository's sprints do not edit `../sc-publish` directly — no commit
  in either sprint touches that repository's worktree. Where a shared-package
  limitation blocks clean adoption (the missing npm channel; stale pinned
  action versions), the sprint states the missing capability as a **named
  upstream prerequisite** that must land in `../sc-publish` and be consumed
  at a reviewed pin before the affected sprint can complete. This repository
  does not substitute a local fork or workaround for that capability, and
  does not accept a permanent gap with only a follow-up ticket in its place.
  If the upstream work cannot land before Phase C needs to execute, the
  sprint stops on the technical compatibility gate and reports the gap rather
  than treating a local substitute as equivalent adoption. Phase C execution
  is authorized; the lead selects the reviewed/validated upstream pin once
  the compatibility evidence is complete.
- Go and future sc-runtime publishing surfaces remain deferred per Phase B.

Planning deliverables: complete plan and two sprint docs, requirements and
architecture deltas, exact legacy-to-shared asset replacement/deletion inventory,
channel/artifact matrix, commands and observable acceptance criteria, failure and
recovery handling, dependency gates and docs/index updates. Preserve historical
Phase B evidence and explicit owner approval deferrals.
