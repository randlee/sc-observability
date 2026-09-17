# Tauri logging consumer

This is a real IPC consumer for the B.3a package. Rust retains the logger
owner and installs `sc-observability-tauri`; the TypeScript side receives only
tagged JSON envelopes and cannot create, close, or mutate the logger.

The frontend uses `@sc-observability/client` with a Tauri `invoke` transport:

```ts
import { invoke } from "@tauri-apps/api/core";
import { createClient, createTauriTransport, encodeEvent } from "@sc-observability/client";

const transport = createTauriTransport(invoke);
const client = transport.kind === "ok" ? createClient(transport.value) : transport;
const event = encodeEvent({ level: "info", target: "tauri-example", action: "startup", fields: { attempt: 1n } });
if (client.kind === "ok" && event.kind === "ok") void client.value.tryLog(event.value);
```

The application also registers `app_observability_level_change` itself. It
uses the authorized main window and a host-selected `user_request` source;
that command is intentionally outside the plugin.

To run in a Tauri application, install the pinned TypeScript package from
`bindings/typescript`, supply the normal Tauri `tauri.conf.json`, and call
`tauri::Builder::default().plugin(sc_observability_tauri::plugin(...))`.
