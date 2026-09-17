import {
  type Failure,
  type RemediationDto,
  SC_OBSERVABILITY_BINDING_INTERNAL,
  SC_OBSERVABILITY_BINDING_INVALID_INPUT,
} from "./generated/index";

export type Result<T> = { kind: "ok"; value: T } | { kind: "error"; error: Failure };

export function ok<T>(value: T): Result<T> {
  return { kind: "ok", value };
}

export function err<T = never>(error: Failure): Result<T> {
  return { kind: "error", error };
}

const recoverable = (step: string): RemediationDto => ({ kind: "recoverable", steps: [step] });

export function validation(field: string, message: string): Failure {
  return {
    kind: "validation",
    at: new Date().toISOString(),
    code: SC_OBSERVABILITY_BINDING_INVALID_INPUT,
    message,
    field,
    remediation: recoverable(`Correct ${field} and submit a new request`),
  };
}

export function internal(message: string): Failure {
  return {
    kind: "internal",
    at: new Date().toISOString(),
    code: SC_OBSERVABILITY_BINDING_INTERNAL,
    message,
    remediation: recoverable("Inspect the retained status and restore the affected host or client"),
  };
}

export function isFailure(value: unknown): value is Failure {
  if (!isRecord(value) || typeof value.kind !== "string") return false;
  return [
    "validation", "queue_full", "below_baseline", "unsupported_level", "permission_denied",
    "closed", "unavailable", "io", "timeout", "cancelled", "unsupported_version",
    "internal", "unknown_remote",
  ].includes(value.kind);
}

export function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

export function safeFailure(error: unknown, context: string): Failure {
  if (isFailure(error)) return error;
  return internal(`${context}: ${error instanceof Error ? error.message : "foreign operation failed"}`);
}

