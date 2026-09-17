# B.3a qualification checklist

Owner: bp-tauri-helper. Branch: feature/phase-b-tauri-qualification.
Direct parent: feature/phase-b-7-publish-bindings. Production owner: lobs.

Each row requires implementation and a separate verification against retained raw evidence.
No row is complete from compilation or a mocked transport alone.

| Contract | Implement | Verify | Evidence required |
| --- | --- | --- | --- |
| AC1 packed npm consumer + packaged Rust adapter | pending | pending | archive hashes, isolated resolved dependencies, install/build/run logs |
| AC1 four client commands + host-owned level command | pending | pending | actual desktop webview IPC reports on Linux/macOS/Windows |
| AC1 correlated Rust/frontend records + JSONL | pending | pending | query and retained JSONL, exact bigint values and trusted provenance |
| AC2 all DTO/Failure/Remediation and narrowing | pending | pending | canonical fixtures and compile-time exhaustive consumer |
| AC2 integer/null/UTC/path/order/schema evolution | pending | pending | fixture outcomes preserving exact payloads |
| AC3 target/window/direct-invoke authorization | pending | pending | main and forbidden webview command reports |
| AC3 size/depth/fields/provenance/redaction | pending | pending | raw invoke negative cases and stored redaction |
| AC3 dispatch/helper saturation, timeout/late completion, responsive I/O | pending | pending | bounded slot/thread evidence and event-loop progress |
| AC3 every public failure path and local retained status | pending | pending | fault cases, hidden exception/rejection detector |
| AC4 level transitions, baseline/caps/lifecycle/diagnostics | pending | pending | core/bridge/frontend coherent snapshots, owner race reports |
| AC5 generation drift and dependency/API invariants | pending | pending | schema/dependency/boundary/docs checks |
| AC5 confined prepublication source-bundle consumer | pending | pending | B.3/B.4a bundle verification and isolation probes |
| Required operational-control-flow scan | pending | pending | authored TS/Rust scan with test-only findings distinguished |
| API-COVERAGE + handoff + platform/hash ledger | pending | pending | exact test references, raw artifacts and source SHA |

Production followups are owned by lobs: six lead boundary reproductions,
Tauri transport/helper exports, adapter packaging metadata, bridge host ownership.
