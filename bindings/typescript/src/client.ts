import {
  type AdmissionDto,
  type ClientOutcome,
  type ClientStatus,
  type CompletionDto,
  type DispatchDto,
  type Failure,
  type FailureCountsDto,
  type LogEventDto,
  type LogHealthDto,
  type LogQueryDto,
  type LogSnapshotDto,
  SC_OBSERVABILITY_BINDING_DISPATCH_FULL,
  validate,
} from "./generated/index";
import { diagnosticTooLarge, err, internal, isFailure, isRecord, ok, safeFailure, type Result, unsupportedVersion, validation } from "./result";

export interface JsonTransport {
  request(operation: "try_log" | "query" | "health" | "flush", request: unknown): Promise<Result<unknown>>;
}

export interface ObservabilityClient {
  log(event: LogEventDto): Result<DispatchDto>;
  tryLog(event: LogEventDto): Promise<Result<AdmissionDto>>;
  query(query: LogQueryDto): Promise<Result<LogSnapshotDto>>;
  health(): Promise<Result<LogHealthDto>>;
  client_status(): Result<ClientStatus>;
  flush(timeoutMs: number): Promise<Result<CompletionDto>>;
}

type Operation = "try_log" | "query" | "health" | "flush";
type FailureKind = Failure["kind"];

export type TauriInvoke = (command: string, args?: Record<string, unknown>) => Promise<unknown>;

const TAURI_COMMANDS: Record<Operation, string> = {
  try_log: "plugin:sc-observability|sc_observability_try_log",
  query: "plugin:sc-observability|sc_observability_query",
  health: "plugin:sc-observability|sc_observability_health",
  flush: "plugin:sc-observability|sc_observability_flush",
};

const FAILURE_KINDS: FailureKind[] = [
  "below_baseline", "cancelled", "closed", "internal", "io", "permission_denied", "queue_full",
  "timeout", "unavailable", "unknown_remote", "unsupported_level", "unsupported_version", "validation",
];

function emptyCounts(): FailureCountsDto {
  return Object.fromEntries(FAILURE_KINDS.map((kind) => [kind, "0"])) as FailureCountsDto;
}

function increment(value: string): string {
  try {
    const number = BigInt(value);
    const max = (1n << 64n) - 1n;
    return (number >= max ? max : number + 1n).toString();
  } catch {
    return "0";
  }
}

const FAILURE_KEYS: Record<string, readonly string[]> = {
  validation: ["kind", "at", "code", "message", "remediation", "field"],
  queue_full: ["kind", "at", "code", "message", "remediation"],
  below_baseline: ["kind", "at", "code", "message", "remediation", "requested", "configured"],
  unsupported_level: ["kind", "at", "code", "message", "remediation", "requested", "available"],
  permission_denied: ["kind", "at", "code", "message", "remediation"],
  closed: ["kind", "at", "code", "message", "remediation"],
  unavailable: ["kind", "at", "code", "message", "remediation"],
  io: ["kind", "at", "code", "message", "remediation"],
  timeout: ["kind", "at", "code", "message", "remediation", "operation"],
  cancelled: ["kind", "at", "code", "message", "remediation", "operation"],
  unsupported_version: ["kind", "at", "code", "message", "remediation", "received"],
  internal: ["kind", "at", "code", "message", "remediation"],
  unknown_remote: ["kind", "at", "code", "message", "remediation", "remote_kind"],
};

function boundedText(value: string): string {
  const max = 4096;
  if (new TextEncoder().encode(value).byteLength <= max) return value;
  let end = value.length;
  while (end > 0 && new TextEncoder().encode(value.slice(0, end)).byteLength > max) end -= 1;
  return value.slice(0, end);
}

function diagnosticExceedsLimit(value: unknown): boolean {
  try {
    if (!isRecord(value)) return false;
    for (const key of ["at", "code", "message", "field", "operation", "remote_kind", "kind"]) {
      const text = value[key];
      if (typeof text === "string" && new TextEncoder().encode(text).byteLength > 4096) return true;
    }
    const remediation = value.remediation;
    if (isRecord(remediation)) {
      const steps = remediation.steps;
      if (Array.isArray(steps) && (steps.length > 32 || steps.some((step) => typeof step !== "string" || new TextEncoder().encode(step).byteLength > 4096))) return true;
    }
  } catch {
    return true;
  }
  return false;
}

function clone<T>(value: T): T {
  return JSON.parse(JSON.stringify(value)) as T;
}

function freezeDeep<T>(value: T): T {
  if (value !== null && typeof value === "object" && !Object.isFrozen(value)) {
    for (const child of Object.values(value as Record<string, unknown>)) freezeDeep(child);
    Object.freeze(value);
  }
  return value;
}

function envelopeFailure(value: unknown, field = "response"): Failure {
  try {
    if (isFailure(value) && isRecord(value) && typeof value.kind === "string") {
      const allowed = FAILURE_KEYS[value.kind];
      if (allowed && allowed.every((key) => Object.hasOwn(value, key))) {
        if (diagnosticExceedsLimit(value)) return diagnosticTooLarge(`${field}.error`);
        return freezeDeep(clone(value) as Failure);
      }
    }
    if (isRecord(value) && typeof value.kind === "string" && !FAILURE_KEYS[value.kind] &&
        typeof value.code === "string" && typeof value.message === "string") {
      if (diagnosticExceedsLimit(value)) return diagnosticTooLarge(`${field}.error`);
      let remediation = { kind: "recoverable" as const, steps: ["Inspect the remote failure and update the client/host contract if required"] };
      if (isRecord(value.remediation) && validate("OutputRemediationDto", value.remediation)) {
        remediation = clone(value.remediation) as typeof remediation;
      }
      return freezeDeep({
        kind: "unknown_remote",
        at: boundedText(typeof value.at === "string" ? value.at : new Date().toISOString()),
        code: boundedText(value.code),
        message: boundedText(value.message),
        remote_kind: boundedText(value.kind),
        remediation,
      });
    }
  } catch {
    // Foreign transport objects, including throwing proxies, are contained.
  }
  return validation(field, "malformed wire response");
}

function hasOnlyKeys(value: unknown, allowed: readonly string[]): boolean {
  try {
    return isRecord(value) && Object.keys(value).every((key) => allowed.includes(key));
  } catch {
    return false;
  }
}

function validEventInput(event: unknown): event is LogEventDto {
  try {
    return hasOnlyKeys(event, ["schema_version", "level", "target", "action", "message", "trace", "request_id", "correlation_id", "outcome", "fields"])
      && validate("InputLogEventDto", event);
  } catch {
    return false;
  }
}

export function parseWireEnvelope<T>(value: unknown, entrypoint: string): Result<T> {
  let raw = value;
  if (isRecord(raw) && raw.kind === "ok" && !Object.hasOwn(raw, "schema_version")) raw = raw.value;
  if (isRecord(raw) && raw.kind === "error" && !Object.hasOwn(raw, "schema_version")) return err(envelopeFailure(raw.error));
  if (isRecord(raw) && typeof raw.schema_version === "number" && Number.isSafeInteger(raw.schema_version) && raw.schema_version >= 0 && raw.schema_version !== 1) {
    return err(unsupportedVersion(raw.schema_version));
  }
  if (!isRecord(raw) || raw.schema_version !== 1 || (raw.kind !== "ok" && raw.kind !== "error")) {
    return err(validation("response", "malformed or unsupported wire envelope"));
  }
  if (raw.kind === "error") return err(envelopeFailure(raw.error));
  try {
    if (!validate(entrypoint, raw)) return err(validation("response", "wire response failed schema validation"));
  } catch {
    return err(validation("response", "wire response could not be validated"));
  }
  return ok(raw.value as T);
}

function normalizeQuery(query: LogQueryDto): Result<LogQueryDto> {
  const allowed = new Set(["schema_version", "service", "levels", "target", "action", "request_id", "correlation_id", "since", "until", "field_matches", "limit", "order"]);
  try {
    if (!isRecord(query)) return err(validation("query", "query must be an object"));
    if (Object.keys(query).some((key) => !allowed.has(key))) return err(validation("query", "unknown query field"));
    if (query.schema_version !== undefined && query.schema_version !== 1) {
      if (typeof query.schema_version === "number" && Number.isSafeInteger(query.schema_version) && query.schema_version >= 0) {
        return err(unsupportedVersion(query.schema_version));
      }
      return err(validation("query.schema_version", "schema_version must be 1"));
    }
    const normalized = {
      schema_version: 1 as const,
      service: query.service ?? null,
      levels: query.levels ?? [],
      target: query.target ?? null,
      action: query.action ?? null,
      request_id: query.request_id ?? null,
      correlation_id: query.correlation_id ?? null,
      since: query.since ?? null,
      until: query.until ?? null,
      field_matches: query.field_matches ?? [],
      limit: query.limit ?? 100,
      order: query.order ?? "oldest_first",
    };
    if (!validate("InputLogQueryDto", normalized)) return err(validation("query", "query does not match schema v1"));
    if (normalized.limit < 1 || normalized.limit > 1000 || !Number.isSafeInteger(normalized.limit)) return err(validation("query.limit", "limit must be an integer in 1..1000"));
    return ok(normalized);
  } catch {
    return err(validation("query", "query could not be validated"));
  }
}

class Client implements ObservabilityClient {
  private inFlight = 0;
  private counts = emptyCounts();
  private lastFailure: Failure | null = null;
  private lastResult: Result<ClientOutcome> = ok({ kind: "idle" });

  public constructor(private readonly transport: JsonTransport) {}

  public log(event: LogEventDto): Result<DispatchDto> {
    const reservation = this.reserve();
    if (reservation.kind === "error") return reservation;
    if (!validEventInput(event)) {
      this.release();
      const failure = validation("event", "event does not match schema v1");
      this.recordFailure(failure);
      return err(failure);
    }
    this.lastResult = ok({ kind: "scheduled", operation: "log" });
    void this.execute<AdmissionDto>("try_log", { schema_version: 1, event }, "OutputWireEnvelopeAdmissionDto")
      .then((result) => {
        if (result.kind === "ok") this.recordSuccess({ kind: result.value.kind, operation: "log" });
        else this.recordFailure(result.error);
      })
      .catch((error: unknown) => this.recordFailure(safeFailure(error, "log dispatch")))
      .finally(() => this.release());
    return ok({ kind: "scheduled" });
  }

  public async tryLog(event: LogEventDto): Promise<Result<AdmissionDto>> {
    const reservation = this.reserve();
    if (reservation.kind === "error") return reservation;
    if (!validEventInput(event)) {
      this.release();
      const failure = validation("event", "event does not match schema v1");
      this.recordFailure(failure);
      return err(failure);
    }
    const result = await this.execute<AdmissionDto>("try_log", { schema_version: 1, event }, "OutputWireEnvelopeAdmissionDto");
    this.release();
    if (result.kind === "error") this.recordFailure(result.error);
    else this.recordSuccess({ kind: result.value.kind, operation: "try_log" });
    return result;
  }

  public async query(query: LogQueryDto): Promise<Result<LogSnapshotDto>> {
    const normalized = normalizeQuery(query);
    if (normalized.kind === "error") {
      this.recordFailure(normalized.error);
      return normalized;
    }
    const reservation = this.reserve();
    if (reservation.kind === "error") return reservation;
    const result = await this.execute<LogSnapshotDto>("query", { schema_version: 1, query: normalized.value }, "OutputWireEnvelopeLogSnapshotDto");
    this.release();
    if (result.kind === "error") this.recordFailure(result.error);
    else this.recordSuccess({ kind: "completed", operation: "query" });
    return result;
  }

  public async health(): Promise<Result<LogHealthDto>> {
    const reservation = this.reserve();
    if (reservation.kind === "error") return reservation;
    const result = await this.execute<LogHealthDto>("health", { schema_version: 1 }, "OutputWireEnvelopeLogHealthDto");
    this.release();
    if (result.kind === "error") this.recordFailure(result.error);
    else this.recordSuccess({ kind: "completed", operation: "health" });
    return result;
  }

  public client_status(): Result<ClientStatus> {
    try {
      return ok(freezeDeep({
        in_flight: this.inFlight,
        failures_by_kind: { ...this.counts },
        last_result: clone(this.lastResult),
        last_failure: this.lastFailure ? clone(this.lastFailure) : null,
      }));
    } catch {
      return err(internal("client status accounting is unavailable"));
    }
  }

  public async flush(timeoutMs: number): Promise<Result<CompletionDto>> {
    if (typeof timeoutMs !== "number" || !Number.isSafeInteger(timeoutMs) || timeoutMs < 0 || timeoutMs > 60000) {
      const failure = validation("timeout_ms", "timeout must be an integer in 0..60000 milliseconds");
      this.recordFailure(failure);
      return err(failure);
    }
    const reservation = this.reserve();
    if (reservation.kind === "error") return reservation;
    const result = await this.execute<CompletionDto>("flush", { schema_version: 1, timeout_ms: timeoutMs }, "OutputWireEnvelopeCompletionDto");
    this.release();
    if (result.kind === "error") this.recordFailure(result.error);
    else this.recordSuccess({ kind: "completed", operation: "flush" });
    return result;
  }

  private reserve(): Result<void> {
    if (this.inFlight >= 256) {
      const failure: Failure = {
        kind: "queue_full",
        at: new Date().toISOString(),
        code: SC_OBSERVABILITY_BINDING_DISPATCH_FULL,
        message: "maximum 256 outstanding requests exceeded",
        remediation: {
          kind: "recoverable",
          steps: ["Wait for an outstanding request to complete before submitting again"],
        },
      };
      this.recordFailure(failure);
      return err(failure);
    }
    this.inFlight += 1;
    return ok(undefined);
  }

  private release(): void {
    this.inFlight = Math.max(0, this.inFlight - 1);
  }

  private async execute<T>(operation: Operation, request: unknown, entrypoint: string): Promise<Result<T>> {
    try {
      const response = await this.transport.request(operation, request);
      if (isRecord(response) && response.kind === "error") return err(envelopeFailure(response.error));
      const value = isRecord(response) && response.kind === "ok" ? response.value : response;
      return parseWireEnvelope<T>(value, entrypoint);
    } catch (error: unknown) {
      return err(safeFailure(error, `${operation} transport`));
    }
  }

  private recordSuccess(outcome: ClientOutcome): void {
    try {
      this.lastResult = freezeDeep(ok({ ...outcome }));
    } catch {
      // Accounting is best effort and must not alter the successful result.
    }
  }

  private recordFailure(failure: Failure): void {
    try {
      const retained = freezeDeep(clone(failure));
      this.counts[retained.kind] = increment(this.counts[retained.kind]);
      this.lastFailure = retained;
      this.lastResult = freezeDeep(err(retained));
    } catch {
      // Accounting is best effort and must not replace the original failure.
    }
  }
}

export function createClient(transport: JsonTransport): Result<ObservabilityClient> {
  try {
    if (!isRecord(transport) || typeof transport.request !== "function") {
      return err(validation("transport", "transport.request must be a function"));
    }
    return ok(new Client(transport as JsonTransport));
  } catch {
    return err(validation("transport", "transport.request could not be inspected"));
  }
}

export function createTauriTransport(invoke: TauriInvoke): Result<JsonTransport> {
  try {
    if (typeof invoke !== "function") return err(validation("invoke", "invoke must be a function"));
    return ok({
      request(operation: Operation, request: unknown): Promise<Result<unknown>> {
        return Promise.resolve()
          .then(() => invoke(TAURI_COMMANDS[operation], { request }))
          .then((value) => ok(value), (error: unknown) => err(safeFailure(error, "tauri invoke")));
      },
    });
  } catch {
    return err(validation("invoke", "invoke could not be configured"));
  }
}
