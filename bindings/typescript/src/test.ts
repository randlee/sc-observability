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

  console.log("TypeScript binding conversion/client smoke tests passed");
}

void main().catch((error: unknown) => {
  console.error(error);
  process.exitCode = 1;
});
