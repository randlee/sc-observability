# B.3a TypeScript/Tauri handoff

Status: in progress on `fix/phase-b-3a-completeness`; parent merge-forwarded
from `origin/feature/phase-b-4a-python-packaging` at `8acf9b6`.

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

Validation performed:

```text
bash scripts/ci/validate_binding_schema.sh       PASS
bash scripts/ci/validate_typescript_bindings.sh  PASS (packed install + adapter IPC test)
cargo test --manifest-path bindings/tauri/Cargo.toml --locked --features test  PASS (4 tests)
```

The package is packed as a real tarball and installed by a temporary consumer
outside the checkout. Generated output is checked without overwriting drift.
The workflow covers Ubuntu, macOS, and Windows. Local headless evidence covers
locked host compilation and the adapter boundary; real desktop IPC evidence is
still required from those supported CI hosts before this handoff can become
complete.
