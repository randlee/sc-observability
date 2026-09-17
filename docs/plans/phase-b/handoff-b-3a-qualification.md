# B.3a artifact and real-IPC qualification

Status: in progress; no sprint completion or publication claim.
Owner: bp-tauri-helper. Branch: feature/phase-b-tauri-qualification.
Direct PR parent: feature/phase-b-7-publish-bindings (PR 141). Qualification: PR 142.

The isolated consumer installs the actual npm tarball and builds the original
example command source against Cargo-packaged adapter/native crates with a
temporary root patch. It uses B.4a's platform sandbox and independently verifies
checkout/cache/network denial. Exact registry versions/checksums and normalized
dependency requirements remain enforced; standalone adapter archives exclude
their irrelevant library lockfile while the reviewed consuming source lock is
retained separately and checked without relaxing comparison.

Observation hooks add result capture, a secondary webview, a bounded holder of
the existing owner mutex, and an additional real console sink. Pausing the
parent's stdout drain fills an actual OS pipe, blocks the native sink, and
exercises queue saturation, unchanged level handlers, timeout/overlap and late
completion. No mock backend, duplicated conversion, or adapter authorization
bypass is used. Hooks exist only in the staged consumer and never ship in the
npm/adapter production artifacts.

Retained artifacts: npm `.tgz`; Rust `.crate` archives; bundle manifest and
reviewed lock identities; exact frontend/host source hashes; `fault-results.json`;
`ipc.json`; JSONL; raw build/runtime commands; platform metadata. The aggregate
requires Linux/macOS/Windows at one source revision, identical npm hash, valid
archive/file hashes, every canonical fixture and every mandatory real IPC case.
The full shell gate includes schema validation; CI executes the shared schema
and contract gate once, each native platform separately, then requires all four
jobs before aggregation. `--platform` is explicitly a partial matrix stage.

Executed checkpoints:

- 5e5c595: isolated macOS packaged build and runtime passed 83 real IPC assertions
  and all 336 installed-client schema/fault cases. The shared schema/contract CI
  job and macOS CI job passed in run 35214318997. Linux failed on an ambient
  WebKit profile write under the read-only sandbox; Windows failed an existing
  mock IPC test's hardcoded local-origin URL. Both failures were retained and
  addressed for the next run; neither was counted as platform qualification.
- 5a3f380: local isolated macOS passed 103 real IPC assertions, including the
  original exact-size normalization regression and the held-output scenarios,
  plus all 336 installed-client cases. New narrowing/max-counter assertions and
  Linux XDG/Windows origin fixes are queued for the next same-source matrix.

Open gates: final same-source three-platform execution/aggregation, full
lifecycle/owner-shutdown and remaining native fixture correlation, exact
artifact retention for the final source, and independent second checklist pass.
The parent production branch owns further production fixes. No registry has
been published, and the qualification task stays open until all authoritative
requirements and the lead completeness gate pass.
