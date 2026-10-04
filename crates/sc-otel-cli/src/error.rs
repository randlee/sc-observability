//! CLI-local failures that occur before the shared submission parser is reached.

use crate::constants;
use sc_observability_types::otlp::submission::TelemetryClientError;
use serde_json::Value;
use std::{error::Error, fmt, path::PathBuf};

/// Failures from CLI input transport and fragment decoding.
#[derive(Debug)]
pub(crate) enum InputError {
    /// Reading the process standard input failed.
    Stdin { source: std::io::Error },
    /// Reading an `@file` fragment failed.
    File {
        path: PathBuf,
        source: std::io::Error,
    },
    /// Parsing a named JSON fragment failed.
    Fragment {
        flag: &'static str,
        source: serde_json::Error,
    },
}

impl fmt::Display for InputError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Stdin { source } => write!(formatter, "unable to read stdin: {source}"),
            Self::File { path, source } => {
                write!(
                    formatter,
                    "unable to read input file {}: {source}",
                    path.display()
                )
            }
            Self::Fragment { flag, source } => {
                write!(formatter, "invalid JSON for {flag}: {source}")
            }
        }
    }
}

impl Error for InputError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Stdin { source } | Self::File { source, .. } => Some(source),
            Self::Fragment { source, .. } => Some(source),
        }
    }
}

/// The complete CLI failure boundary.
#[derive(Debug)]
pub(crate) enum CliError {
    /// Input failed before the canonical submission parser could run.
    Input(InputError),
    /// The CLI could not render its own result payload.
    Internal(String),
    /// A shared client or parser failure.
    Telemetry(TelemetryClientError),
}

impl CliError {
    pub(crate) fn code(&self) -> &str {
        match self {
            Self::Input(_) => constants::ERROR_INVALID_JSON,
            Self::Internal(_) => constants::ERROR_INTERNAL,
            Self::Telemetry(error) => error.code().as_str(),
        }
    }

    pub(crate) fn remediation(&self) -> Value {
        match self {
            Self::Input(InputError::Stdin { .. }) => {
                Value::String("supply readable JSON on standard input".into())
            }
            Self::Input(InputError::File { .. }) => {
                Value::String("supply a readable JSON fragment file".into())
            }
            Self::Input(InputError::Fragment { .. }) => {
                Value::String("correct the JSON fragment before retrying".into())
            }
            Self::Internal(_) => {
                Value::String("report the internal CLI failure with the result code".into())
            }
            Self::Telemetry(error) => telemetry_remediation(error),
        }
    }
}

fn telemetry_remediation(error: &TelemetryClientError) -> Value {
    use sc_observability_types::otlp::submission::TelemetryClientError;

    match error {
        TelemetryClientError::Submission(error) => remediation_value(error.context()),
        TelemetryClientError::Admission(error) => remediation_value(error.context()),
        TelemetryClientError::Delivery(error) => remediation_value(error.context()),
        TelemetryClientError::Config(error) => remediation_value(error.context()),
        _ => Value::String("report the unknown telemetry failure with the result code".into()),
    }
}

fn remediation_value(context: &sc_observability_types::ErrorContext) -> Value {
    serde_json::to_value(&context.diagnostic().remediation)
        .expect("the typed remediation is serializable")
}

impl fmt::Display for CliError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Input(error) => error.fmt(formatter),
            Self::Internal(message) => formatter.write_str(message),
            Self::Telemetry(error) => error.fmt(formatter),
        }
    }
}

impl Error for CliError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Input(error) => error.source(),
            Self::Internal(_) => None,
            Self::Telemetry(error) => Some(error),
        }
    }
}

impl From<InputError> for CliError {
    fn from(error: InputError) -> Self {
        Self::Input(error)
    }
}

impl From<TelemetryClientError> for CliError {
    fn from(error: TelemetryClientError) -> Self {
        Self::Telemetry(error)
    }
}
