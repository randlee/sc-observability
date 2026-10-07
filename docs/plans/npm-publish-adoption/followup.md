---
status: in-progress
branch: fix/npm-scope-publish-adoption-followup
worktree: /Users/randlee/github/sc-observability-worktrees/fix/npm-scope-publish-adoption-followup
---
# Complete shared installer adoption

Lead: aobs. Developer: cobs. Parent: PR199/fix/npm-scope-publish-adoption at db49780c70be64464be806d4fc292e74cdaaa2c5, frozen.

## Checklist

- [x] Correct the inherited production-wheel consumer harness discovery error: production validation excludes `test_runtime_faults.py`, while the six-test private suite runs against a distinct `maturin --features test-hooks` companion wheel. Production publication remains hook-free.
- [x] Run the production consumer harness and focused feature-wheel validation. `bash scripts/ci/validate_binding_registry_consumers.sh` passes its 83 production Python tests; isolated CPython 3.10.21 companion execution passes all 6 private fault tests.
- [x] Adopt final scoped combined sc-publish revision `22137c2da13bf4638b4267b69c6c2f021617da73` through its actual immutable installer; no hand-copying managed assets.
- [x] Update `release/sc-publish-pin.toml` and all installer-managed assets/provenance consistently, retaining corrected `@synaptic-canvas/sc-observability` identity.
- [x] Repeat installer dry-run with zero drift; installed publish-kit suite passes 273 tests (14 skipped, 43 subtests). Relevant package/consumer checks pass; no publication or registry mutation performed.
- [x] Opened ready child PR200 above frozen PR199; current head is reported in the handoff. Parent remains unchanged.

## Evidence

- Immutable installer source: `https://github.com/randlee/sc-publish` at
  `22137c2da13bf4638b4267b69c6c2f021617da73`.
- Installer command: `python3 plugins/sc-publish/install.py --input install.json .`;
  repeat `--dry-run` exits 0 with `Publish-kit assets are in sync.`
- Installed suite: `pytest -q .github/scripts/tests` => 273 passed, 14 skipped,
  43 subtests.
- Companion command: `maturin build --locked --features test-hooks` under
  CPython 3.10.21, then isolated `pytest test_runtime_faults.py` => 6 passed.
- Child PR: https://github.com/randlee/sc-observability/pull/200.

## Boundaries

No release/version bump, production dispatch, registry mutation, tag/asset replacement, or secret inspection. Historical evidence preserved. Shared package owns target mechanics/agents; local validation lives in declared extension points. Fully reported failures never receive an overall pass. Full fenced JSON required, including commands, exit codes, sanitized errors and evidence URLs.
