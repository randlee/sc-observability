# Tauri logging consumer

This is a real IPC consumer for the B.3a package. Rust retains the logger
owner and installs `sc-observability-tauri`; the TypeScript side receives only
tagged JSON envelopes and cannot create, close, or mutate the logger.

The frontend uses `@synaptic-canvas/sc-observability` with a Tauri `invoke` transport:

```ts
import { invoke } from "@tauri-apps/api/core";
import { createClient, createTauriTransport, encodeEvent } from "@synaptic-canvas/sc-observability";

const transport = createTauriTransport(invoke);
const client = transport.kind === "ok" ? createClient(transport.value) : transport;
const event = encodeEvent({ level: "info", target: "tauri-example", action: "startup", fields: { attempt: 1n } });
if (client.kind === "ok" && event.kind === "ok") void client.value.tryLog(event.value);
```

The checked-in Tauri capabilities keep core APIs on `main` while routing the
four plugin commands and application level command through ACL for all windows.
The adapter and application handler still apply their own window and target
policies before backend admission, returning tagged permission denial for
forbidden windows.

The application also registers `app_observability_level_change` itself. It
uses the authorized main window and a host-selected `user_request` source;
that command is intentionally outside the plugin.

To run in a Tauri application, install the pinned TypeScript package from
`bindings/typescript`, supply the normal Tauri `tauri.conf.json`, and call
`tauri::Builder::default().plugin(sc_observability_tauri::plugin(...))`.
