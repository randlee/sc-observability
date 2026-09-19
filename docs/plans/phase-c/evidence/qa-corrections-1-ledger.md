# Phase C QA corrections ledger

Source report: [PR183 QA report](https://github.com/randlee/sc-observability/pull/183#issuecomment-5738364140)
Correction review: [PR184](https://github.com/randlee/sc-observability/pull/184)

This ledger records the disposition of every PHC-QA finding. “Fixed-candidate”
means the correction is present on this branch and requires lead QA; it is not
an independent QA approval. External items remain open until their owner
provides evidence.

| ID | Disposition | Evidence / correction |
| --- | --- | --- |
| PHC-QA-001 | fixed-candidate | C.1 inventory and completion evidence identify `7b899fea` as the current pin; `b2f18cf` and older pins are explicitly historical/superseded. |
| PHC-QA-002 | fixed-candidate | C.1 acceptance uses the installed manifest/order/lockstep commands and no longer names the nonexistent `validate-preflight-checks` command. |
| PHC-QA-003 | fixed-candidate | Publishing contract explicitly names `NPM_TOKEN` in the GitHub `npm` environment. |
| PHC-QA-004 | partially fixed/external | C.2 closure records Windows real-IPC completion; npm environment configuration and lead completeness remain external/open. |
| PHC-QA-005 | fixed-candidate | Obsolete `scripts/ci/tests/test_release_artifacts.py` was removed; installed `.github/scripts/tests/test_release_artifacts.py` is authoritative. |
| PHC-QA-006 | fixed-candidate | QA agent references use `.github/scripts/release_artifacts.py validate-publish-order`; deleted shell path is no longer invoked. |
| PHC-QA-007 | fixed-candidate | C.1 disposition table lists each companion manifest, policy, helper, and adaptation path. |
| PHC-QA-008 | fixed-candidate | Binding manifest records current Tauri/PyPI status and the npm `NPM_TOKEN` contract with only owner authorization/environment pending. |
| PHC-QA-009 | fixed-candidate | Staging archive inspection is shared through `_log_staging.inspect_archive` with explicit package/macro parameters. |
| PHC-QA-010 | fixed-candidate | CI and `just lint` run action-version and install-contract validators. |
| PHC-QA-011 | fixed-candidate | CI runs the full installed script suite; `just test` runs the caller-owned staged-package and retry-idempotency suites. |
| PHC-QA-012 | fixed-candidate | npm retry fixture archives are created under a per-test temporary directory. |
| PHC-QA-013 | fixed-candidate/upstream-verified | Immutable installer regeneration from upstream `7b899fea2325b6bda55a5d061f2c507366246974` replaced local timeout edits; repeat `install.py --dry-run` reports `Publish-kit assets are in sync.` Upstream CI 35413991243 and source/consumer suites are recorded by the lead. |
| PHC-QA-014 | fixed-candidate | `scripts/release_bindings_artifacts.py` now names the shared `.github/scripts/release_artifacts.py` helper in its module docstring. |

## Verification record

The regenerated installed script suite passed `168 passed, 11 skipped`; the
focused regenerated publish-kit tests passed `87 passed, 8 skipped`. Targeted
archive/staging tests (including workspace-inheritance and nine-crate
first-party-version negative guards) passed `30/30`; targeted package/retry
tests passed `11/11`. The remaining external gates are the
GitHub `npm` environment/secret and lead completeness approval; this ledger
does not claim either gate is complete.
