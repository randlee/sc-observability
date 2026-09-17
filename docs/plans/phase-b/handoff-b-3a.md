# B.3a TypeScript/Tauri handoff

Status: in progress on `fix/phase-b-3a-completeness`; parent merge-forwarded
from `origin/feature/phase-b-4a-python-packaging` at `c403bcf` (including
the latest B.4a merge-forward). Current child checkpoint: `ce5bb92`.

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
level command. The adapter query observer uses the fixed 2000 ms deadline.
The specialist checkpoint `5a3f380` reports 103 isolated macOS IPC assertions
and 336 packed-client schema/fault cases PASS, including the exact-size
correction. Full installed Rust/npm artifacts, platform CI, and broad C05
qualification remain delegated to `bp-tauri-helper` on
`feature/phase-b-tauri-qualification`; this handoff does not claim those
gates. B.3a remains in progress pending specialist retest and explicit lead
completeness PASS.
