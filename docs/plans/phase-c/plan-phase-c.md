---
phase: C
status: draft
branch: plan/phase-c-sc-publish
worktree: /Users/randlee/github/sc-observability-worktrees/plan/phase-c-sc-publish
---
# Phase C — Shared publishing migration

Owner scope: one or two sprints, executed only after Phase B merges into develop.
This branch is planning only; do not install or execute publication while planning.

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

Sequence: Phase B review and merge → Phase C shared-pipeline migration/preflight →
authorized publication → BTIT repository integration tests after publication.
BTIT tests and actual publishing are not authorized by this planning request.

Planning deliverables: complete plan and one or two sprint docs, requirements and
architecture deltas, exact legacy-to-shared asset replacement/deletion inventory,
channel/artifact matrix, commands and observable acceptance criteria, failure and
recovery handling, dependency gates and docs/index updates. Preserve historical
Phase B evidence and explicit owner approval deferrals.
