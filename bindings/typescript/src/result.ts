import {
  CANONICAL_ERROR_CODES,
  type Failure,
  type RemediationDto,
  SC_OBSERVABILITY_BINDING_DIAGNOSTIC_TOO_LARGE,
  SC_OBSERVABILITY_BINDING_INTERNAL,
  SC_OBSERVABILITY_BINDING_INVALID_INPUT,
  SC_OBSERVABILITY_BINDING_UNSUPPORTED_VERSION,
  validate,
} from "./generated/index";

export { CANONICAL_ERROR_CODES } from "./generated/index";

/** Canonical D.12 variant names and codes carried by language projections. */
export type CanonicalErrorName = keyof typeof CANONICAL_ERROR_CODES;

export function canonicalErrorCode(name: CanonicalErrorName): string {
  return CANONICAL_ERROR_CODES[name];
}

export function canonicalErrorNamesForCode(code: string): readonly CanonicalErrorName[] {
  return (Object.keys(CANONICAL_ERROR_CODES) as CanonicalErrorName[]).filter(
    (name) => CANONICAL_ERROR_CODES[name] === code,
  );
}

/**
 * Resolves a code only when it identifies one canonical variant.
 *
 * Several canonical variants intentionally share a diagnostic code. Returning
 * `undefined` for those ambiguous codes prevents a language binding from
 * inventing a singular variant identity; callers that need the candidates can
 * use `canonicalErrorNamesForCode`.
 */
export function canonicalErrorNameForCode(code: string): CanonicalErrorName | undefined {
  const names = canonicalErrorNamesForCode(code);
  return names.length === 1 ? names[0] : undefined;
}

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

export function unsupportedVersion(received: number): Failure {
  return {
    kind: "unsupported_version",
    at: new Date().toISOString(),
    code: SC_OBSERVABILITY_BINDING_UNSUPPORTED_VERSION,
    message: `unsupported schema version ${received}`,
    received,
    remediation: recoverable("Install client and host packages supporting the same schema"),
  };
}

export function diagnosticTooLarge(field: string): Failure {
  return {
    kind: "validation",
    at: new Date().toISOString(),
    code: SC_OBSERVABILITY_BINDING_DIAGNOSTIC_TOO_LARGE,
    message: "remote diagnostic exceeds the documented size limit",
    field,
    remediation: recoverable("Reduce remote diagnostic text or remediation steps to the documented bounds"),
  };
}

export function isFailure(value: unknown): value is Failure {
  try {
    return isRecord(value) && validate("OutputFailure", value);
  } catch {
    return false;
  }
}

export function isRecord(value: unknown): value is Record<string, unknown> {
  try {
    return typeof value === "object" && value !== null && !Array.isArray(value);
  } catch {
    return false;
  }
}

export function safeFailure(error: unknown, context: string): Failure {
  try {
    if (isFailure(error)) return error;
    return internal(`${context}: ${error instanceof Error ? error.message : "foreign operation failed"}`);
  } catch {
    return internal(`${context}: foreign operation failed`);
  }
}
