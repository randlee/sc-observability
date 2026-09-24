# `@synaptic-canvas/sc-observability`

This package is a generated-schema TypeScript client for the Tauri host
adapter. Install the packed tarball in a clean consumer, pass Tauri's `invoke`
function to `createTauriTransport`, and pass the returned transport to
`createClient`. `log` is bounded fire-and-forget dispatch; use `tryLog` when
the admission result is required. Query, health, and flush resolve tagged
`Result` values and do not reject for operational failures.

The host application must authorize the four plugin commands through its Tauri
capability and must retain any application-owned command, such as a level
change, in the app ACL. The example under `examples/tauri-logging` routes the
four plugin commands plus its application-owned level-change command (five
observability commands total) through ACL for all windows; its handlers retain
the `main`-window policy checks.

Use `encodeValue` and `encodeEvent` for ergonomic event construction. BigInts
are retained as decimal tagged integers; unsafe numbers, cycles, getters,
reserved provenance keys, unsupported objects, oversized requests, and deep
containers return validation failures. The host, not the frontend, stamps
service, identity, timestamps, and protected provenance.

Generated declarations live in `src/generated` and must only be changed by
the canonical schema generator. Run `npm run check-generated` before review.
