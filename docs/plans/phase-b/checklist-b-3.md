# B.3 implementation and verification checklist

Each row requires separate implementation and verification evidence. No row is waived by an earlier compiling increment.

| Criterion | Implement | Verify | Evidence |
| --- | --- | --- | --- |
| All contracted wire declarations, explicit Serde shapes and defaults | implemented | verification in progress | |
| Decimal signed/unsigned canonical limits, finite values, paths | implemented | verification in progress | |
| Checked event/query/level input, native constructors, UTC, bounds | implemented | verification in progress | |
| Request 64KiB/depth32, recursive normalized provenance rejection | implemented | verification in progress | |
| Complete stored events, health, diagnostics and level conversions | implemented | verification in progress | |
| All Failure/remediation/result/envelope variants and remote handling | implemented | verification in progress | |
| Binding error registry unique literals and faithful projections | implemented | verification in progress | |
| Optional exact Schemars, isolated locked generator, input/output schema | implemented | verification in progress | |
| Local refs/entrypoints/hints/canonicalization/schema-Serde fixtures | implemented | verification in progress | |
| TS schema-only declarations and validators, deterministic goldens | implemented | verification in progress | |
| Python schema-only frozen data/stubs, integer mapping and goldens | implemented | verification in progress | |
| Conformance corpus including every AC3 edge and additive outputs | implemented | verification in progress | |
| API coverage, public API approval, dependency/ownership governance | implemented | verification in progress | |
| Real source bundle checksums/archives/vendor/patches/frozen lock | implemented | verification in progress | |
| Isolated offline external conversion consumer and confined metadata | implemented | verification in progress | |
| Missing member/stale lock/escaping archive and manifest negatives | implemented | verification in progress | |
| Schema CI deterministic generation, drift and toolchain/hash proof | implemented | verification in progress | |
| Required validators, Rust sweep, handoff hashes and completeness | implemented | verification in progress | |

| B3-C01: preserve source-lock registry versions/checksums across staging | implemented | passed | reviewed-source.lock closure equals staged Cargo.lock; changed selection regression returns BUNDLE_REGISTRY_DRIFT |
| B3-C02: mandatory first-party versions and normalized requirement equivalence | implemented | passed | direct/workspace path-only regressions return BUNDLE_MISSING_VERSION; normalized drift returns BUNDLE_REQUIREMENT_DRIFT |
