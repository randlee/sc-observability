import { type LogEventDto, type ValueDto, validate } from "./generated/index";
import { err, isRecord, ok, type Result, validation } from "./result";

export type EventValueInput =
  | null
  | boolean
  | string
  | number
  | bigint
  | readonly EventValueInput[]
  | { readonly [key: string]: EventValueInput };

export type LogEventInput = Pick<LogEventDto, "level" | "target" | "action"> & Partial<Pick<
  LogEventDto,
  "message" | "trace" | "request_id" | "correlation_id" | "outcome"
>> & {
  fields?: Readonly<Record<string, EventValueInput>>;
};

const MAX_DEPTH = 32;
const MAX_REQUEST_BYTES = 65536;
const MIN_I64 = -(1n << 63n);
const MAX_U64 = (1n << 64n) - 1n;
const RESERVED = "sc_observability.binding.";

function utf8Bytes(value: string): number {
  return new TextEncoder().encode(value).byteLength;
}

function normalizeKey(key: string): string {
  return key
    .replaceAll("::", ".")
    .split("")
    .map((char) => /[A-Za-z0-9._-]/.test(char) ? char : "_")
    .join("");
}

function rejectReserved(key: string, field: string): Result<never> | null {
  if (key.startsWith(RESERVED) || normalizeKey(key).startsWith(RESERVED)) {
    return err(validation(field, "reserved binding provenance fields are host-owned"));
  }
  return null;
}

function encode(value: EventValueInput, depth: number, field: string): Result<ValueDto> {
  if (value === null) return ok({ kind: "null" });
  if (typeof value === "boolean") return ok({ kind: "boolean", value });
  if (typeof value === "string") return ok({ kind: "string", value });
  if (typeof value === "bigint") {
    if (value < MIN_I64 || value > MAX_U64) {
      return err(validation(field, "integer is outside the signed i64/unsigned u64 range"));
    }
    return ok({ kind: "integer", value: value.toString() });
  }
  if (typeof value === "number") {
    if (!Number.isFinite(value)) return err(validation(field, "number must be finite"));
    if (Number.isInteger(value)) {
      if (!Number.isSafeInteger(value)) return err(validation(field, "unsafe integral numbers must use bigint"));
      return ok({ kind: "integer", value: Object.is(value, -0) ? "0" : String(value) });
    }
    return ok({ kind: "float", value });
  }
  if (depth >= MAX_DEPTH) return err(validation(field, "maximum container depth is 32"));

  let prototype: object | null;
  try {
    prototype = Object.getPrototypeOf(value);
  } catch {
    return err(validation(field, "value prototype could not be inspected"));
  }
  if (Array.isArray(value)) {
    const array = value as readonly EventValueInput[];
    const values: ValueDto[] = [];
    try {
      for (let index = 0; index < array.length; index += 1) {
        const item = encode(array[index] as EventValueInput, depth + 1, `${field}[${index}]`);
        if (item.kind === "error") return item;
        values.push(item.value);
      }
    } catch {
      return err(validation(field, "array value could not be read"));
    }
    return ok({ kind: "array", value: values });
  }
  if (prototype !== Object.prototype && prototype !== null) {
    return err(validation(field, "only plain objects are supported"));
  }

  let keys: string[];
  try {
    if (Object.getOwnPropertySymbols(value).length > 0) {
      return err(validation(field, "symbol keys are not supported"));
    }
    keys = Object.keys(value);
  } catch {
    return err(validation(field, "object keys could not be inspected"));
  }
  const output: Record<string, ValueDto> = Object.create(null) as Record<string, ValueDto>;
  const object = value as { readonly [key: string]: EventValueInput };
  for (const key of keys) {
    const reserved = rejectReserved(key, `${field}.${key}`);
    if (reserved) return reserved;
    let child: unknown;
    try {
      child = object[key];
    } catch {
      return err(validation(`${field}.${key}`, "property getter failed"));
    }
    const item = encode(child as EventValueInput, depth + 1, `${field}.${key}`);
    if (item.kind === "error") return item;
    output[key] = item.value;
  }
  return ok({ kind: "object", value: output });
}

function checkSize<T>(result: Result<T>, field: string): Result<T> {
  if (result.kind === "error") return result;
  try {
    if (utf8Bytes(JSON.stringify(result.value)) > MAX_REQUEST_BYTES) {
      return err(validation(field, "request exceeds 65536 UTF-8 bytes"));
    }
  } catch {
    return err(validation(field, "value could not be serialized"));
  }
  return result;
}

export function encodeValue(value: EventValueInput): Result<ValueDto> {
  try {
    return checkSize(encode(value, 0, "value"), "value");
  } catch {
    return err(validation("value", "value could not be encoded"));
  }
}

const EVENT_KEYS = new Set([
  "level", "target", "action", "message", "trace", "request_id", "correlation_id", "outcome", "fields",
]);

export function encodeEvent(event: LogEventInput): Result<LogEventDto> {
  if (!isRecord(event)) return err(validation("event", "event must be a plain object"));
  try {
    const prototype = Object.getPrototypeOf(event);
    if (prototype !== Object.prototype && prototype !== null) return err(validation("event", "event must be a plain object"));
    if (Object.getOwnPropertySymbols(event).length > 0) return err(validation("event", "symbol keys are not supported"));
    for (const key of Object.keys(event)) {
      if (!EVENT_KEYS.has(key)) return err(validation(`event.${key}`, "unknown event field"));
    }
    const level = event.level;
    const target = event.target;
    const action = event.action;
    if (typeof level !== "string" || typeof target !== "string" || typeof action !== "string") {
      return err(validation("event", "level, target and action are required strings"));
    }
    const fields: Record<string, ValueDto> = Object.create(null) as Record<string, ValueDto>;
    const inputFields = event.fields ?? {};
    if (!isRecord(inputFields)) return err(validation("event.fields", "fields must be an object"));
    if (Object.getOwnPropertySymbols(inputFields).length > 0) return err(validation("event.fields", "symbol keys are not supported"));
    for (const key of Object.keys(inputFields)) {
      const reserved = rejectReserved(key, `event.fields.${key}`);
      if (reserved) return reserved;
      const encoded = encodeValue(inputFields[key] as EventValueInput);
      if (encoded.kind === "error") return err(validation(`event.fields.${key}`, encoded.error.message));
      fields[key] = encoded.value;
    }
    const output: LogEventDto = {
      schema_version: 1,
      level: level as LogEventDto["level"],
      target,
      action,
      message: event.message ?? null,
      trace: event.trace ?? null,
      request_id: event.request_id ?? null,
      correlation_id: event.correlation_id ?? null,
      outcome: event.outcome ?? null,
      fields,
    };
    if (!validate("InputLogEventDto", output)) return err(validation("event", "event does not match schema v1"));
    return checkSize(ok(output), "event");
  } catch {
    return err(validation("event", "event could not be encoded"));
  }
}
