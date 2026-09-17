import {
  createClient,
  encodeEvent,
  encodeValue,
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

  let releaseDispatch!: () => void;
  const delayed = createClient({
    request: async () => new Promise<Result<unknown>>((resolve) => {
      releaseDispatch = () => resolve({
        kind: "ok",
        value: { schema_version: 1, kind: "ok", value: { kind: "accepted" } },
      });
    }),
  });
  assert(delayed.kind === "ok" && event.kind === "ok", "delayed transport setup failed");
  if (delayed.kind === "ok" && event.kind === "ok") {
    const dispatch = delayed.value.log(event.value);
    assert(dispatch.kind === "ok", "fire-and-forget dispatch was rejected");
    const pending = delayed.value.client_status();
    assert(pending.kind === "ok" && pending.value.in_flight === 1, "dispatch reservation was not retained");
    releaseDispatch();
    await new Promise((resolve) => setTimeout(resolve, 0));
    const recovered = delayed.value.client_status();
    assert(recovered.kind === "ok" && recovered.value.in_flight === 0, "dispatch reservation was not released exactly once");
    if (recovered.kind === "ok") {
      try {
        (recovered.value.failures_by_kind as Record<string, string>).internal = "999";
      } catch {
        // Object.freeze throws in strict mode; either outcome must not mutate state.
      }
      const unchanged = delayed.value.client_status();
      assert(unchanged.kind === "ok" && unchanged.value.failures_by_kind.internal === "0", "status exposed mutable retained state");
    }
  }

  const unknownRemote = createClient({
    request: async () => ({
      kind: "ok",
      value: {
        schema_version: 1,
        kind: "error",
        error: {
          kind: "future_failure",
          at: new Date().toISOString(),
          code: "FUTURE_CODE",
          message: "future host failure",
          remediation: { kind: "recoverable", steps: [] },
        },
      },
    }),
  });
  assert(unknownRemote.kind === "ok" && event.kind === "ok", "unknown remote setup failed");
  if (unknownRemote.kind === "ok" && event.kind === "ok") {
    const result = await unknownRemote.value.tryLog(event.value);
    assert(result.kind === "error" && result.error.kind === "unknown_remote", "unknown remote failure was not retained as unknown_remote");
    if (result.kind === "error" && result.error.kind === "unknown_remote") assert(result.error.code === "FUTURE_CODE" && result.error.remote_kind === "future_failure", "unknown remote details were lost");
  }

  const malformedKnown = createClient({
    request: async () => ({
      kind: "ok",
      value: {
        schema_version: 1,
        kind: "error",
        error: { kind: "validation", code: "SC_OBSERVABILITY_BINDING_INVALID_INPUT", message: "missing required fields" },
      },
    }),
  });
  assert(malformedKnown.kind === "ok" && event.kind === "ok", "malformed-known setup failed");
  if (malformedKnown.kind === "ok" && event.kind === "ok") {
    const result = await malformedKnown.value.tryLog(event.value);
    assert(result.kind === "error" && result.error.kind === "validation", "malformed known failure was not validation");
  }

  console.log("TypeScript binding conversion/client smoke tests passed");
}

void main().catch((error: unknown) => {
  console.error(error);
  process.exitCode = 1;
});
