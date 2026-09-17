# Binding API coverage

| Surface | Evidence |
| --- | --- |
| Generated schema/types and runtime validation | `bindings/typescript/src/generated/index.ts`; `scripts/ci/validate_binding_schema.sh` |
| `encodeValue` null, boolean, string, finite float, safe integer, bigint, arrays and objects | `bindings/typescript/src/encoding.ts`; `bindings/typescript/src/test.ts` |
| BigInt min/max, unsafe numbers, negative zero, cycles, depth, size, symbols, getters and protected keys | `bindings/typescript/src/test.ts` and `bindings/tauri/src/lib.rs` boundary tests |
| `createClient`, `log`, `tryLog`, `query`, `health`, `client_status`, `flush` | `bindings/typescript/src/client.ts`; `bindings/typescript/src/test.ts` |
| Fixed in-flight dispatch and failure counters | `bindings/typescript/src/client.ts` (`reserve`, `recordFailure`); client status smoke assertions |
| Tauri request validation, authorization, target allowlist and redaction | `bindings/tauri/src/lib.rs` strict-request, policy, target and recursive-redaction paths/tests |
| Exact IPC command names and host-owned lifecycle | `bindings/tauri/src/lib.rs`; `examples/tauri-logging/src-tauri/src/main.rs` |
| Tauri plugin ACL and app command registration | `bindings/tauri/build.rs`; `bindings/tauri/permissions/default.toml`; `examples/tauri-logging/src-tauri/build.rs`; `examples/tauri-logging/src-tauri/capabilities/default.json` |
| Exported Tauri invoke transport, application level helper, and ergonomic event | `bindings/typescript/src/client.ts`; `examples/tauri-logging/src/main.ts`; `examples/tauri-logging/README.md` |
| Source package and adapter gates | `scripts/ci/validate_typescript_bindings.sh`; `bindings/typescript/src/test.ts`; `bindings/tauri/src/lib.rs` focused tests |

The native backend remains the shared B.3b `HostLoggingBackend`; this layer
does not duplicate core conversion or logger ownership. Full installed-package,
real-IPC, platform, and broad qualification evidence is owned by the
`bp-tauri-helper` specialist.
