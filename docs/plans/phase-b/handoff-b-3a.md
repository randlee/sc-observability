# B.3a TypeScript/Tauri handoff

Status at checkpoint `2076600` on `fix/phase-b-3a-completeness` (parent
merge-forwarded from `origin/feature/phase-b-4a-python-packaging` at
`c403bcf`, including the latest B.4a merge-forward): superseded by
`handoff-b-3a-qualification.md`'s terminal qualification and the recorded
development-complete decision.

The child consumes the B.3 canonical schema and the B.3b native
`HostLoggingBackend`. The TypeScript package is generated-schema driven and
exports `createClient`, `createTauriTransport`, `encodeValue`, and `encodeEvent`.
Operational paths
resolve tagged `Result` values, with bounded dispatch status and retained
failure diagnostics. The isolated Tauri adapter validates raw JSON before
backend admission, authorizes every command by window label, enforces target
allowlists, recursively redacts configured fields, and stamps the trusted
Tauri origin through the shared backend.

Exact plugin commands are:

* `plugin:sc-observability|sc_observability_try_log`
* `plugin:sc-observability|sc_observability_query`
* `plugin:sc-observability|sc_observability_health`
* `plugin:sc-observability|sc_observability_flush`

The application-owned `app_observability_level_change` remains outside the
plugin and retains owner authority in Rust. The example exports a nonrejecting
`requestLevelChange` helper, uses the shared Tauri transport, and demonstrates
BigInt event conversion; the host composition point is
`examples/tauri-logging/src-tauri/src/main.rs`.
The plugin ships generated Tauri ACL metadata for all four plugin commands;
the example build registers its application command. Its checked-in
capabilities keep core APIs on `main` while routing only the five observability
commands through ACL on all windows, allowing handler-level permission denial.

Focused source validation performed:

```text
bash scripts/ci/validate_binding_schema.sh       PASS
bash scripts/ci/validate_typescript_bindings.sh  PASS (schema/package source checks)
cargo test --manifest-path bindings/tauri/Cargo.toml --locked --features test  PASS (5 tests)
node /tmp/b3a-boundary-check.cjs <built-client-dist>  PASS (6/6 exact lead cases)
cargo check --manifest-path examples/tauri-logging/src-tauri/Cargo.toml --locked  PASS
```

The TypeScript tests cover the packaged client’s source-level encoding,
transport, failure-containment, version rejection, diagnostic bounds, proxy
containment, additive output evolution, prototype-safe encoding, lifecycle
boundaries, response-version mapping, remote remediation preservation, and
best-effort accounting faults. The adapter tests
exercise strict request policy, redaction, and the Tauri command dispatcher
through its mock IPC harness, including a portable platform-origin invocation,
native Tauri window-label and native target-category policy validation, and an
exact-64-KiB request with omitted
nullable event fields. Raw request sizing now occurs before DTO defaulting, so
serde normalization cannot reject an otherwise in-bound request. The source
example owns a `LogGuard`, shares its
control with the adapter backend, and emits correlated Rust/frontend startup
records; its ACL build resolves the four plugin commands plus the application
level command. The isolated example lock is refreshed for the native validator
dependencies, and its locked build passes. The adapter query observer uses the
fixed 2000 ms deadline. The production owner path uses `OwnerState::shutdown`
from the host `RunEvent::ExitRequested` callback: it takes the sole
`Mutex<Option<LogGuard>>` owner with a nonblocking try-lock, maps contention to
typed `DISPATCH_FULL`, then releases the mutex before bounded guard shutdown;
retained `LogControl` observes the stopped report and level requests return
tagged `CLOSED` after ownership is consumed.
Native shutdown failures retain their native diagnostic code and remediation:
timeouts map to binding `TIMEOUT`, final-flush failures to `IO`, helper-start
failures to `UNAVAILABLE`, and helper-loss failures to `INTERNAL`; no display
text is parsed to classify them.

Independent phase-end QA remains pending; formal API/ADR approval and publication remain deferred to B.7.

The example bundle now includes a deterministic 64x64 `icons/icon.ico` derived
from the checked-in PNG and lists it in `tauri.conf.json`, satisfying the
Windows resource build without adding new imagery.
The application-owned `requestLevelChange` helper delegates response handling
to the shared `parseWireEnvelope` conversion, preserving unknown remote failure
kinds/remediations and rejecting oversized known diagnostics with
`DIAGNOSTIC_TOO_LARGE`.
The shared parser also contains throwing getters and revoked proxies across
the complete response boundary, returning typed `INTERNAL`/validation results
instead of allowing fulfilled hostile responses to reject the helper promise.
The Tauri raw-request depth inspector now rejects a container at the configured
limit while allowing a primitive leaf at that depth, matching the shared DTO
boundary (32 containers accepted; the 33rd container rejected).
The specialist checkpoint `5a3f380` reports 103 isolated macOS IPC assertions
and 336 packed-client schema/fault cases PASS, including the exact-size
correction. Full installed Rust/npm artifacts, platform CI, and broad C05
qualification were delegated to `bp-tauri-helper` on
`feature/phase-b-tauri-qualification`; this handoff does not itself claim
those gates -- see `handoff-b-3a-qualification.md`'s "Terminal qualification"
section, where the specialist retest passed (run 35303041402) and the lead
recorded development completeness PASS (aobs, 2026-09-18).
