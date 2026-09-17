# B.3 implementation and verification checklist

Each row requires separate implementation and verification evidence. No row is waived by an earlier compiling increment.

| Criterion | Implement | Verify | Evidence |
| --- | --- | --- | --- |
| All contracted wire declarations, explicit Serde shapes and defaults | implemented | passed | wire.rs;291 schema/Serde/generator cases; API-COVERAGE |
| Decimal signed/unsigned canonical limits, finite values, paths | implemented | passed | conversions.rs decimal_domains/nonfinite/paths; newline goldens |
| Checked event/query/level input, native constructors, UTC, bounds | implemented | passed | decode_event/query/level tests;13 exact-kind/code semantic fixtures |
| Request 64KiB/depth32, recursive normalized provenance rejection | implemented | passed | exact_request_size_and_depth_boundaries; recursive normalized spoof tests |
| Complete stored events, health, diagnostics and level conversions | implemented | passed | all_stored_event_fields;complete_health_projection;level change fixtures |
| All Failure/remediation/result/envelope variants and remote handling | implemented | passed | every union schema fixture; unknown/malformed/oversized remote tests |
| Binding error registry unique literals and faithful projections | implemented | passed | registry_has_unique_literals;errors-v1 projection from canonical registry |
| Optional exact Schemars, isolated locked generator, input/output schema | implemented | passed | locked compiler;schema-gen feature;input/output required-field fixtures |
| Local refs/entrypoints/hints/canonicalization/schema-Serde fixtures | implemented | passed | compiler supported/local-ref checks; frozen schema-cases |
| TS schema-only declarations and validators, deterministic goldens | implemented | passed | two fresh TS runs;291 Node validator cases; drift/unsupported negatives |
| Python schema-only frozen data/stubs, integer mapping and goldens | implemented | passed | two fresh Python runs;frozen records;exact Python int projection;stubs |
| Conformance corpus including every AC3 edge and additive outputs | implemented | passed | 17 Rust tests +291 schema cases +13 exact-code semantic cases |
| API coverage, public API approval, dependency/ownership governance | implemented | passed | phase-b-dto.json actual lead approval;dependency/boundary/docs gates |
| Real source bundle checksums/archives/vendor/patches/frozen lock | implemented | passed | b3-final/manifest.json;exact archives;both frozen locks;25 registry identities |
| Isolated offline external conversion consumer and confined metadata | implemented | passed | b3-final/isolated-consumer.json: all sandbox probes and public conversion stdout |
| Missing member/stale lock/escaping archive and manifest negatives | implemented | passed | six actual mutated-artifact negatives plus direct/workspace source checks |
| Schema CI deterministic generation, drift and toolchain/hash proof | implemented | passed | binding-schema.yml; generation-manifest.json; schema/language drift negatives |
| Required validators, Rust sweep and handoff hashes | implemented | passed | b3-final/gates/results.json: nine exit0; fresh approved API digest match |
| B3-C01: preserve source-lock registry versions/checksums across staging | implemented | passed | reviewed-source.lock closure equals staged Cargo.lock; changed selection regression returns BUNDLE_REGISTRY_DRIFT |
| B3-C02: mandatory first-party versions and normalized requirement equivalence | implemented | passed | direct/workspace path-only regressions return BUNDLE_MISSING_VERSION; normalized drift returns BUNDLE_REQUIREMENT_DRIFT |
| B3-C03: generated Python3.10 runtime and strict stub compatibility | implemented | passed | NoReturn, dataclass_field alias; actual CPython3.10.21 import and strict mypy2.3.1 target3.10 |

Lead completeness acceptance is requested after this independent verification pass; task close remains with that gate.
