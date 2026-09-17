# B.3a TypeScript/Tauri handoff

Status: in progress on `fix/phase-b-3a-completeness`; parent merge-forwarded
from `origin/feature/phase-b-4a-python-packaging` at `7a785bf` (including
`b3d90dd`).

The child consumes the B.3 canonical schema and the B.3b native
`HostLoggingBackend`. The TypeScript package is generated-schema driven and
exports `createClient`, `encodeValue`, and `encodeEvent`. Operational paths
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
plugin and retains owner authority in Rust. The example demonstrates the
frontend transport and BigInt event conversion; the host composition point is
`examples/tauri-logging/src-tauri/src/main.rs`.

Focused source validation performed:

```text
bash scripts/ci/validate_binding_schema.sh       PASS
bash scripts/ci/validate_typescript_bindings.sh  PASS (schema/package source checks)
cargo test --manifest-path bindings/tauri/Cargo.toml --locked --features test  PASS (4 tests)
```

The TypeScript tests cover the packaged client’s source-level encoding,
transport, failure-containment, version rejection, diagnostic bounds, proxy
containment, additive output evolution, prototype-safe encoding, and lifecycle
boundaries. The adapter tests
exercise strict request policy, redaction, and the Tauri command dispatcher
through its mock IPC harness. Full installed Rust/npm artifacts, real desktop
IPC, platform CI, and broad C05 qualification are delegated to
`bp-tauri-helper` on `feature/phase-b-tauri-qualification`; this handoff does
not claim those gates. B.3a remains in progress pending that specialist
evidence and explicit lead completeness PASS.
