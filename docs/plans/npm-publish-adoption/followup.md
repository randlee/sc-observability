---
status: in-progress
branch: fix/npm-scope-publish-adoption-followup
worktree: /Users/randlee/github/sc-observability-worktrees/fix/npm-scope-publish-adoption-followup
---
# Complete shared installer adoption

Lead: aobs. Developer: cobs. Parent: PR199/fix/npm-scope-publish-adoption at db49780c70be64464be806d4fc292e74cdaaa2c5, frozen.

## Checklist

- [ ] Correct the inherited production-wheel consumer harness discovery error: five tests in test_runtime_faults.py require a distinct test-hooks wheel. Follow the existing validate_python_distribution.py production/private companion separation. Preserve production tests and prove private hooks absent; preserve feature-wheel fault validation separately. Do not enable private hooks in publication artifacts or merely suppress legitimate failures.
- [ ] Run the production consumer harness and focused feature-wheel validation, retain exact commands/status and distinguish inherited baseline failures. This work can proceed before upstream is approved.
- [ ] Adopt the eventual corrected combined sc-publish revision through its actual immutable installer. Current917b5cf is rejected, not a final pin. No hand-copying managed assets. Upstream child fix/combined-publish-review is active; source candidate may be installed in the worktree for validation, but final completion requires aobs/solar/clint exact-pin agreement.
- [ ] Update release/sc-publish-pin.toml plus all installer-managed assets/provenance consistently. Keep corrected @synaptic-canvas/sc-observability identity, active docs and local configuration.
- [ ] Repeat installer dry-run reports in sync; complete installed publish-kit suite, relevant manifests/docs/tests pass; no unaccounted drift or mismatched pin.
- [ ] Open a ready PR above PR199, register stack and report exact head/PR immediately. Send QA handoff immediately once implemented. Parent remains unchanged.

## Boundaries

No release/version bump, production dispatch, registry mutation, tag/asset replacement, or secret inspection. Historical evidence preserved. Shared package owns target mechanics/agents; local validation lives in declared extension points. Fully reported failures never receive an overall pass. Full fenced JSON required, including commands, exit codes, sanitized errors and evidence URLs.
