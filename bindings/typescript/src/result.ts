import {
  type Failure,
  type RemediationDto,
  SC_OBSERVABILITY_BINDING_DIAGNOSTIC_TOO_LARGE,
  SC_OBSERVABILITY_BINDING_INTERNAL,
  SC_OBSERVABILITY_BINDING_INVALID_INPUT,
  SC_OBSERVABILITY_BINDING_UNSUPPORTED_VERSION,
  validate,
} from "./generated/index";

/** Canonical D.12 variant names and codes carried by language projections. */
export const CANONICAL_ERROR_CODES = {
  "IdentityError::Process": "SC_OBSERVABILITY_TYPES_IDENTITY_RESOLUTION_FAILED",
  "InitError::Configuration": "SC_OBSERVABILITY_TYPES_DIAGNOSTIC_INVALID",
  "InitError::Runtime": "SC_OBSERVABILITY_TYPES_DIAGNOSTIC_INVALID",
  "EventError::Validation": "SC_OBSERVABILITY_TYPES_VALUE_VALIDATION_FAILED",
  "EventError::Routing": "SC_OBSERVABILITY_TYPES_DIAGNOSTIC_INVALID",
  "FlushError::Drain": "SC_LOG_QUERY_IO",
  "ShutdownError::Timeout": "SC_OBSERVABILITY_LEVEL_STOPPING",
  "ShutdownError::Drain": "SC_OBSERVABILITY_LEVEL_STOPPED",
  "ProjectionError::Projection": "SC_OBSERVABILITY_TYPES_DIAGNOSTIC_INVALID",
  "SubscriberError::Subscriber": "SC_OBSERVABILITY_TYPES_DIAGNOSTIC_INVALID",
  "LogSinkError::Write": "SC_OBSERVABILITY_TYPES_DIAGNOSTIC_INVALID",
  "LogSinkError::Flush": "SC_OBSERVABILITY_TYPES_DIAGNOSTIC_INVALID",
  "ConfigFailure::ZeroDuration": "OTLP_CONFIG_ZERO_DURATION",
  "ConfigFailure::DurationOverflow": "OTLP_CONFIG_DURATION_OVERFLOW",
  "ConfigFailure::InvalidBoundOrdering": "OTLP_CONFIG_BOUND_ORDER",
  "ConfigFailure::InvalidJitterPercent": "OTLP_CONFIG_JITTER_PERCENT",
  "ConfigFailure::InvalidQueueCapacity": "OTLP_CONFIG_QUEUE_CAPACITY",
  "ConfigFailure::InvalidQueueByteCapacity": "OTLP_CONFIG_QUEUE_BYTE_CAPACITY",
  "ConfigFailure::ConfigFieldNotApplicable": "OTLP_CONFIG_FIELD_NOT_APPLICABLE",
  "ConfigFailure::InsecureTransportRejected": "OTLP_CONFIG_INSECURE_TRANSPORT_REJECTED",
  "ConfigFailure::InvalidEndpoint": "OTLP_CONFIG_INVALID_ENDPOINT",
  "ConfigFailure::InvalidHeader": "OTLP_CONFIG_INVALID_HEADER",
  "ConfigFailure::TransportConstructionFailed": "OTLP_TRANSPORT_CONSTRUCTION_FAILED",
  "ConfigFailure::UnsupportedBackend": "OTLP_UNSUPPORTED_BACKEND",
  "ConfigFailure::UnsupportedProtocol": "OTLP_UNSUPPORTED_PROTOCOL",
  "ConfigFailure::TokioRuntimeRequired": "OTLP_TOKIO_RUNTIME_REQUIRED",
  "MetricModelError::InvalidHistogram": "SC_METRIC_INVALID_HISTOGRAM",
  "MetricModelError::InvalidTemporality": "SC_METRIC_INVALID_TEMPORALITY",
  "MetricModelError::InvalidInterval": "SC_METRIC_INVALID_INTERVAL",
} as const;

export type CanonicalErrorName = keyof typeof CANONICAL_ERROR_CODES;

export function canonicalErrorCode(name: CanonicalErrorName): string {
  return CANONICAL_ERROR_CODES[name];
}

export function canonicalErrorNameForCode(code: string): CanonicalErrorName | undefined {
  return (Object.keys(CANONICAL_ERROR_CODES) as CanonicalErrorName[]).find(
    (name) => CANONICAL_ERROR_CODES[name] === code,
  );
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
