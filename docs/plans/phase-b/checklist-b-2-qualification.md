# B.2 qualification implementation and verification checklist

Owner: bp-package-helper. Branch: `feature/phase-b-2-qualification`.
No live publication; lead completeness review is required before task closure.

| Requirement | Implementation pass | Independent verification pass / evidence |
| --- | --- | --- |
| D1: six-package 1.4.0 train, exact bridge/macros pin, licenses and metadata | implemented | final evidence being recorded |
| D1: immutable archives, normalized manifests, candidate SHA/checksums | implemented | final evidence being recorded |
| D1: preserve B.P2 records and B.1 import provenance; release-only variation record | implemented | final evidence being recorded |
| D2: per-crate API/semver handling, proc macros, never-published baseline, private exclusion | implemented | final evidence being recorded |
| D2: scoped approvals cannot mask tool errors or unrelated crate failures | implemented | final evidence being recorded |
| D3: full ordered preflight staging on candidate branch; no publish/tag | implemented | final evidence being recorded |
| D4: isolated exact-version consumer resolves all six verified archives | implemented | final evidence being recorded |
| D4: enabled macro, flush, shutdown and JSONL assertions | implemented | final evidence being recorded |
| AC3: same immutable artifacts consumed on macOS/Linux/Windows | implemented | final evidence being recorded |
| Integrity negatives: tamper, wrong version/source, ambient paths, private leakage | implemented | final evidence being recorded |
| All sprint validation commands and raw logs | implemented | final evidence being recorded |
| Latest parent merge, source corrections inherited, lead completeness PASS | implemented | final evidence being recorded |
