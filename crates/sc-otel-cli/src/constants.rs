//! Stable process exit codes and result schema names.

pub(crate) const EXIT_OK: u8 = 0;
pub(crate) const EXIT_INTERNAL: u8 = 1;
pub(crate) const EXIT_USAGE: u8 = 2;
pub(crate) const EXIT_INVALID_INPUT: u8 = 3;
pub(crate) const EXIT_CONFIG: u8 = 4;
pub(crate) const EXIT_ADMISSION: u8 = 5;
pub(crate) const EXIT_DELIVERY_PENDING: u8 = 6;
pub(crate) const EXIT_DELIVERY_FAILED: u8 = 7;
pub(crate) const RESULT_SCHEMA: &str = "sc-otel.result/v1";
pub(crate) const ERROR_INVALID_JSON: &str = "SC_OBSERVABILITY_SUBMIT_INVALID_JSON";
pub(crate) const ERROR_INTERNAL: &str = "SC_OBSERVABILITY_CLI_INTERNAL";
#[cfg(feature = "test-double")]
pub(crate) const TEST_DOUBLE_ENV: &str = "SC_OTEL_TEST_DOUBLE";
#[cfg(feature = "test-double")]
pub(crate) const TEST_DOUBLE_RECORD_ENV: &str = "SC_OTEL_TEST_DOUBLE_RECORD";
/// Test-double builds only; selects the real durable client.
#[cfg(feature = "test-double")]
pub(crate) const REAL_TRANSPORT_ENV: &str = "SC_OTEL_REAL_TRANSPORT";

#[derive(Clone, Copy)]
pub(crate) enum CommandName {
    Validate,
    Emit,
    Flush,
    Status,
}
impl CommandName {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Validate => "validate",
            Self::Emit => "emit",
            Self::Flush => "flush",
            Self::Status => "status",
        }
    }
}

#[derive(Clone, Copy)]
pub(crate) enum OutcomeState {
    Validated,
    Status,
    Rejected,
    AdmittedPending,
    AdmittedDelivered,
    AdmittedFailed,
}
impl OutcomeState {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Validated => "validated",
            Self::Status => "status",
            Self::Rejected => "rejected",
            Self::AdmittedPending => "admitted_pending",
            Self::AdmittedDelivered => "admitted_delivered",
            Self::AdmittedFailed => "admitted_failed",
        }
    }
}
