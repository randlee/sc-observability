# B.4a distribution implementation and verification checklist

Owner: bp-package-helper. Branch: feature/phase-b-4a-python-packaging.
Parent: feature/phase-b-4-python. First compiling parent f3fb224 inherited.
Lead completeness is required before task closure. No publication.

| Requirement | Implementation pass | Verification pass |
| --- | --- | --- |
| D1: locked maturin1.10.2/PyO3 abi3-py310, complete Python sources/stubs/py.typed | pending | pending |
| D1: sole B.3 source helper, all unpublished first-party closure and locked target registry dependencies | pending parent helper inheritance | pending |
| D1: normalized versioned manifests and confined root patches, frozen sdist-specific lock | pending | pending |
| D1/AC3: self-contained sdist with source inventory/hashes and offline external rebuild | pending | pending |
| D2/AC1: one wheel per five native platforms, all five GIL CPython interpreters | policy prepared: release/python-platform-policy.json | all 25 actual cells pending |
| D2/AC2: abi3 tags, Linux manylinux_2_28, macOS 11.0 arm64/10.13 Intel and linked-library inspection | policy prepared | pending |
| D3/AC1: full unchanged installed B.4 API/error/lifecycle/integer/diagnostic suite; no skipped cases | awaiting parent runtime suite | pending |
| D3: installed stubs and exhaustive Result/Failure type fixture on every interpreter | awaiting parent typed fixture | pending |
| D3/AC2: separate-link Rust embedding external consumer on every target | awaiting parent embedding example | pending |
| AC3: deny checkout/network/cache fallback; metadata confines every resolution | pending | pending |
| Negatives: missing unpublished/vendor/target dependency, checksum, stale lock, escaping path | pending | pending |
| Negatives: missing stubs/py.typed, wrong architecture/tag, extension flags in embedding | pending | pending |
| AC4: immutable wheel/sdist source/hashes, exact platform/interpreter/targets and per-cell results | pending | pending |
| Full required validation, final parent merge, handoff/sprint/project plan | pending | pending |
| Lead completeness review | pending | pending |

The B.4a plan's phrase “published core packages stay at the B.2 registry
versions” describes the selected versions, not present registry availability:
B.2's 1.4.0 archives are staged and immutable; B.7 alone publishes them. Use
those verified first-party bytes plus vendored locked third-party sources.
Bundle paths are read from the sole helper's manifest (archives/packages/vendor),
so packaging does not fork or rename its checksummed layout.
