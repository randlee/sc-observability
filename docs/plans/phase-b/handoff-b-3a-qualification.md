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
requires Linux/macOS/Windows at one source revision, identical npm and Rust
archive hashes, valid
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

- `ac245df`: local isolated macOS passed 244 ordinary IPC assertions, 20 capped
  release-host IPC assertions, all 339 installed-client cases and 15 native host
  policy cases. The actual host-owned shutdown takes the sole guard, returns
  native timeout at zero milliseconds while the real console pipe is held,
  preserves level state, rejects level/admission calls as closed, and reaches
  stopped after pipe release. Native code/remediation survives the timeout
  mapping. Raw evidence is retained in the ATM team share under
  `b3a-evidence/qualification-ac245df-macos`.
- CI run `35216642109` at `d283f87` passed the schema/contracts gate and Linux and
  macOS artifact/IPC jobs; Windows remained running at this handoff checkpoint.
- CI run `35217602809` at `ac245df` passed schema/contracts plus the canonical
  npm/Rust artifact producer; all three platform jobs were running at this
  handoff checkpoint. This matrix consumes the same actual archives everywhere.

The committed case inventory names every required main/forbidden/capped IPC,
custom client fault and native policy assertion. Canonical schema case IDs and
native debug/release CASE/helper-count logs are independently required as well.
The aggregate rejects missing/skipped cases, changed artifacts, mismatched source
revisions and any absent isolation probe. Seven rejection tests exercise the
aggregate gate; seven source-bundle tests exercise lock/confinement behavior.

Final open gates are the same-source three-platform aggregate and lead
completeness check. `--platform` is a partial platform stage and never signals
sprint completion. No registry has been published. B.3a remains in progress until
production and qualification evidence pass the lead's complete review.

The current fixture imports the unchanged production `requestLevelChange`
helper into both actual webviews and an installed-client fault consumer linked
to locked `@tauri-apps/api` 2.11.1. Its inventory additionally requires every
Failure payload, unknown remote preservation, diagnostic bounds and foreign
invoke/response containment. These supplement the earlier 339-case checkpoint;
the current required fault suite has 368 cases. The reusable workflow accepts a
full `source_commit` and returns successful source/inventory hashes plus canonical
npm/Rust artifact names only after the strict three-platform aggregate passes.
