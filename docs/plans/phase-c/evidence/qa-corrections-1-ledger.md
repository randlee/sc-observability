# Phase C QA corrections ledger

Source report: [PR183 QA report](https://github.com/randlee/sc-observability/pull/183#issuecomment-3314656659)

This ledger records the disposition of every PHC-QA finding. “Fixed-candidate”
means the correction is present on this branch and requires lead QA; it is not
an independent QA approval. External items remain open until their owner
provides evidence.

| ID | Disposition | Evidence / correction |
| --- | --- | --- |
| PHC-QA-001 | fixed-candidate | C.1 inventory and completion evidence identify `b2f18cf` as the current pin; old pins are explicitly historical/superseded. |
| PHC-QA-002 | fixed-candidate | C.1 acceptance uses the installed manifest/order/lockstep commands and no longer names the nonexistent `validate-preflight-checks` command. |
| PHC-QA-003 | fixed-candidate | Publishing contract explicitly names `NPM_TOKEN` in the GitHub `npm` environment. |
| PHC-QA-004 | fixed-candidate | C.2 closure records Windows real-IPC completion and leaves only npm environment/lead completeness open. |
| PHC-QA-005 | fixed-candidate | Obsolete `scripts/ci/tests/test_release_artifacts.py` was removed; installed `.github/scripts/tests/test_release_artifacts.py` is authoritative. |
| PHC-QA-006 | fixed-candidate | QA agent references use `.github/scripts/release_artifacts.py validate-publish-order`; deleted shell path is no longer invoked. |
| PHC-QA-007 | fixed-candidate | C.1 disposition table lists each companion manifest, policy, helper, and adaptation path. |
| PHC-QA-008 | fixed-candidate | Binding manifest records current Tauri/PyPI status and the npm `NPM_TOKEN` contract with only owner authorization/environment pending. |
| PHC-QA-009 | fixed-candidate | Staging archive inspection is shared through `_log_staging.inspect_archive` with explicit package/macro parameters. |
| PHC-QA-010 | fixed-candidate | CI and `just lint` run action-version and install-contract validators. |
| PHC-QA-011 | fixed-candidate | CI and `just test` run the installed script suite plus the staged-package and retry-idempotency suites. |
| PHC-QA-012 | fixed-candidate | npm retry fixture archives are created under a per-test temporary directory. |
| PHC-QA-013 | upstream-verified | The corrected immutable upstream sc-publish pin is already adopted as `b2f18cf`; no local fork or installer-managed override was introduced. |
| PHC-QA-014 | fixed-candidate | Production archive/build subprocesses have bounded timeouts; test subprocesses use bounded timeouts where applicable. |

## Verification record

The installed script suite passed `165 passed, 11 skipped`. Targeted staged
package/retry tests passed `11/11`. The remaining external gates are the
GitHub `npm` environment/secret and lead completeness approval; this ledger
does not claim either gate is complete.
