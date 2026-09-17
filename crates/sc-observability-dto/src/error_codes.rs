//! Stable binding-owned diagnostics; native codes are passed through unchanged.
/// One stable diagnostic registry entry.
#[derive(Debug, Clone, Copy, serde::Serialize)]
pub struct ErrorEntry {
    /// Wire code.
    pub code: &'static str,
    /// Wire kind.
    pub kind: &'static str,
    /// Wire remediation.
    pub remediation: &'static str,
}
/// Correct the named input field and submit a new request.
pub const SC_OBSERVABILITY_BINDING_INVALID_INPUT: &str = "SC_OBSERVABILITY_BINDING_INVALID_INPUT";
/// Install client and host packages supporting the same schema.
pub const SC_OBSERVABILITY_BINDING_UNSUPPORTED_VERSION: &str =
    "SC_OBSERVABILITY_BINDING_UNSUPPORTED_VERSION";
/// Reduce remote diagnostic text or remediation steps to the documented bounds.
pub const SC_OBSERVABILITY_BINDING_DIAGNOSTIC_TOO_LARGE: &str =
    "SC_OBSERVABILITY_BINDING_DIAGNOSTIC_TOO_LARGE";
/// Stop submitting through the closed backend and inspect its retained health.
pub const SC_OBSERVABILITY_BINDING_CLOSED: &str = "SC_OBSERVABILITY_BINDING_CLOSED";
/// Wait for an outstanding request to complete before submitting again.
pub const SC_OBSERVABILITY_BINDING_DISPATCH_FULL: &str = "SC_OBSERVABILITY_BINDING_DISPATCH_FULL";
/// Wait for the current adapter flush to finish before submitting another.
pub const SC_OBSERVABILITY_BINDING_FLUSH_IN_PROGRESS: &str =
    "SC_OBSERVABILITY_BINDING_FLUSH_IN_PROGRESS";
/// Restore native thread resources before explicitly creating another backend.
pub const SC_OBSERVABILITY_BINDING_COORDINATOR_START_FAILED: &str =
    "SC_OBSERVABILITY_BINDING_COORDINATOR_START_FAILED";
/// Wait for an existing operation observer to finish before registering another.
pub const SC_OBSERVABILITY_BINDING_WAITERS_FULL: &str = "SC_OBSERVABILITY_BINDING_WAITERS_FULL";
/// Wait for the existing query to finish before starting another.
pub const SC_OBSERVABILITY_BINDING_QUERY_IN_PROGRESS: &str =
    "SC_OBSERVABILITY_BINDING_QUERY_IN_PROGRESS";
/// Install a host backend before requesting an attached logger.
pub const SC_OBSERVABILITY_BINDING_HOST_NOT_INSTALLED: &str =
    "SC_OBSERVABILITY_BINDING_HOST_NOT_INSTALLED";
/// Reuse the module's existing backend; replacement is unsupported.
pub const SC_OBSERVABILITY_BINDING_HOST_ALREADY_INSTALLED: &str =
    "SC_OBSERVABILITY_BINDING_HOST_ALREADY_INSTALLED";
/// Request access through the application's authorized window.
pub const SC_OBSERVABILITY_BINDING_PERMISSION_DENIED: &str =
    "SC_OBSERVABILITY_BINDING_PERMISSION_DENIED";
/// Restore the host connection before submitting a new request.
pub const SC_OBSERVABILITY_BINDING_TRANSPORT_UNAVAILABLE: &str =
    "SC_OBSERVABILITY_BINDING_TRANSPORT_UNAVAILABLE";
/// Inspect operation status before deciding whether another operation is needed.
pub const SC_OBSERVABILITY_BINDING_TIMEOUT: &str = "SC_OBSERVABILITY_BINDING_TIMEOUT";
/// Inspect the saved operation result if confirmation is still needed.
pub const SC_OBSERVABILITY_BINDING_CANCELLED: &str = "SC_OBSERVABILITY_BINDING_CANCELLED";
/// Remove logging calls from handler formatting and error callbacks.
pub const SC_OBSERVABILITY_PY_HANDLER_REENTRANT: &str = "SC_OBSERVABILITY_PY_HANDLER_REENTRANT";
/// Inspect the retained status and restore the affected host or client.
pub const SC_OBSERVABILITY_BINDING_INTERNAL: &str = "SC_OBSERVABILITY_BINDING_INTERNAL";
/// Complete ordered binding diagnostic registry.
pub const REGISTRY: &[ErrorEntry] = &[
    ErrorEntry {
        code: SC_OBSERVABILITY_BINDING_INVALID_INPUT,
        kind: "validation",
        remediation: "Correct the named input field and submit a new request",
    },
    ErrorEntry {
        code: SC_OBSERVABILITY_BINDING_UNSUPPORTED_VERSION,
        kind: "unsupported_version",
        remediation: "Install client and host packages supporting the same schema",
    },
    ErrorEntry {
        code: SC_OBSERVABILITY_BINDING_DIAGNOSTIC_TOO_LARGE,
        kind: "validation",
        remediation: "Reduce remote diagnostic text or remediation steps to the documented bounds",
    },
    ErrorEntry {
        code: SC_OBSERVABILITY_BINDING_CLOSED,
        kind: "closed",
        remediation: "Stop submitting through the closed backend and inspect its retained health",
    },
    ErrorEntry {
        code: SC_OBSERVABILITY_BINDING_DISPATCH_FULL,
        kind: "queue_full",
        remediation: "Wait for an outstanding request to complete before submitting again",
    },
    ErrorEntry {
        code: SC_OBSERVABILITY_BINDING_FLUSH_IN_PROGRESS,
        kind: "queue_full",
        remediation: "Wait for the current adapter flush to finish before submitting another",
    },
    ErrorEntry {
        code: SC_OBSERVABILITY_BINDING_COORDINATOR_START_FAILED,
        kind: "unavailable",
        remediation: "Restore native thread resources before explicitly creating another backend",
    },
    ErrorEntry {
        code: SC_OBSERVABILITY_BINDING_WAITERS_FULL,
        kind: "queue_full",
        remediation: "Wait for an existing operation observer to finish before registering another",
    },
    ErrorEntry {
        code: SC_OBSERVABILITY_BINDING_QUERY_IN_PROGRESS,
        kind: "queue_full",
        remediation: "Wait for the existing query to finish before starting another",
    },
    ErrorEntry {
        code: SC_OBSERVABILITY_BINDING_HOST_NOT_INSTALLED,
        kind: "unavailable",
        remediation: "Install a host backend before requesting an attached logger",
    },
    ErrorEntry {
        code: SC_OBSERVABILITY_BINDING_HOST_ALREADY_INSTALLED,
        kind: "unavailable",
        remediation: "Reuse the module's existing backend; replacement is unsupported",
    },
    ErrorEntry {
        code: SC_OBSERVABILITY_BINDING_PERMISSION_DENIED,
        kind: "permission_denied",
        remediation: "Request access through the application's authorized window",
    },
    ErrorEntry {
        code: SC_OBSERVABILITY_BINDING_TRANSPORT_UNAVAILABLE,
        kind: "unavailable",
        remediation: "Restore the host connection before submitting a new request",
    },
    ErrorEntry {
        code: SC_OBSERVABILITY_BINDING_TIMEOUT,
        kind: "timeout",
        remediation: "Inspect operation status before deciding whether another operation is needed",
    },
    ErrorEntry {
        code: SC_OBSERVABILITY_BINDING_CANCELLED,
        kind: "cancelled",
        remediation: "Inspect the saved operation result if confirmation is still needed",
    },
    ErrorEntry {
        code: SC_OBSERVABILITY_PY_HANDLER_REENTRANT,
        kind: "internal",
        remediation: "Remove logging calls from handler formatting and error callbacks",
    },
    ErrorEntry {
        code: SC_OBSERVABILITY_BINDING_INTERNAL,
        kind: "internal",
        remediation: "Inspect the retained status and restore the affected host or client",
    },
];
