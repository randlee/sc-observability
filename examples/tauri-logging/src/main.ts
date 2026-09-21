import { invoke } from "@tauri-apps/api/core";
import {
  createClient,
  createTauriTransport,
  encodeEvent,
  err,
  parseWireEnvelope,
  safeFailure,
  type LevelChangeDto,
  type LevelRequestDto,
  type Result,
} from "@synaptic-canvas/sc-observability";

const transport = createTauriTransport(invoke);

function levelResponse(value: unknown): Result<LevelChangeDto> {
  return parseWireEnvelope<LevelChangeDto>(value, "OutputWireEnvelopeLevelChangeDto");
}

export function requestLevelChange(change: LevelRequestDto): Promise<Result<LevelChangeDto>> {
  try {
    return invoke<unknown>("app_observability_level_change", {
      request: { schema_version: 1, change },
    }).then(levelResponse, (error: unknown) => err(safeFailure(error, "level change invoke")));
  } catch (error: unknown) {
    return Promise.resolve(err(safeFailure(error, "level change invoke")));
  }
}

const client = transport.kind === "ok" ? createClient(transport.value) : transport;
const event = encodeEvent({
  level: "info",
  target: "tauri-example",
  action: "startup",
  correlation_id: "tauri-example-startup",
  fields: { attempt: 1n },
});
if (client.kind === "ok" && event.kind === "ok") {
  void client.value.tryLog(event.value);
}
