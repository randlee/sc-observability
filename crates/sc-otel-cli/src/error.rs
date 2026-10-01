//! CLI-local failures that occur before the shared submission parser is reached.

use crate::constants;
use sc_observability_types::otlp::submission::TelemetryClientError;
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
    /// A shared client or parser failure.
    Telemetry(TelemetryClientError),
}

impl CliError {
    pub(crate) fn code(&self) -> &str {
        match self {
            Self::Input(_) => constants::ERROR_INVALID_JSON,
            Self::Telemetry(error) => error.code().as_str(),
        }
    }

    pub(crate) fn remediation(&self) -> &'static str {
        match self {
            Self::Input(InputError::Stdin { .. }) => "supply readable JSON on standard input",
            Self::Input(InputError::File { .. }) => "supply a readable JSON fragment file",
            Self::Input(InputError::Fragment { .. }) => "correct the JSON fragment before retrying",
            Self::Telemetry(_) => "follow the remediation attached to the telemetry diagnostic",
        }
    }

    pub(crate) fn telemetry(&self) -> Option<&TelemetryClientError> {
        match self {
            Self::Input(_) => None,
            Self::Telemetry(error) => Some(error),
        }
    }
}

impl fmt::Display for CliError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Input(error) => error.fmt(formatter),
            Self::Telemetry(error) => error.fmt(formatter),
        }
    }
}

impl Error for CliError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Input(error) => error.source(),
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
