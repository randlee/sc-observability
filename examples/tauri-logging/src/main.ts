import { invoke } from "@tauri-apps/api/core";
import { createClient, encodeEvent } from "@sc-observability/client";

const transport = {
  request: (operation: "try_log" | "query" | "health" | "flush", request: unknown) =>
    invoke(`plugin:sc-observability|sc_observability_${operation}`, { request }),
};

const client = createClient(transport);
const event = encodeEvent({
  level: "info",
  target: "tauri-example",
  action: "startup",
  fields: { attempt: 1n },
});
if (client.kind === "ok" && event.kind === "ok") {
  void client.value.tryLog(event.value);
}
