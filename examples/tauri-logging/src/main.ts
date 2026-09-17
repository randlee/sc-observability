import { invoke } from "@tauri-apps/api/core";
import {
  createClient,
  createTauriTransport,
  encodeEvent,
  err,
  isRecord,
  ok,
  safeFailure,
  type LevelChangeDto,
  type LevelRequestDto,
  type Result,
  type WireEnvelope,
  validate,
  validation,
} from "@sc-observability/client";

const transport = createTauriTransport(invoke);

function levelResponse(value: unknown): Result<LevelChangeDto> {
  try {
    if (!isRecord(value) || !validate("OutputWireEnvelopeLevelChangeDto", value)) {
      return err(validation("response", "level response failed schema validation"));
    }
    const envelope = value as WireEnvelope<LevelChangeDto>;
    return envelope.kind === "error" ? err(envelope.error) : ok(envelope.value);
  } catch (error: unknown) {
    return err(safeFailure(error, "level change response"));
  }
}

export function requestLevelChange(change: LevelRequestDto): Promise<Result<LevelChangeDto>> {
  try {
    return invoke<WireEnvelope<LevelChangeDto>>("app_observability_level_change", {
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
