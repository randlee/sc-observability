import {
  createClient,
  createTauriTransport,
  canonicalErrorCode,
  canonicalErrorNameForCode,
  encodeEvent,
  encodeValue,
  parseWireEnvelope,
  SC_OBSERVABILITY_BINDING_DIAGNOSTIC_TOO_LARGE,
  SC_OBSERVABILITY_BINDING_UNSUPPORTED_VERSION,
  type JsonTransport,
  type Result,
} from "./index";

declare const process: { exitCode: number };

function assert(condition: boolean, message: string): void {
  if (!condition) throw new Error(message);
}

const transport: JsonTransport = {
  async request(operation, request): Promise<Result<unknown>> {
    assert(operation === "try_log", "test transport received an unexpected operation");
    assert(typeof request === "object" && request !== null, "request was not an object");
    return {
      kind: "ok",
      value: { schema_version: 1, kind: "ok", value: { kind: "accepted" } },
    };
  },
};

async function main(): Promise<void> {
  assert(canonicalErrorCode("EventError::Validation") === "SC_OBSERVABILITY_TYPES_VALUE_VALIDATION_FAILED",
    "canonical v2 event name did not retain its stable code");
  assert(canonicalErrorNameForCode("SC_OBSERVABILITY_TYPES_VALUE_VALIDATION_FAILED") === "EventError::Validation",
    "canonical v2 code did not resolve to its variant name");
  assert(canonicalErrorNameForCode("SC_OBSERVABILITY_TYPES_VALUE_VALIDATION_FAILED")?.startsWith("EventError::") === true,
    "retained 1.x wrapper name was accepted as canonical");
  const maximum = encodeValue(18446744073709551615n);
  assert(maximum.kind === "ok" && maximum.value.kind === "integer" && maximum.value.value === "18446744073709551615", "maximum u64 was not encoded losslessly");
  const negative = encodeValue(-9223372036854775808n);
  assert(negative.kind === "ok" && negative.value.kind === "integer" && negative.value.value === "-9223372036854775808", "minimum i64 was not encoded losslessly");
  assert(encodeValue(Number.MAX_SAFE_INTEGER + 1).kind === "error", "unsafe integral number was accepted");
  assert(encodeValue(Number.NaN).kind === "error", "NaN was accepted");
  assert(encodeValue({ "sc_observability::binding::language": "forged" }).kind === "error", "reserved provenance key was accepted");
  const cyclic: { self?: unknown } = {};
  cyclic.self = cyclic;
  assert(encodeValue(cyclic as never).kind === "error", "cyclic input was accepted");
  const getterFailure = {} as { value: unknown };
  Object.defineProperty(getterFailure, "value", { enumerable: true, get: () => { throw new Error("getter"); } });
  assert(encodeValue(getterFailure as never).kind === "error", "getter failure escaped encoding");
  const prototypeKey = encodeValue(JSON.parse('{"__proto__":"sentinel"}'));
  assert(prototypeKey.kind === "ok" && prototypeKey.value.kind === "object" &&
    Object.hasOwn(prototypeKey.value.value, "__proto__") && prototypeKey.value.value["__proto__"]?.kind === "string",
  "__proto__ was silently dropped during encoding");

  const revokedEvent = Proxy.revocable({}, {});
  revokedEvent.revoke();
  assert(encodeEvent(revokedEvent.proxy as never).kind === "error", "revoked Proxy escaped encodeEvent");

  const event = encodeEvent({ level: "info", target: "example", action: "write", fields: { value: 18446744073709551615n } });
  assert(event.kind === "ok", "valid event was rejected");
  if (event.kind === "ok") {
    assert(event.value.fields.value?.kind === "integer", "event integer was not tagged");
    const client = createClient(transport);
    assert(client.kind === "ok", "valid transport was rejected");
    if (client.kind === "ok") {
      const admission = await client.value.tryLog(event.value);
      assert(admission.kind === "ok" && admission.value.kind === "accepted", "accepted admission was not returned");
      const status = client.value.client_status();
      assert(status.kind === "ok" && status.value.in_flight === 0, "in-flight status did not recover");
    }
  }

  const invoked: string[] = [];
  const tauriTransport = createTauriTransport(async (command, args) => {
    invoked.push(command);
    assert(args !== undefined && Object.hasOwn(args, "request"), "Tauri request argument was not wrapped");
    return { schema_version: 1, kind: "ok", value: { kind: "accepted" } };
  });
  assert(tauriTransport.kind === "ok" && event.kind === "ok", "Tauri transport setup failed");
  if (tauriTransport.kind === "ok" && event.kind === "ok") {
    const result = await tauriTransport.value.request("try_log", { schema_version: 1, event: event.value });
    assert(result.kind === "ok" && invoked[0] === "plugin:sc-observability|sc_observability_try_log", "Tauri command mapping failed");
  }

  let queryCalls = 0;
  const queryClient = createClient({
    request: async () => {
      queryCalls += 1;
      return { kind: "ok", value: { schema_version: 1, kind: "ok", value: { events: [], truncated: false } } };
    },
  });
  assert(queryClient.kind === "ok", "query transport setup failed");
  if (queryClient.kind === "ok") {
    const unsupported = await queryClient.value.query({ schema_version: 2 } as never);
    assert(unsupported.kind === "error" && unsupported.error.kind === "unsupported_version" &&
      unsupported.error.code === SC_OBSERVABILITY_BINDING_UNSUPPORTED_VERSION && queryCalls === 0,
    "unsupported query version was dispatched");
    const revokedQuery = Proxy.revocable({}, {});
    revokedQuery.revoke();
    const contained = await queryClient.value.query(revokedQuery.proxy as never);
    assert(contained.kind === "error" && queryCalls === 0, "revoked Proxy escaped query or reached transport");
  }

  let releaseDispatch!: () => void;
  const delayed = createClient({
    request: async () => new Promise<Result<unknown>>((resolve) => {
      releaseDispatch = () => resolve({ kind: "ok", value: { schema_version: 1, kind: "ok", value: { kind: "accepted" } } });
    }),
  });
  assert(delayed.kind === "ok" && event.kind === "ok", "delayed transport setup failed");
  if (delayed.kind === "ok" && event.kind === "ok") {
    assert(delayed.value.log(event.value).kind === "ok", "fire-and-forget dispatch was rejected");
    const pending = delayed.value.client_status();
    assert(pending.kind === "ok" && pending.value.in_flight === 1, "dispatch reservation was not retained");
    releaseDispatch();
    await new Promise((resolve) => setTimeout(resolve, 0));
    const recovered = delayed.value.client_status();
    assert(recovered.kind === "ok" && recovered.value.in_flight === 0, "dispatch reservation was not released");
    if (recovered.kind === "ok") {
      try { (recovered.value.failures_by_kind as Record<string, string>).internal = "999"; } catch { /* frozen snapshot */ }
      const unchanged = delayed.value.client_status();
      assert(unchanged.kind === "ok" && unchanged.value.failures_by_kind.internal === "0", "status retained mutable state");
    }
  }

  const unknownRemote = createClient({
    request: async () => ({ kind: "ok", value: {
      schema_version: 1, kind: "error",
      error: { kind: "future_failure", at: new Date().toISOString(), code: "FUTURE_CODE", message: "future host failure", remediation: { kind: "recoverable", steps: [] } },
    } }),
  });
  assert(unknownRemote.kind === "ok" && event.kind === "ok", "unknown remote setup failed");
  if (unknownRemote.kind === "ok" && event.kind === "ok") {
    const result = await unknownRemote.value.tryLog(event.value);
    assert(result.kind === "error" && result.error.kind === "unknown_remote", "unknown remote was not contained");
    if (result.kind === "error" && result.error.kind === "unknown_remote") {
      assert(result.error.code === "FUTURE_CODE", "unknown remote code was lost");
      assert(result.error.remediation.kind === "recoverable" && result.error.remediation.steps.length === 0, "valid unknown remote remediation was replaced");
    }
  }

  const oversizedRemoteKind = createClient({
    request: async () => ({ kind: "ok", value: {
      schema_version: 1, kind: "error",
      error: { kind: "x".repeat(5000), at: new Date().toISOString(), code: "REMOTE_CODE", message: "short", remediation: { kind: "recoverable", steps: ["Preserve this remote instruction"] } },
    } }),
  });
  assert(oversizedRemoteKind.kind === "ok" && event.kind === "ok", "oversized remote kind setup failed");
  if (oversizedRemoteKind.kind === "ok" && event.kind === "ok") {
    const result = await oversizedRemoteKind.value.tryLog(event.value);
    assert(result.kind === "error" && result.error.code === SC_OBSERVABILITY_BINDING_DIAGNOSTIC_TOO_LARGE, "oversized remote kind was truncated");
  }

  const unsupportedResponse = createClient({
    request: async () => ({ kind: "ok", value: { schema_version: 2, kind: "ok", value: { kind: "accepted" } } }),
  });
  assert(unsupportedResponse.kind === "ok" && event.kind === "ok", "unsupported response setup failed");
  if (unsupportedResponse.kind === "ok" && event.kind === "ok") {
    const result = await unsupportedResponse.value.tryLog(event.value);
    assert(result.kind === "error" && result.error.kind === "unsupported_version" && result.error.received === 2, "unsupported response version was mapped as input validation");
  }

  const malformedKnown = createClient({
    request: async () => ({ kind: "ok", value: { schema_version: 1, kind: "error", error: { kind: "validation", code: "SC_OBSERVABILITY_BINDING_INVALID_INPUT", message: "missing fields" } } }),
  });
  assert(malformedKnown.kind === "ok" && event.kind === "ok", "malformed-known setup failed");
  if (malformedKnown.kind === "ok" && event.kind === "ok") {
    const result = await malformedKnown.value.tryLog(event.value);
    assert(result.kind === "error" && result.error.kind === "validation", "malformed known failure was not validation");
  }

  const frozenAccounting = createClient({
    request: async () => ({ kind: "ok", value: {
      schema_version: 1, kind: "error",
      error: { kind: "closed", at: new Date().toISOString(), code: "SC_OBSERVABILITY_BINDING_CLOSED", message: "closed", remediation: { kind: "recoverable", steps: [] } },
    } }),
  });
  assert(frozenAccounting.kind === "ok" && event.kind === "ok", "frozen accounting setup failed");
  if (frozenAccounting.kind === "ok" && event.kind === "ok") {
    Object.freeze((frozenAccounting.value as unknown as { counts: object }).counts);
    const result = await frozenAccounting.value.tryLog(event.value);
    assert(result.kind === "error" && result.error.kind === "closed", "frozen accounting replaced the original failure");
  }

  const unavailableAccounting = createClient(transport);
  assert(unavailableAccounting.kind === "ok", "unavailable accounting setup failed");
  if (unavailableAccounting.kind === "ok") {
    Object.defineProperty(unavailableAccounting.value, "counts", { configurable: true, get: () => { throw new Error("accounting storage unavailable"); } });
    const status = unavailableAccounting.value.client_status();
    assert(status.kind === "error" && status.error.kind === "internal", "unavailable accounting did not return internal Result");
  }

  const additiveKnown = createClient({
    request: async () => ({ kind: "ok", value: {
      schema_version: 1,
      kind: "error",
      error: {
        kind: "closed",
        at: new Date().toISOString(),
        code: "SC_OBSERVABILITY_BINDING_CLOSED",
        message: "closed",
        remediation: { kind: "recoverable", steps: [] },
        future_detail: "retained",
      },
    } }),
  });
  assert(additiveKnown.kind === "ok" && event.kind === "ok", "additive known setup failed");
  if (additiveKnown.kind === "ok" && event.kind === "ok") {
    const result = await additiveKnown.value.tryLog(event.value);
    assert(result.kind === "error" && result.error.kind === "closed" &&
      (result.error as FailureWithAdditiveField).future_detail === "retained",
    "additive known Failure was not retained");
  }

  const oversizedRemote = createClient({
    request: async () => ({ kind: "ok", value: {
      schema_version: 1,
      kind: "error",
      error: { kind: "future_failure", code: "FUTURE_CODE", message: "x".repeat(5000) },
    } }),
  });
  assert(oversizedRemote.kind === "ok" && event.kind === "ok", "oversized remote setup failed");
  if (oversizedRemote.kind === "ok" && event.kind === "ok") {
    const result = await oversizedRemote.value.tryLog(event.value);
    assert(result.kind === "error" && result.error.kind === "validation" &&
      result.error.code === SC_OBSERVABILITY_BINDING_DIAGNOSTIC_TOO_LARGE,
    "oversized remote diagnostic was silently truncated");
  }

  const throwingLevelResponse = {} as { kind: unknown };
  Object.defineProperty(throwingLevelResponse, "kind", { enumerable: true, get: () => { throw new Error("kind getter"); } });
  const containedLevelResponse = parseWireEnvelope(throwingLevelResponse, "OutputWireEnvelopeLevelChangeDto");
  assert(containedLevelResponse.kind === "error" && containedLevelResponse.error.kind === "internal", "throwing level response escaped shared conversion");
  const revokedLevelResponse = Proxy.revocable({}, {});
  revokedLevelResponse.revoke();
  assert(parseWireEnvelope(revokedLevelResponse.proxy, "OutputWireEnvelopeLevelChangeDto").kind === "error", "revoked level response escaped shared conversion");
  const oversizedKnownLevelResponse = parseWireEnvelope({
    schema_version: 1,
    kind: "error",
    error: {
      kind: "closed",
      at: new Date().toISOString(),
      code: "SC_OBSERVABILITY_BINDING_CLOSED",
      message: "x".repeat(5000),
      remediation: { kind: "recoverable", steps: [] },
    },
  }, "OutputWireEnvelopeLevelChangeDto");
  assert(oversizedKnownLevelResponse.kind === "error" &&
    oversizedKnownLevelResponse.error.code === SC_OBSERVABILITY_BINDING_DIAGNOSTIC_TOO_LARGE,
  "oversized known level response was accepted");

  console.log("TypeScript binding conversion/client smoke tests passed");
}

type FailureWithAdditiveField = { future_detail?: string };

void main().catch((error: unknown) => {
  console.error(error);
  process.exitCode = 1;
});
