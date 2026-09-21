---
status: in-progress
branch: fix/npm-scope-publish-adoption
worktree: /Users/randlee/github/sc-observability-worktrees/fix/npm-scope-publish-adoption
---
# npm scope and shared publishing adoption

Lead: aobs. Developer: cobs. Base: origin/develop, as requested by the owner.

## Scope

Adopt the reviewed sc-publish reporting-contract and npm organization follow-up through the immutable installer. Correct the active npm identity to `@synaptic-canvas/sc-observability`. Publication at version 1.4.1 has already succeeded; this task does not publish, bump versions, or modify immutable tags/assets. Do not merge diagnostic or recovery-only workflow overrides.

## Deliverables

- Align TypeScript package and lockfile names, install.json inventory, both release manifests, install-contract validator, active consumer docs/examples/imports, and affected qualification/tests with the corrected identity.
- Inspect remaining old-name references, preserving historical evidence rather than rewriting it. Update current README/skills where relevant.
- Adopt the reviewed immutable upstream child revision stacked above sc-publish PR103 once available, regenerate managed assets using its installer, and retain source SHA plus installer/check evidence. Prepare local identity fixes while upstream is in progress.
- Preserve generic configurable publishing support and independent channel retries. Record recovery provenance separately from immutable original release artifacts.
- Open a ready PR targeting develop; report head and URL immediately, followed by validation evidence. Do not merge or publish.

## Validation

Run focused npm package/type/build and affected consumer tests, manifest/inventory checks, installer drift validation, installed publish-kit tests, docs consistency, and git diff --check as applicable. Retain concrete commands and outcomes. No production workflow dispatch. Send each completed fix for independent QA immediately; reports belong on the PR.

## Evidence

Successful npm recovery run: https://github.com/randlee/sc-observability/actions/runs/35549256399

Publication evidence: https://github.com/randlee/sc-observability/pull/198#issuecomment-5754069077

Upstream reporting fix: https://github.com/randlee/sc-publish/pull/103

The corrected archive SHA-256 is d93d38568f8f86b56266fafc12e802de579666ba5ed6c1922492bb9dce0916c9. Public version, tarball integrity, and latest=1.4.1 were independently verified.
