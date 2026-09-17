# Binding API coverage

| Surface | Evidence |
| --- | --- |
| Generated schema/types and runtime validation | `bindings/typescript/src/generated/index.ts`; `scripts/ci/validate_binding_schema.sh` |
| `encodeValue` null, boolean, string, finite float, safe integer, bigint, arrays and objects | `bindings/typescript/src/encoding.ts`; `bindings/typescript/src/test.ts` |
| BigInt min/max, unsafe numbers, negative zero, cycles, depth, size, symbols, getters and protected keys | `bindings/typescript/src/test.ts` and `bindings/tauri/src/lib.rs` policy tests |
| `createClient`, `log`, `tryLog`, `query`, `health`, `client_status`, `flush` | `bindings/typescript/src/client.ts`; `bindings/typescript/src/test.ts` |
| Fixed in-flight dispatch and failure counters | `bindings/typescript/src/client.ts` (`reserve`, `recordFailure`); client status smoke assertions |
| Tauri request validation, authorization, target allowlist and redaction | `bindings/tauri/src/lib.rs`; `bindings/tauri/src/lib.rs` policy test |
| Exact IPC command names and host-owned lifecycle | `bindings/tauri/src/lib.rs`; `examples/tauri-logging/src-tauri/src/main.rs` |
| Real consumer transport and ergonomic event | `examples/tauri-logging/src/main.ts`; `examples/tauri-logging/README.md` |
| Installation, package and adapter gates | `scripts/ci/validate_typescript_bindings.sh` |

The native backend remains the shared B.3b `HostLoggingBackend`; this layer
does not duplicate core conversion or logger ownership.
