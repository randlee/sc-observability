# B.2 qualification implementation and verification checklist

Owner: bp-package-helper. Branch: `feature/phase-b-2-qualification`.
Implementation source: `136799758a5e91aabbf064ae50b5c7c6f20d4413`.
Two passes: implement each row, then inspect its files and execute the listed
checks. Developer verification is separate from independent QA. No live publication.

| Requirement | Implementation pass | Verification pass / evidence |
| --- | --- | --- |
| D1: six-package 1.4.0 train, exact bridge/macros pin, licenses and metadata | Complete: Cargo workspace, six manifests/LICENSE files, release inventory/version validator | PASS: version and publish-order checks; normalized manifests and all six verified Cargo archives in final stage |
| D1: immutable archives, normalized manifests, candidate SHA/checksums | Complete: prepare_log_staged_packages.py, _log_staging.py | PASS: final stage manifest records source, per-member hashes, normalized manifests; re-verification rejects tampering |
| D1: preserve B.P2 and B.1 history, explicit release variations | Complete: separate bp2 inventory and release-adaptations-b-2.json; opt-in strict import composition | PASS: historical+warning+release full proof; 52 import tests including unrecorded-file rejection; no historical provenance edits |
| D2: per-crate API/semver, proc macros, initial baselines, private exclusion | Complete: public-api-policy.json, validate_public_api.py and wrappers | PASS: four 1.2.0 minor semver baselines; two initial API surfaces; all six stdout hashes match actual dated approval |
| D2: approvals cannot mask execution or unrelated failures | Complete: scoped crate/version/hash approval, distinct tool-error exit and unconditional semver failure | PASS: approval negative cases in 17 staging tests; docs gate passed with original scoped aobs record |
| D3: full ordered candidate preflight, no live publish/tag | Complete: release-preflight and B.2 qualification workflows; bounded future index gate | PASS: Cargo verifies all six; explicit selection ignores extra public packages, requires six public and private consumer private; future index retry tests |
| D4: isolated exact-version consumer using all six archives | Complete: validate_log_staged_consumer.py and metadata checks | PASS: actual resolution paths and consumed-byte hashes in final per-platform JSON; private consumer absent |
| D4: enabled macro, flush, shutdown, JSONL | Complete: accepted-API executable fixture | PASS: per-platform raw logs and assertions, exact emitted message and fields |
| AC3: same immutable artifacts on macOS/Linux/Windows | Complete: single stage plus three-platform and aggregate jobs | PASS: run 35204643293 stage + macOS/Linux/Windows + aggregate all successful; retained byte re-verification passed |
| Integrity negatives | Complete: archive, source/version, traversal, ambient resolution, private leakage, scoped approval, future index cases | PASS: 17 staging tests; 52 provenance tests; raw logs retained |
| Sprint validation commands | Complete: local required commands plus full package/consumer CI | PASS: final evidence local-gates/results.json and API logs; one exploratory import call omitted required source argument, then full explicit invocation passed (both retained) |
| Parent/source correction incorporation | Complete: definitive c001cc6 parent merged; B.1 copy repair and migration included | PASS: ancestry and final source SHA; no further parent chasing |
| Lead completeness and independent QA | Pending lead review after final evidence push | Pending; no lead PASS or independent QA verdict claimed by developer |
