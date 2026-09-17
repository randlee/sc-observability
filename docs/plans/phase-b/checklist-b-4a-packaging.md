# B.4a distribution implementation and verification checklist

Owner: bp-package-helper. Branch: feature/phase-b-4a-python-packaging.
Parent: feature/phase-b-4-python. First compiling parent f3fb224 inherited; 2e5f505 runtime corrections inherited.
Lead completeness is required before task closure. No publication.

| Requirement | Implementation pass | Verification pass |
| --- | --- | --- |
| D1: locked maturin1.10.2/PyO3 abi3-py310, complete Python sources/stubs/py.typed | pyproject and pinned build tools implemented | package-data inventory and development macOS wheels pass; final matrix pending |
| D1: sole B.3 source helper, all unpublished first-party closure and locked target registry dependencies | sole helper inherited; complete original manifest and vendor retained | development sdist verified locally and in CI; final source pending |
| D1: normalized versioned manifests and confined root patches, frozen sdist-specific lock | outer extension/host manifests and frozen locks implemented | exact selected registry identities checked; actual offline metadata passes locally |
| D1/AC3: self-contained sdist with source inventory/hashes and offline external rebuild | sole-helper assembly and immutable inventories implemented | development sdist and external macOS rebuild pass; final source pending |
| D2/AC1: one wheel per five native platforms, all five GIL CPython interpreters | policy prepared: release/python-platform-policy.json | all 25 actual cells pending |
| D2/AC2: abi3 tags, Linux manylinux_2_28, macOS 11.0 arm64/10.13 Intel and linked-library inspection | policy prepared | pending |
| D3/AC1: full unchanged installed B.4 API/error/lifecycle/integer/diagnostic suite; no skipped cases | whole tests directory discovery and raw JUnit gates implemented | 12 tests plus strict typing pass locally on development CPython3.14; completed parent contract and 25 cells pending |
| D3: installed stubs and exhaustive Result/Failure type fixture on every interpreter | parent typing fixture discovered through suite typing_paths | all 25 installed checks pending |
| D3/AC2: separate-link Rust embedding external consumer on every target | parent host example bundled with separate normalized manifest/lock | actual external macOS arm64 host passed; other platforms in CI |
| AC3: deny checkout/network/cache fallback; metadata confines every resolution | shared platform sandbox and per-package metadata confinement | actual local macOS denial probes passed; five-platform CI active |
| Negatives: missing unpublished/vendor/target dependency, checksum, stale lock, escaping path | independent disposable copies with actual offline wheel/host failures | all local development negatives pass; final matrix pending |
| Negatives: missing stubs/py.typed, wrong architecture/tag, extension flags in embedding | archive guards, native executable headers and real altered host feature fixture | seven boundary tests and nine actual development negative cases pass |
| AC4: immutable wheel/sdist source/hashes, exact platform/interpreter/targets and per-cell results | pending | pending |
| Full required validation, final parent merge, handoff/sprint/project plan | pending | pending |
| Lead completeness review | pending | pending |

The B.4a plan's phrase “published core packages stay at the B.2 registry
versions” describes the selected versions, not present registry availability:
B.2's 1.4.0 archives are staged and immutable; B.7 alone publishes them. Use
those verified first-party bytes plus vendored locked third-party sources.
Bundle paths are read from the sole helper's manifest (archives/packages/vendor),
so packaging does not fork or rename its checksummed layout.

Development evidence is never final acceptance. Source 19ab74f produced a
self-contained sdist and the actual external macOS arm64 wheel/host proof;
source fb3aa49 is running the first five-platform CI build (35208870501).
The full runtime contract remains incomplete in the active B.4 parent.
Explicit development execution may exercise its present suite, but every such
result is marked development_only and the aggregate unconditionally rejects it.
