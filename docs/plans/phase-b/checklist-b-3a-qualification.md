# B.3a qualification checklist

Owner: bp-tauri-helper. Branch: feature/phase-b-tauri-qualification.
Direct parent: feature/phase-b-7-publish-bindings. Production owner: lobs.

Each row requires implementation and a separate verification against retained raw evidence.
No row is complete from compilation or a mocked transport alone.

| Contract | Implement | Verify | Evidence required |
| --- | --- | --- | --- |
| AC1 packed npm consumer + packaged Rust adapter | implemented | Linux/macOS/Windows all PASS (run 35303041402) | archive hashes, isolated resolved dependencies, install/build/run logs |
| AC1 four client commands + host-owned level command | implemented | Linux/macOS/Windows all PASS (run 35303041402) | actual desktop webview IPC reports on Linux/macOS/Windows |
| AC1 correlated Rust/frontend records + JSONL | implemented | Linux/macOS/Windows all PASS (run 35303041402) | query and retained JSONL, exact bigint values and trusted provenance |
| AC2 all DTO/Failure/Remediation and narrowing | implemented | Linux/macOS/Windows all PASS (run 35303041402) | canonical fixtures and compile-time exhaustive consumer |
| AC2 integer/null/UTC/path/order/schema evolution | implemented | Linux/macOS/Windows all PASS (run 35303041402) | fixture outcomes preserving exact payloads |
| AC3 target/window/direct-invoke authorization | implemented | Linux/macOS/Windows all PASS (run 35303041402) | main and forbidden webview command reports |
| AC3 size/depth/fields/provenance/redaction | implemented | Linux/macOS/Windows all PASS (run 35303041402) | raw invoke negative cases and stored redaction |
| AC3 dispatch/helper saturation, timeout/late completion, responsive I/O | implemented | Linux/macOS/Windows all PASS (run 35303041402) | bounded slot/thread evidence and event-loop progress |
| AC3 every public failure path and local retained status | implemented | Linux/macOS/Windows all PASS (run 35303041402) | fault cases, hidden exception/rejection detector |
| AC4 level transitions, baseline/caps/lifecycle/diagnostics | implemented | Linux/macOS/Windows all PASS (run 35303041402) | core/bridge/frontend coherent snapshots, owner race reports |
| AC5 generation drift and dependency/API invariants | implemented | Linux/macOS/Windows all PASS (run 35303041402) | schema/dependency/boundary/docs checks |
| AC5 confined prepublication source-bundle consumer | implemented | Linux/macOS/Windows all PASS (run 35303041402) | B.3/B.4a bundle verification and isolation probes |
| Required operational-control-flow scan | implemented | Linux/macOS/Windows all PASS (run 35303041402) | authored TS/Rust scan with test-only findings distinguished |
| API-COVERAGE + handoff + platform/hash ledger | implemented | Linux/macOS/Windows all PASS (run 35303041402) | exact test references, raw artifacts and source SHA |

Implementation pass is complete against the fixture inventory in
`scripts/ci/fixtures/tauri-qualification/required-evidence-cases.json`.
The separate verification pass at `71215ca4c519c91b3f454f1c429a576c761e5972`
has passed Linux/macOS CI and local macOS against the identical CI-produced npm and
Rust archives. Each includes 258 ordinary real IPC assertions, 20 capped release
IPC assertions, 368 installed-client/helper cases, 15 policy cases, and canonical
native debug/release fixtures. The platform gate also passes seven bundle tests,
seven aggregate rejection tests, eight sandbox tests and seven Windows-supervisor tests, and six adapter tests.
Raw macOS evidence is retained in the ATM team share at
`b3a-evidence/qualification-71215ca-macos`; Linux is retained in CI run
`35232045864`. The aggregate has verified their hashes and required inventories,
and correctly rejects missing Windows evidence.

Final verification requires one source revision and identical npm/Rust archive
hashes across Linux/macOS/Windows. The strict aggregate rejects missing custom
fault, policy, native lifecycle, forbidden-window, capped or ordinary IPC cases.
That aggregate has since passed in full at source `c6d794c5d8c12a69938b2ec3ccd1cec24d1abd18`
(run 35303041402) -- see `handoff-b-3a-qualification.md`'s "Terminal
qualification" section. Lead completeness decision: **PASS** (aobs, 2026-09-18).

Production followups were consumed from lobs, including all six boundary
reproductions, transport/helper exports, publishable package metadata,
nonblocking owner/shutdown ownership, and typed native shutdown diagnostics.
