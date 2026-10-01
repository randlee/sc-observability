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
pub(crate) const COMMAND_VALIDATE: &str = "validate";
pub(crate) const COMMAND_EMIT: &str = "emit";
pub(crate) const COMMAND_FLUSH: &str = "flush";
pub(crate) const COMMAND_STATUS: &str = "status";
pub(crate) const STATE_VALIDATED: &str = "validated";
pub(crate) const STATE_STATUS: &str = "status";
pub(crate) const STATE_REJECTED: &str = "rejected";
pub(crate) const STATE_ADMITTED_PENDING: &str = "admitted_pending";
pub(crate) const STATE_ADMITTED_DELIVERED: &str = "admitted_delivered";
pub(crate) const STATE_ADMITTED_FAILED: &str = "admitted_failed";
