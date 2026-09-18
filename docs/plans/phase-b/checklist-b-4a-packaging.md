# B.4a distribution implementation and verification checklist

Owner: bp-package-helper. Branch: feature/phase-b-4a-python-packaging.
Parent: feature/phase-b-4-python. First compiling parent f3fb224 inherited; 2e5f505 runtime corrections inherited.
Lead completeness is required before task closure. No publication.

| Requirement | Implementation pass | Verification pass |
| --- | --- | --- |
| D1: locked maturin1.10.2/PyO3 abi3-py310, complete Python sources/stubs/py.typed | pyproject and pinned build tools implemented | package-data inventory and development macOS wheels passed locally; final matrix passed in run 35303039765 (all 5 wheel jobs) |
| D1: sole B.3 source helper, all unpublished first-party closure and locked target registry dependencies | sole helper inherited; complete original manifest and vendor retained | development sdist verified locally and in CI; final `sdist` job passed in run 35303039765 |
| D1: normalized versioned manifests and confined root patches, frozen sdist-specific lock | outer extension/host manifests and frozen locks implemented | exact selected registry identities checked; actual offline metadata passes locally and in the final `sdist`/`wheel` jobs |
| D1/AC3: self-contained sdist with source inventory/hashes and offline external rebuild | sole-helper assembly and immutable inventories implemented | development sdist and external macOS rebuild pass; final `sdist` job passed in run 35303039765 |
| D2/AC1: one wheel per five native platforms, all five GIL CPython interpreters | policy prepared: release/python-platform-policy.json | all 25 `installed-suite` cells passed in run 35303039765 |
| D2/AC2: abi3 tags, Linux manylinux_2_28, macOS 11.0 arm64/10.13 Intel and linked-library inspection | policy prepared | passed: all 5 `wheel` platform jobs in run 35303039765 |
| D3/AC1: full unchanged installed B.4 API/error/lifecycle/integer/diagnostic suite; no skipped cases | whole tests directory discovery and raw JUnit gates implemented | 12 tests plus strict typing pass locally on development CPython3.14; all 25 cells passed in run 35303039765 with `aggregate` JUnit cross-check |
| D3: installed stubs and exhaustive Result/Failure type fixture on every interpreter | parent typing fixture discovered through suite typing_paths | all 25 `installed-suite` cells passed in run 35303039765 |
| D3/AC2: separate-link Rust embedding external consumer on every target | parent host example bundled with separate normalized manifest/lock | actual external macOS arm64 host passed locally; all 5 platforms passed in run 35303039765 |
| AC3: deny checkout/network/cache fallback; metadata confines every resolution | shared platform sandbox and per-package metadata confinement | actual local macOS denial probes passed; all 25 cells passed the same denial proof in run 35303039765 |
| Negatives: missing unpublished/vendor/target dependency, checksum, stale lock, escaping path | independent disposable copies with actual offline wheel/host failures | all local development negatives pass; these are local-fixture regressions by design (tampering the release artifact itself is out of scope), not a separate final-matrix job |
| Negatives: missing stubs/py.typed, wrong architecture/tag, extension flags in embedding | archive guards, native executable headers and real altered host feature fixture | seven boundary tests and nine actual development negative cases pass |
| AC4: immutable wheel/sdist source/hashes, exact platform/interpreter/targets and per-cell results | implemented (`production-artifacts.json`) | passed: run 35303039765's `aggregate` job and inventory artifact record exact source SHA `c6d794c5d8c12a69938b2ec3ccd1cec24d1abd18`, five wheel hashes and per-cell results; inventory hash `ef492dc1793d68d29ddeebda4afcc9bd87557fd3d65771b08cede24cbe9852f7` independently confirmed |
| Full required validation, final parent merge, handoff/sprint/project plan | implemented | passed: `validate_python_bindings.sh` full matrix in run 35303039765; handoff/sprint docs updated from this evidence |
| Lead completeness review | PASS (aobs, 2026-09-18) | PASS -- recorded in `handoff-b-4a.md` |
| Approved production/fault separation: same sdist, separate feature identity/hash/venv, no production hooks | implemented in build/cell/aggregate; companion excluded from publication inventory | release-gate negative regression passes; actual companion proof awaits parent fault file contract |
| Clean source assembly after runtime tests create ignored Python caches | copy only tracked Python/tests/examples/embedding source; preserve generated local files | tracked-copy regression covers bytecode and optimized caches |

The B.4a plan's phrase “published core packages stay at the B.2 registry
versions” describes the selected versions, not present registry availability:
B.2's 1.4.0 archives are staged and immutable; B.7 alone publishes them. Use
those verified first-party bytes plus vendored locked third-party sources.
Bundle paths are read from the sole helper's manifest (archives/packages/vendor),
so packaging does not fork or rename its checksummed layout.

Development evidence is never final acceptance. Source 19ab74f produced a
self-contained sdist and the actual external macOS arm64 wheel/host proof;
source fb3aa49 ran the first five-platform CI build (35208870501). Those
development runs, and the subsequent failed diagnostic runs `35295219562`
and `35300170241`, are retained as history: every such result is marked
`development_only` and the aggregate unconditionally rejects it. The final
aggregation has since run to completion at source `c6d794c5d8c12a69938b2ec3ccd1cec24d1abd18`
in run 35303039765 (`conclusion: success`, all 33 jobs) -- see
`handoff-b-4a.md`'s "Terminal qualification" section and
`sprint-b-4a-python-packaging.md`'s "Current qualification evidence" for the
verified detail. Development qualification is complete; the lead completeness
review above is recorded PASS. Independent phase-end QA remains separately
pending. This does not claim QA acceptance, API/ADR approval or publication.
