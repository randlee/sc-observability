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

/// The stable meanings for machine-readable process exits.
///
/// This table is consumed both by the renderer contract and its versioned
/// snapshot, preventing the schema artifact from becoming a second source of
/// truth for exit semantics.
#[cfg(test)]
pub(crate) const EXIT_CODE_MEANINGS: &[(u8, &str)] = &[
    (EXIT_OK, "success"),
    (EXIT_INTERNAL, "internal_failure"),
    (EXIT_USAGE, "clap_usage_error"),
    (EXIT_INVALID_INPUT, "invalid_submission_input"),
    (EXIT_CONFIG, "invalid_or_missing_configuration"),
    (EXIT_ADMISSION, "submission_not_admitted"),
    (EXIT_DELIVERY_PENDING, "admitted_delivery_pending"),
    (EXIT_DELIVERY_FAILED, "admitted_delivery_failed"),
];
#[cfg(feature = "test-double")]
pub(crate) const TEST_DOUBLE_ENV: &str = "SC_OTEL_TEST_DOUBLE";
#[cfg(feature = "test-double")]
pub(crate) const TEST_DOUBLE_RECORD_ENV: &str = "SC_OTEL_TEST_DOUBLE_RECORD";

#[derive(Clone, Copy)]
pub(crate) enum CommandName {
    Validate,
    Emit,
    Flush,
    Status,
}
impl CommandName {
    #[cfg(test)]
    pub(crate) const ALL: [Self; 4] = [Self::Validate, Self::Emit, Self::Flush, Self::Status];

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
    #[cfg(test)]
    pub(crate) const ALL: [Self; 6] = [
        Self::Validated,
        Self::Status,
        Self::Rejected,
        Self::AdmittedPending,
        Self::AdmittedDelivered,
        Self::AdmittedFailed,
    ];

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
