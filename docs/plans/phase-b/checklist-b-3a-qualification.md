# B.3a qualification checklist

Owner: bp-tauri-helper. Branch: feature/phase-b-tauri-qualification.
Direct parent: feature/phase-b-7-publish-bindings. Production owner: lobs.

Each row requires implementation and a separate verification against retained raw evidence.
No row is complete from compilation or a mocked transport alone.

| Contract | Implement | Verify | Evidence required |
| --- | --- | --- | --- |
| AC1 packed npm consumer + packaged Rust adapter | implemented | local PASS; final matrix pending | archive hashes, isolated resolved dependencies, install/build/run logs |
| AC1 four client commands + host-owned level command | implemented | local PASS; final matrix pending | actual desktop webview IPC reports on Linux/macOS/Windows |
| AC1 correlated Rust/frontend records + JSONL | implemented | local PASS; final matrix pending | query and retained JSONL, exact bigint values and trusted provenance |
| AC2 all DTO/Failure/Remediation and narrowing | implemented | local PASS; final matrix pending | canonical fixtures and compile-time exhaustive consumer |
| AC2 integer/null/UTC/path/order/schema evolution | implemented | local PASS; final matrix pending | fixture outcomes preserving exact payloads |
| AC3 target/window/direct-invoke authorization | implemented | local PASS; final matrix pending | main and forbidden webview command reports |
| AC3 size/depth/fields/provenance/redaction | implemented | local PASS; final matrix pending | raw invoke negative cases and stored redaction |
| AC3 dispatch/helper saturation, timeout/late completion, responsive I/O | implemented | local PASS; final matrix pending | bounded slot/thread evidence and event-loop progress |
| AC3 every public failure path and local retained status | implemented | local PASS; final matrix pending | fault cases, hidden exception/rejection detector |
| AC4 level transitions, baseline/caps/lifecycle/diagnostics | implemented | local PASS; final matrix pending | core/bridge/frontend coherent snapshots, owner race reports |
| AC5 generation drift and dependency/API invariants | implemented | local PASS; final matrix pending | schema/dependency/boundary/docs checks |
| AC5 confined prepublication source-bundle consumer | implemented | local PASS; final matrix pending | B.3/B.4a bundle verification and isolation probes |
| Required operational-control-flow scan | implemented | local PASS; final matrix pending | authored TS/Rust scan with test-only findings distinguished |
| API-COVERAGE + handoff + platform/hash ledger | implemented | local PASS; final matrix pending | exact test references, raw artifacts and source SHA |

Implementation pass is complete against the fixture inventory in
`scripts/ci/fixtures/tauri-qualification/required-evidence-cases.json`.
The independent verification pass has local evidence at `ac245df`: 244 ordinary
real IPC assertions, 20 capped release IPC assertions, 339 installed-client
cases, 15 policy cases, canonical native debug/release fixtures, and seven
aggregate rejection tests. Raw local evidence is retained under the ATM team
share `b3a-evidence/qualification-ac245df-macos`.

Final verification requires one source revision and identical npm/Rust archive
hashes across Linux/macOS/Windows. The strict aggregate rejects missing custom
fault, policy, native lifecycle, forbidden-window, capped or ordinary IPC cases.
The lead completeness check remains required before task/sprint closure.

Production followups were consumed from lobs, including all six boundary
reproductions, transport/helper exports, publishable package metadata,
nonblocking owner/shutdown ownership, and typed native shutdown diagnostics.
