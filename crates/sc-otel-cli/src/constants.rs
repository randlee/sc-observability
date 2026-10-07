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

#[cfg(test)]
pub(crate) const COMMAND_CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");
#[cfg(test)]
pub(crate) const SNAPSHOT_HISTORY_BASELINE: &str = "51be650a08e2a118039eb62ae0f1af7cdc78789a";
#[cfg(test)]
pub(crate) const UNACCEPTED_INITIAL_COMMAND_SNAPSHOT: &str =
    "schema/cli/sc-otel/commands/1.5.0.json";
#[cfg(test)]
pub(crate) const UNACCEPTED_INITIAL_RESULT_SNAPSHOT: &str = "schema/cli/sc-otel/results/v1.json";

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
// Keep the enum, its string mapping, and test-only contract list in one
// declaration. A new variant therefore cannot be added without updating the
// values exposed by the versioned result contract.
macro_rules! stable_contract_enum {
    ($name:ident { $($variant:ident => $value:literal),+ $(,)? }) => {
        #[derive(Clone, Copy)]
        pub(crate) enum $name {
            $($variant),+
        }

        impl $name {
            #[cfg(test)]
            pub(crate) const ALL: &[Self] = &[$(Self::$variant),+];

            pub(crate) const fn as_str(self) -> &'static str {
                match self {
                    $(Self::$variant => $value),+
                }
            }
        }
    };
}

stable_contract_enum!(CommandName {
    Validate => "validate",
    Emit => "emit",
    Flush => "flush",
    Status => "status",
});

stable_contract_enum!(OutcomeState {
    Validated => "validated",
    Status => "status",
    Rejected => "rejected",
    AdmittedPending => "admitted_pending",
    AdmittedDelivered => "admitted_delivered",
    AdmittedFailed => "admitted_failed",
});
